//! Telling what kind of folder this is, for `sourceKind: "auto"`.
//! See `docs/beyond-obsidian.md#detection`.

use std::fs;
use std::path::{Path, PathBuf};

use super::formats::{count_links, read_note};
use super::guard::{Guard, is_dataless};
use super::kind::{SourceKind, is_note_name};

/// What detection decided, and why (for the log and the settings page).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub kind: SourceKind,
    pub reason: String,
}

/// Entries the sample looks at, notes it reads, and bytes per note.
const SAMPLE_ENTRIES: usize = 2_000;
const SAMPLE_NOTES: usize = 200;
const SAMPLE_BYTES: usize = 256 * 1024;

pub fn detect(root: &Path) -> Detection {
    let found = |kind: SourceKind, reason: &str| Detection { kind, reason: reason.to_string() };
    if root.join(".obsidian").is_dir() {
        return found(SourceKind::Obsidian, "found .obsidian");
    }
    if root.join("logseq/config.edn").is_file() {
        return found(SourceKind::Logseq, "found logseq/config.edn");
    }
    if root.join("dendron.yml").is_file() {
        return found(SourceKind::Dendron, "found dendron.yml");
    }
    if root.join(".foam").is_dir() {
        return found(SourceKind::Notes, "found .foam");
    }
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from);
    if let Some(vault) = obsidian_parent(root, home.as_deref()) {
        let name = vault.file_name().map_or_else(|| vault.display().to_string(), |n| n.to_string_lossy().into_owned());
        return Detection { kind: SourceKind::Obsidian, reason: format!("inside the Obsidian vault “{name}”") };
    }
    sample(root)
}

/// The nearest parent (up to the home folder, never above it) that is an Obsidian vault.
fn obsidian_parent(root: &Path, home: Option<&Path>) -> Option<PathBuf> {
    let mut dir = root.parent();
    while let Some(d) = dir {
        // In the sandbox parents can't be read; is_dir is then simply false.
        if d.join(".obsidian").is_dir() {
            return Some(d.to_path_buf());
        }
        if home.is_none_or(|h| d == h || !d.starts_with(h)) {
            return None;
        }
        dir = d.parent();
    }
    None
}

/// Looks at a bounded sample: are there notes, and do they link to each other?
fn sample(root: &Path) -> Detection {
    let guard = Guard::default();
    let mut files = 0usize;
    let mut notes: Vec<PathBuf> = Vec::new();
    let mut seen = 0usize;
    let mut queue = std::collections::VecDeque::from([root.to_path_buf()]);
    'walk: while let Some(dir) = queue.pop_front() {
        let Ok(read) = fs::read_dir(&dir) else { continue };
        let mut names: Vec<(String, PathBuf)> =
            read.flatten().filter_map(|e| Some((e.file_name().to_str()?.to_string(), e.path()))).collect();
        names.sort_unstable();
        for (name, path) in names {
            seen += 1;
            if seen > SAMPLE_ENTRIES {
                break 'walk;
            }
            if name.starts_with('.') {
                continue;
            }
            let Ok(meta) = fs::symlink_metadata(&path) else { continue };
            if is_dataless(&meta) {
                continue;
            }
            if meta.is_dir() {
                if !guard.skip_dir(&dir, &name, &path) {
                    queue.push_back(path);
                }
            } else if meta.is_file() {
                files += 1;
                if is_note_name(&name) {
                    notes.push(path);
                }
            }
        }
    }
    if notes.is_empty() {
        return Detection { kind: SourceKind::Folder, reason: "no notes".into() };
    }
    let read: Vec<&PathBuf> = notes.iter().take(SAMPLE_NOTES).collect();
    let links: usize = read
        .iter()
        .filter_map(|p| read_note(p))
        .map(|(_, text)| {
            let mut cut = text.len().min(SAMPLE_BYTES);
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            count_links(&text[..cut])
        })
        .sum();
    let share = notes.len() as f64 / files as f64;
    let per_note = links as f64 / read.len() as f64;
    if share >= 0.25 && per_note >= 0.2 {
        Detection { kind: SourceKind::Notes, reason: "notes with links".into() }
    } else if share < 0.25 {
        Detection { kind: SourceKind::Folder, reason: "mostly files that aren't notes".into() }
    } else {
        Detection { kind: SourceKind::Folder, reason: "notes without links".into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    fn kind(root: &Path) -> SourceKind {
        detect(root).kind
    }

    #[test]
    fn markers_win() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        write(r, "a.txt", "");
        assert_eq!(kind(r), SourceKind::Folder);
        write(r, ".foam/x", "");
        assert_eq!(kind(r), SourceKind::Notes);
        write(r, "dendron.yml", "version: 5");
        assert_eq!(kind(r), SourceKind::Dendron);
        write(r, "logseq/config.edn", "{}");
        assert_eq!(kind(r), SourceKind::Logseq);
        fs::create_dir(r.join(".obsidian")).unwrap();
        assert_eq!(kind(r), SourceKind::Obsidian);
    }

    #[test]
    fn folder_inside_a_vault_is_obsidian() {
        // Under a fake HOME, so the walk up stops there.
        let d = tempfile::tempdir().unwrap();
        let vault = d.path().join("Vault");
        fs::create_dir_all(vault.join(".obsidian")).unwrap();
        write(&vault, "Projects/a.md", "plain");
        assert!(obsidian_parent(&vault.join("Projects"), Some(d.path())).is_some());
        assert!(obsidian_parent(d.path(), Some(d.path())).is_none());
    }

    #[test]
    fn linked_notes_plain_markdown_and_other_trees() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        for i in 0..10 {
            write(r, &format!("n{i}.md"), &format!("see [[n{}]]", (i + 1) % 10));
        }
        assert_eq!(detect(r), Detection { kind: SourceKind::Notes, reason: "notes with links".into() });

        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        for i in 0..10 {
            write(r, &format!("n{i}.md"), "just text");
        }
        assert_eq!(detect(r).reason, "notes without links");

        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        write(r, "README.md", "[[x]] [[y]]");
        for i in 0..10 {
            write(r, &format!("src/f{i}.rs"), "fn main() {}");
        }
        write(r, "node_modules/pkg/a.md", "[[x]]");
        assert_eq!(detect(r).kind, SourceKind::Folder);
    }
}
