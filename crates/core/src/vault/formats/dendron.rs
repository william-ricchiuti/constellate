//! Dendron workspaces: vault folders from `dendron.yml`, dotted hierarchies
//! (`a.b.c` → `a.b` → `a` → `root`) and wikilinks in Dendron's `[[label|target]]` order.

use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use regex::Regex;
use rustc_hash::FxHashMap as HashMap;

use super::markdown::unquote;

/// One vault of a workspace: its folder (root-relative, "/"-separated, "" for the root)
/// and its name for `dendron://name/…` links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultDir {
    pub fs_path: String,
    pub name: String,
}

/// The vaults listed in a `dendron.yml` (`vaults:` or `workspace: vaults:` lists of
/// `fsPath:` items). Read line by line, not as full YAML.
pub fn parse_config(yml: &str) -> Vec<VaultDir> {
    let indent = |l: &str| l.len() - l.trim_start().len();
    let mut out = Vec::new();
    let mut in_vaults: Option<usize> = None;
    let mut item: Option<(usize, Option<String>, Option<String>)> = None;
    let flush = |item: &mut Option<(usize, Option<String>, Option<String>)>, out: &mut Vec<VaultDir>| {
        if let Some((_, Some(fs), name)) = item.take() {
            let fs_path = fs.trim_matches('/').trim_start_matches("./").to_string();
            let fs_path = if fs_path == "." { String::new() } else { fs_path };
            let name = name.unwrap_or_else(|| fs_path.rsplit('/').next().unwrap_or("").to_string());
            if !out.iter().any(|v: &VaultDir| v.fs_path == fs_path) {
                out.push(VaultDir { fs_path, name });
            }
        }
    };
    for line in yml.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let ind = indent(line);
        if let Some(base) = in_vaults {
            let is_item = trimmed.starts_with("- ");
            if ind < base || (ind == base && !is_item) {
                flush(&mut item, &mut out);
                in_vaults = None;
            } else {
                let mut kv = trimmed;
                if is_item {
                    flush(&mut item, &mut out);
                    item = Some((ind, None, None));
                    kv = trimmed[2..].trim_start();
                }
                // Only keys of the item itself, not of nested maps (`remote: url:`).
                let item_key_indent = item.as_ref().map_or(0, |i| i.0 + 2);
                if let Some(it) = item.as_mut()
                    && (is_item || ind == item_key_indent)
                    && let Some((k, v)) = kv.split_once(':')
                {
                    match k.trim() {
                        "fsPath" if it.1.is_none() => it.1 = Some(unquote(v)),
                        "name" if it.2.is_none() => it.2 = Some(unquote(v)),
                        _ => {}
                    }
                }
                continue;
            }
        }
        if trimmed == "vaults:" {
            in_vaults = Some(ind);
        }
    }
    flush(&mut item, &mut out);
    out
}

type Cached = (Option<SystemTime>, Vec<VaultDir>);
static CONFIGS: LazyLock<Mutex<HashMap<PathBuf, Cached>>> = LazyLock::new(Default::default);

/// The vaults of the workspace at `root` (the root itself when `dendron.yml` lists
/// none). Cached per root until `dendron.yml` changes.
pub fn vaults(root: &Path) -> Vec<VaultDir> {
    let yml = root.join("dendron.yml");
    let mtime = std::fs::metadata(&yml).and_then(|m| m.modified()).ok();
    if let Some((t, v)) = CONFIGS.lock().ok().and_then(|c| c.get(root).cloned())
        && t == mtime
    {
        return v;
    }
    let mut v = std::fs::read_to_string(&yml).map(|s| parse_config(&s)).unwrap_or_default();
    if v.is_empty() {
        v.push(VaultDir { fs_path: String::new(), name: String::new() });
    }
    for vault in &mut v {
        if vault.name.is_empty() {
            vault.name = root_name(root);
        }
    }
    if let Ok(mut c) = CONFIGS.lock() {
        c.insert(root.to_path_buf(), (mtime, v.clone()));
    }
    v
}

fn root_name(root: &Path) -> String {
    root.file_name().and_then(|n| n.to_str()).unwrap_or("vault").to_string()
}

static WIKILINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!?\[\[([^\[\]]+)\]\]").expect("valid regex"));
static NON_NOTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\.(png|jpe?g|gif|webp|svg|bmp|avif|heic|pdf|mp3|mp4|m4a|wav|ogg|webm|mov|csv|zip)$")
        .expect("valid regex")
});

/// A link target: the note name, and the vault when the link names one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub vault: Option<String>,
    pub name: String,
}

/// Wikilinks and note references in Dendron's syntax, in order: `[[target]]`,
/// `[[label|target]]`, `[[target#anchor]]`, `![[target]]`, `[[dendron://vault/target]]`.
pub fn extract_links(content: &str) -> Vec<Target> {
    let mut out = Vec::new();
    for m in WIKILINK.captures_iter(content) {
        let inner = &m[1];
        let target = inner.rsplit('|').next().unwrap_or(inner);
        let target = target.split(['#', '^']).next().unwrap_or("").trim();
        let (vault, name) = match target.strip_prefix("dendron://") {
            Some(rest) => match rest.split_once('/') {
                Some((v, n)) => (Some(v.to_string()), n),
                None => (None, rest),
            },
            None => (None, target),
        };
        let name = name.trim();
        let name = name.strip_suffix(".md").unwrap_or(name);
        if name.is_empty() || NON_NOTE.is_match(name) || name.contains("://") {
            continue;
        }
        out.push(Target { vault, name: name.to_string() });
    }
    out
}

/// The hierarchy parents of a dotted name, nearest first: `a.b.c` → `a.b`, `a`.
pub fn parents(name: &str) -> impl Iterator<Item = &str> {
    name.char_indices().rev().filter(|&(_, c)| c == '.').map(move |(i, _)| &name[..i]).filter(|p| !p.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_vaults() {
        let yml = "\
version: 5
workspace:
  vaults:
    - fsPath: notes
      name: main
      remote:
        type: git
        name: not-this
    - fsPath: \"./other/\"
    -   fsPath: deep/nested
  journal:
    name: journal
dev:
  vaults:
  - fsPath: '.'
";
        let v = parse_config(yml);
        assert_eq!(
            v,
            [
                VaultDir { fs_path: "notes".into(), name: "main".into() },
                VaultDir { fs_path: "other".into(), name: "other".into() },
                VaultDir { fs_path: "deep/nested".into(), name: "nested".into() },
                VaultDir { fs_path: "".into(), name: "".into() },
            ]
        );
        assert!(parse_config("version: 5\n").is_empty());
    }

    #[test]
    fn links_in_dendron_order() {
        let text = "[[a.b]] [[Label|proj.x]] [[c.d#anchor]] ![[e.f]] [[dendron://other/g.h]] [[dendron://$this/i]] \
                    [[pic.png]] [[j.md]] [[https://x.com]] [[ |  ]]";
        let got = extract_links(text);
        let names: Vec<(Option<&str>, &str)> = got.iter().map(|t| (t.vault.as_deref(), t.name.as_str())).collect();
        assert_eq!(
            names,
            [(None, "a.b"), (None, "proj.x"), (None, "c.d"), (None, "e.f"), (Some("other"), "g.h"), (Some("$this"), "i"), (None, "j")]
        );
    }

    #[test]
    fn hierarchy() {
        assert_eq!(parents("a.b.c").collect::<Vec<_>>(), ["a.b", "a"]);
        assert_eq!(parents("root").count(), 0);
    }
}
