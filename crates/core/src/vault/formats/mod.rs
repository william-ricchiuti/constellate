//! Readers for the notes kinds other than Obsidian: `notes` (Markdown and org with
//! wikilinks, Markdown links and org links), `logseq` and `dendron`.
//!
//! SKELETON: parsing currently applies Obsidian's tag and wikilink rules to every note
//! and builds the graph with Obsidian's `materialize`. The format-specific rules are
//! filled in by the formats work (see `docs/beyond-obsidian.md`).

use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

use super::graph::{Graph, GraphOptions, materialize as materialize_obsidian};
use super::kind::{SourceKind, note_stem};
use super::parse::{extract_tags, extract_wikilinks};
use super::scan::Entry;

/// Notes over this size are read only up to it.
pub const MAX_NOTE_BYTES: u64 = 4 * 1024 * 1024;

/// What the other formats know about a note beyond tags and wikilinks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NoteExtra {
    /// Display name when it differs from the file name (`title::`, `#+title:`, Dendron's
    /// `title:`, a Logseq namespace page's full name).
    pub title: Option<String>,
    /// Other names links may use (`alias::`, `aliases:`, `#+roam_aliases:`).
    pub aliases: Vec<String>,
    /// Links by root-relative path, "/"-separated, without the note extension
    /// (Markdown links and org `file:` links, already resolved against the note's folder).
    pub path_links: Vec<String>,
    /// Ids this note answers to (org `:ID:`, Logseq block `id::`).
    pub ids: Vec<String>,
    /// Links by id (org `[[id:…]]`, Logseq `((uuid))`).
    pub id_links: Vec<String>,
}

/// Reads up to [`MAX_NOTE_BYTES`] of a note. `None` when it vanished or can't be read.
pub(crate) fn read_note(path: &Path) -> Option<(fs::Metadata, String)> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mut bytes = Vec::with_capacity(meta.len().min(MAX_NOTE_BYTES) as usize);
    match fs::File::open(path).and_then(|f| f.take(MAX_NOTE_BYTES).read_to_end(&mut bytes)) {
        Ok(_) => {}
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("note: skipped {} ({e})", path.display());
            }
            return None;
        }
    }
    Some((meta, String::from_utf8_lossy(&bytes).into_owned()))
}

pub(crate) fn mtime_ms(meta: &fs::Metadata) -> f64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0.0, |d| d.as_secs() as f64 * 1000.0 + f64::from(d.subsec_nanos()) / 1e6)
}

/// Reads and parses one note of `kind` inside `root`.
pub fn parse(kind: SourceKind, root: &Path, path: &Path) -> Option<Entry> {
    let _ = (kind, root);
    let (meta, content) = read_note(path)?;
    let name = path.file_name()?.to_str()?;
    Some(Entry {
        path: path.to_path_buf(),
        basename: note_stem(name).to_string(),
        mtime_ms: mtime_ms(&meta),
        tags: extract_tags(&content),
        wikilinks: extract_wikilinks(&content),
        size: meta.len(),
        extra: None,
    })
}

/// Builds the graph for notes of `kind`.
pub fn materialize<'a>(kind: SourceKind, entries: impl IntoIterator<Item = &'a Entry>, root: &Path, opts: &GraphOptions) -> Graph {
    let _ = kind;
    materialize_obsidian(entries, root, opts)
}

/// Roughly how many links a note's text holds, in any supported syntax. Used by
/// detection to tell linked notes from plain Markdown.
pub fn count_links(content: &str) -> usize {
    extract_wikilinks(content).len()
}
