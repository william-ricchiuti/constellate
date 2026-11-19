//! The vault reader: scans a folder (an Obsidian vault, other notes, or any folder) into
//! a [`Graph`] and keeps it current. See `docs/beyond-obsidian.md`.

mod detect;
mod folder;
mod formats;
mod graph;
mod guard;
mod kind;
mod parse;
mod sample;
mod scan;
mod watch;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

pub use detect::{Detection, detect};
pub use folder::file_type;
pub use formats::NoteExtra;
pub use graph::{Graph, GraphOptions, Link, Node, materialize, size_weight};
pub use guard::{Guard, MAX_FILES};
pub use kind::SourceKind;
pub use parse::{extract_tags, extract_wikilinks, note_excerpt};
pub use sample::sample_graph;
pub use scan::{Entry, has_hidden_segment, is_ignored, is_markdown, parse_entry};
pub use watch::{VaultWatcher, watch};

#[derive(Debug, Clone)]
pub struct VaultOptions {
    /// Folder names or vault-relative paths to leave out (`ignorePaths`).
    pub ignore_paths: Vec<String>,
    pub graph: GraphOptions,
    /// How to read the folder. `Auto` is resolved by [`detect`] when scanning.
    pub kind: SourceKind,
    /// Size nodes by bytes on disk instead of link count (`nodeSize: "fileSize"`).
    pub size_by_file: bool,
}

impl Default for VaultOptions {
    /// Obsidian, as the web version reads: what `vw-dump` and the parity test use.
    fn default() -> Self {
        VaultOptions {
            ignore_paths: Vec::new(),
            graph: GraphOptions::default(),
            kind: SourceKind::Obsidian,
            size_by_file: false,
        }
    }
}

/// Every note (or file) in the folder, keyed by path.
pub struct Vault {
    root: PathBuf,
    /// `opts.kind` is always resolved (never `Auto`).
    opts: VaultOptions,
    /// Why `opts.kind` was chosen: "set in settings" or what detection found.
    reason: String,
    guard: Guard,
    entries: HashMap<PathBuf, Entry>,
    /// The last walk stopped at the file cap.
    capped: bool,
}

impl Vault {
    /// Full scan of the folder, parsing notes in parallel. `Auto` is detected first.
    pub fn scan(root: impl Into<PathBuf>, opts: VaultOptions) -> Vault {
        Vault::scan_with(root, opts, Guard::default())
    }

    /// [`scan`](Self::scan) with a given guard (a smaller cap, in tests).
    pub fn scan_with(root: impl Into<PathBuf>, mut opts: VaultOptions, guard: Guard) -> Vault {
        let root = root.into();
        let reason = if opts.kind == SourceKind::Auto {
            let d = detect(&root);
            opts.kind = d.kind;
            d.reason
        } else {
            "set in settings".to_string()
        };
        let walk = scan::collect_files(&root, &root, &opts.ignore_paths, opts.kind, &guard, 0);
        if walk.capped {
            eprintln!("note: more than {} files in {}; showing the first ones", guard.max_files, root.display());
        }
        let entries = parse_all(opts.kind, &root, &walk.files).into_iter().map(|e| (e.path.clone(), e)).collect();
        Vault { root, opts, reason, guard, entries, capped: walk.capped }
    }

    /// The kind the folder is read as (never `Auto`).
    pub fn kind(&self) -> SourceKind {
        self.opts.kind
    }

    /// Why [`kind`](Self::kind) was chosen.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// The folder holds more files than the cap.
    pub fn capped(&self) -> bool {
        self.capped
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Paths of every note currently in the vault.
    pub fn paths(&self) -> impl Iterator<Item = &Path> {
        self.entries.keys().map(PathBuf::as_path)
    }

    pub fn graph(&self) -> Graph {
        let mut graph = match self.opts.kind {
            SourceKind::Obsidian | SourceKind::Auto => materialize(self.entries.values(), &self.root, &self.opts.graph),
            SourceKind::Folder => folder::materialize(self.entries.values(), &self.root, self.opts.size_by_file),
            kind => formats::materialize(kind, self.entries.values(), &self.root, &self.opts.graph),
        };
        if self.opts.size_by_file && self.opts.kind != SourceKind::Folder {
            apply_size_weights(&mut graph, self.entries.values());
        }
        graph.capped = self.capped;
        graph
    }

    fn wanted(&self, path: &Path) -> bool {
        let kind = self.opts.kind;
        let wanted_name = match kind {
            SourceKind::Obsidian | SourceKind::Auto => is_markdown(path),
            _ => path.file_name().and_then(|n| n.to_str()).is_some_and(|n| kind.wants_name(n)),
        };
        wanted_name
            && !has_hidden_segment(&self.root, path)
            && !is_ignored(&self.root, path, &self.opts.ignore_paths)
            && (!kind.guarded() || !scan::guard_skips(&self.root, path, &self.guard))
    }

    /// Room for `n` more files under the cap (always, for Obsidian).
    fn room(&self) -> bool {
        !self.opts.kind.guarded() || self.entries.len() < self.guard.max_files
    }

    /// Brings the entries for these paths up to date with the disk, whatever happened
    /// to them (created, edited, deleted, renamed, or a folder moved). Returns true when
    /// anything that affects the graph changed.
    pub fn refresh_paths(&mut self, paths: &[PathBuf]) -> bool {
        let mut changed = false;
        let mut reparse: Vec<PathBuf> = Vec::new();
        // macOS reports a moved folder by its new name only, so a folder event or a
        // vanished path means some notes we hold may be gone without their own event.
        let mut prune = false;
        for path in paths {
            if path.strip_prefix(&self.root).is_err() {
                continue;
            }
            match std::fs::symlink_metadata(path) {
                Ok(meta) if meta.is_dir() => {
                    // A folder appeared or was moved in: drop what we had under it and
                    // rescan it, unless the folder itself is hidden or ignored.
                    prune = true;
                    changed |= self.remove_under(path);
                    let hidden = has_hidden_segment(&self.root, path)
                        || is_ignored(&self.root, path, &self.opts.ignore_paths)
                        || (self.opts.kind.guarded() && scan::guard_skips(&self.root, path, &self.guard));
                    if !hidden {
                        let walk = scan::collect_files(
                            &self.root,
                            path,
                            &self.opts.ignore_paths,
                            self.opts.kind,
                            &self.guard,
                            self.entries.len() + reparse.len(),
                        );
                        if walk.capped && !self.capped {
                            self.capped = true;
                            changed = true;
                        }
                        reparse.extend(walk.files);
                    }
                }
                Ok(meta) if meta.is_file() => {
                    if self.wanted(path) {
                        reparse.push(path.clone());
                    }
                }
                // Gone (or a symlink, which is never followed): forget it and anything
                // that lived under it.
                _ => {
                    prune = true;
                    changed |= self.entries.remove(path).is_some();
                    changed |= self.remove_under(path);
                }
            }
        }
        if prune {
            changed |= self.prune_missing();
        }
        for entry in parse_all(self.opts.kind, &self.root, &reparse) {
            let known = self.entries.contains_key(&entry.path);
            if !known && !self.room() {
                // A new file past the cap: left out, as the full walk would.
                if !self.capped {
                    self.capped = true;
                    changed = true;
                }
                continue;
            }
            let differs = self.entries.get(&entry.path).is_none_or(|prev| *prev != entry);
            if differs {
                self.entries.insert(entry.path.clone(), entry);
                changed = true;
            }
        }
        changed
    }

    /// Full rescan, for when the OS says events were dropped.
    pub fn rescan(&mut self) -> bool {
        // Same kind as before: detection runs once per open, not on every rescan.
        let fresh = Vault::scan_with(self.root.clone(), self.opts.clone(), self.guard.clone());
        let changed = fresh.entries != self.entries || fresh.capped != self.capped;
        self.entries = fresh.entries;
        self.capped = fresh.capped;
        changed
    }

    /// Drops entries whose file no longer exists.
    fn prune_missing(&mut self) -> bool {
        let missing: Vec<PathBuf> =
            self.entries.par_iter().filter(|(p, _)| !p.is_file()).map(|(p, _)| p.clone()).collect();
        for p in &missing {
            self.entries.remove(p);
        }
        !missing.is_empty()
    }

    fn remove_under(&mut self, dir: &Path) -> bool {
        let before = self.entries.len();
        self.entries.retain(|p, _| !(p != dir && p.starts_with(dir)));
        self.entries.len() != before
    }
}

/// Reads many files of `kind` in parallel, dropping any that fail.
fn parse_all(kind: SourceKind, root: &Path, paths: &[PathBuf]) -> Vec<Entry> {
    match kind {
        SourceKind::Obsidian | SourceKind::Auto => scan::parse_entries(paths),
        SourceKind::Folder => paths.par_iter().filter_map(|p| folder::stat_entry(p)).collect(),
        kind => paths.par_iter().filter_map(|p| formats::parse(kind, root, p)).collect(),
    }
}

/// Sets each note's size weight from its file size. Relies on every reader putting the
/// notes first, in path order, before any ghost.
fn apply_size_weights<'a>(graph: &mut Graph, entries: impl IntoIterator<Item = &'a Entry>) {
    let mut sorted: Vec<&Entry> = entries.into_iter().collect();
    sorted.sort_unstable_by(|a, b| a.path.as_os_str().cmp(b.path.as_os_str()));
    for (node, e) in graph.nodes.iter_mut().zip(sorted) {
        if !node.ghost {
            node.weight = Some(size_weight(e.size));
        }
    }
}

#[cfg(test)]
mod tests;
