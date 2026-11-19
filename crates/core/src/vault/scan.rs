//! Finding and reading the vault's markdown files.
//!
//! Same rules as the web version's `collectMarkdownFiles` / `parseMarkdownEntry`: names
//! starting with "." are skipped at every level, `ignorePaths` entries match a whole
//! relative path, a path prefix, or any single folder name, only files ending in exactly
//! ".md" count, and symlinks are not followed (Node's `Dirent` reports them as neither
//! file nor directory).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rayon::prelude::*;

use super::formats::NoteExtra;
use super::guard::{Guard, is_dataless};
use super::kind::SourceKind;
use super::parse::{extract_tags, extract_wikilinks};

/// One parsed note.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    /// File name without the ".md" suffix.
    pub basename: String,
    /// Modification time in milliseconds since the epoch, like Node's `stat.mtimeMs`.
    pub mtime_ms: f64,
    pub tags: Vec<String>,
    pub wikilinks: Vec<String>,
    /// Bytes on disk, for `nodeSize: "fileSize"`.
    pub size: u64,
    /// What the other notes formats know beyond tags and wikilinks. `None` for Obsidian.
    pub extra: Option<Box<NoteExtra>>,
}

/// The path relative to the vault root, as a string with the platform separator,
/// or `None` when it is the root itself or outside it.
pub(crate) fn relative(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let rel = rel.to_str()?;
    if rel.is_empty() { None } else { Some(rel.to_string()) }
}

/// `isIgnoredPath`: true when a pattern equals the relative path, is a prefix folder of
/// it, or equals any one of its segments. Paths outside the vault count as ignored.
pub fn is_ignored(root: &Path, path: &Path, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return false;
    }
    let Some(rel) = relative(root, path) else { return true };
    let sep = std::path::MAIN_SEPARATOR;
    patterns.iter().any(|p| {
        rel == *p || rel.starts_with(&format!("{p}{sep}")) || rel.split(sep).any(|seg| seg == p)
    })
}

/// True when any segment of the path below the root starts with ".".
pub fn has_hidden_segment(root: &Path, path: &Path) -> bool {
    match relative(root, path) {
        None => true,
        Some(rel) => rel.split(std::path::MAIN_SEPARATOR).any(|seg| seg.starts_with('.')),
    }
}

pub fn is_markdown(path: &Path) -> bool {
    path.to_str().is_some_and(|p| p.ends_with(".md"))
}

/// All markdown files under `dir` (which is the root or a folder inside it).
pub fn collect_markdown_files(root: &Path, dir: &Path, patterns: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read) = fs::read_dir(&dir) else { continue };
        for entry in read.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            if is_ignored(root, &path, patterns) {
                continue;
            }
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() && name.ends_with(".md") {
                files.push(path);
            }
        }
    }
    files
}

/// Reads and parses one note. `None` when it vanished or can't be read.
pub fn parse_entry(path: &Path) -> Option<Entry> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("note: skipped {} ({e})", path.display());
            }
            return None;
        }
    };
    let content = String::from_utf8_lossy(&bytes);
    let name = path.file_name()?.to_str()?;
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0.0, |d| d.as_secs() as f64 * 1000.0 + f64::from(d.subsec_nanos()) / 1e6);
    Some(Entry {
        path: path.to_path_buf(),
        basename: name.strip_suffix(".md").unwrap_or(name).to_string(),
        mtime_ms,
        tags: extract_tags(&content),
        wikilinks: extract_wikilinks(&content),
        size: meta.len(),
        extra: None,
    })
}

/// Parses many files in parallel, dropping any that fail.
pub fn parse_entries(paths: &[PathBuf]) -> Vec<Entry> {
    paths.par_iter().filter_map(|p| parse_entry(p)).collect()
}

/// The files a walk found.
#[derive(Debug, Default)]
pub struct Walk {
    pub files: Vec<PathBuf>,
    /// The walk stopped at the guard's cap; there are more files than `files`.
    pub capped: bool,
}

/// The files of `kind` under `dir` (the root or a folder inside it). Obsidian walks
/// exactly as [`collect_markdown_files`]. Every other kind walks breadth first, in
/// name order (so a capped walk keeps the same files every time), with the guard's
/// skips, without entering or reading iCloud placeholders, and stops at the cap.
/// `already` is how many files the vault holds outside `dir`, counted against the cap.
pub fn collect_files(root: &Path, dir: &Path, patterns: &[String], kind: SourceKind, guard: &Guard, already: usize) -> Walk {
    if !kind.guarded() {
        return Walk { files: collect_markdown_files(root, dir, patterns), capped: false };
    }
    let mut walk = Walk::default();
    let mut visited = 0usize;
    let mut queue = std::collections::VecDeque::from([dir.to_path_buf()]);
    while let Some(dir) = queue.pop_front() {
        let Ok(read) = fs::read_dir(&dir) else { continue };
        let mut names: Vec<(String, PathBuf)> = read
            .flatten()
            .filter_map(|e| Some((e.file_name().to_str()?.to_string(), e.path())))
            .collect();
        names.sort_unstable();
        for (name, path) in names {
            visited += 1;
            if visited > guard.max_visited {
                walk.capped = true;
                return walk;
            }
            if name.starts_with('.') || is_ignored(root, &path, patterns) {
                continue;
            }
            // lstat: symlinks are never followed, and nothing is downloaded.
            let Ok(meta) = fs::symlink_metadata(&path) else { continue };
            if is_dataless(&meta) {
                continue;
            }
            if meta.is_dir() {
                if !guard.skip_dir(&dir, &name, &path) {
                    queue.push_back(path);
                }
            } else if meta.is_file() && kind.wants_name(&name) {
                if already + walk.files.len() >= guard.max_files {
                    walk.capped = true;
                    return walk;
                }
                walk.files.push(path);
            }
        }
    }
    walk
}

/// Whether a path inside a guarded vault is skipped by the guard: any folder on the
/// way down that the walk would not enter.
pub fn guard_skips(root: &Path, path: &Path, guard: &Guard) -> bool {
    let Ok(rel) = path.strip_prefix(root) else { return true };
    let mut parent = root.to_path_buf();
    let mut comps = rel.components().peekable();
    while let Some(c) = comps.next() {
        let here = parent.join(c);
        // The last component is the file itself (or the folder an event named).
        let is_last = comps.peek().is_none();
        if let Some(name) = c.as_os_str().to_str()
            && (!is_last || here.is_dir())
            && guard.skip_dir(&parent, name, &here)
        {
            return true;
        }
        parent = here;
    }
    false
}
