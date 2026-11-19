//! Readers for the notes kinds other than Obsidian: `notes` (Markdown and org with
//! wikilinks, Markdown links and org links: Foam, Zettlr, org-roam, plain Markdown),
//! `logseq` and `dendron`. See `docs/beyond-obsidian.md`.
//!
//! Obsidian vaults never come through here; their reading stays in `scan.rs` and
//! `graph.rs`, byte for byte as the web version's.

mod dendron;
mod logseq;
mod markdown;
mod org;

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use super::graph::{Graph, GraphOptions, Link, Node};
use super::kind::{SourceKind, note_stem};
use super::parse::{extract_tags, extract_wikilinks};
use super::scan::Entry;

/// Notes over this size are read only up to it.
pub const MAX_NOTE_BYTES: u64 = 4 * 1024 * 1024;

/// What the other formats know about a note beyond tags and wikilinks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NoteExtra {
    /// Display name when it differs from the file name (`title::`, `#+title:`, Dendron's
    /// `title:`, a Logseq page's name).
    pub title: Option<String>,
    /// Other names links may use (`alias::`, `aliases:`, `#+roam_aliases:`).
    pub aliases: Vec<String>,
    /// Links by root-relative path, "/"-separated, without the note extension
    /// (Markdown links and org `file:` links, already resolved against the note's folder).
    /// A missing target becomes a ghost.
    pub path_links: Vec<String>,
    /// Like `path_links`, but written without an extension (`[x](other)`): they count only
    /// when a note is there.
    pub soft_path_links: Vec<String>,
    /// Ids this note answers to (org `:ID:`, Logseq block `id::`).
    pub ids: Vec<String>,
    /// Links by id (org `[[id:…]]`, Logseq `((uuid))`).
    pub id_links: Vec<String>,
    /// The Dendron vault the note is in (its name, for `dendron://vault/…` links).
    pub vault: Option<String>,
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

/// The path below the root, "/"-separated.
fn rel_slash(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?.to_str()?;
    if rel.is_empty() {
        return None;
    }
    Some(if std::path::MAIN_SEPARATOR == '/' { rel.to_string() } else { rel.replace(std::path::MAIN_SEPARATOR, "/") })
}

fn dir_of(rel: &str) -> &str {
    rel.rfind('/').map_or("", |i| &rel[..i])
}

fn is_org(name: &str) -> bool {
    name.len() > 4 && name[name.len() - 4..].eq_ignore_ascii_case(".org")
}

/// Reads and parses one note of `kind` inside `root`. `None` when the file isn't part of
/// a graph of that kind (a Logseq file outside `pages/` and `journals/`, a Dendron file
/// outside its vaults), or can't be read.
pub fn parse(kind: SourceKind, root: &Path, path: &Path) -> Option<Entry> {
    let rel = rel_slash(root, path)?;
    let name = path.file_name()?.to_str()?;
    let stem = note_stem(name);
    let dir = dir_of(&rel);
    // Decide before reading, so files that don't count are never opened.
    let vault = match kind {
        SourceKind::Logseq => {
            let top = rel.split('/').next().unwrap_or("");
            if !(top == "pages" || top == "journals") || !rel.contains('/') {
                return None;
            }
            None
        }
        SourceKind::Dendron => Some(dendron::vaults(root).into_iter().find(|v| v.fs_path == dir)?.name),
        _ => None,
    };
    let (meta, content) = read_note(path)?;
    let mut extra = NoteExtra { vault, ..NoteExtra::default() };
    let (tags, wikilinks) = match kind {
        SourceKind::Logseq => {
            let page = logseq::parse(&content);
            let file_name = logseq::page_name(stem);
            let journal = (rel.starts_with("journals/")).then(|| logseq::journal_date(stem)).flatten();
            let mut aliases = Vec::new();
            let name = match (&page.title, journal) {
                (Some(t), _) => {
                    aliases.push(file_name);
                    t.clone()
                }
                (None, Some(date)) => {
                    let [label, long] = logseq::journal_names(date);
                    aliases.push(long);
                    label
                }
                (None, None) => file_name,
            };
            if let (Some(_), Some(date)) = (&page.title, journal) {
                aliases.extend(logseq::journal_names(date));
            }
            aliases.extend(page.aliases);
            extra.title = Some(name);
            extra.aliases = aliases;
            extra.ids = page.ids;
            extra.id_links = page.block_refs;
            (page.tags, page.links)
        }
        SourceKind::Dendron => {
            let text = markdown::blank_code(&content);
            extra.title = markdown::frontmatter(&content).and_then(|fm| markdown::fm_values(fm, "title").into_iter().next());
            let links = dendron::extract_links(&text)
                .into_iter()
                .map(|t| match t.vault {
                    Some(v) => format!("dendron://{v}/{}", t.name),
                    None => t.name,
                })
                .collect();
            (extract_tags(&text), links)
        }
        _ if is_org(name) => {
            let o = org::parse(&content, dir);
            extra.title = o.title;
            extra.aliases = o.aliases;
            extra.path_links = o.path_links.hard;
            extra.soft_path_links = o.path_links.soft;
            extra.ids = o.ids;
            extra.id_links = o.id_links;
            (o.tags, o.wikilinks.iter().map(|w| note_stem(w).to_string()).collect())
        }
        _ => {
            let text = markdown::blank_code(&content);
            if let Some(fm) = markdown::frontmatter(&content) {
                extra.title = markdown::fm_values(fm, "title").into_iter().next();
                extra.aliases = markdown::fm_values(fm, "aliases");
                extra.aliases.extend(markdown::fm_values(fm, "alias"));
            }
            let links = markdown::extract_path_links(&text, dir);
            extra.path_links = links.hard;
            extra.soft_path_links = links.soft;
            let wikilinks = extract_wikilinks(&text).iter().map(|w| note_stem(w).to_string()).collect();
            (extract_tags(&text), wikilinks)
        }
    };
    Some(Entry {
        path: path.to_path_buf(),
        basename: stem.to_string(),
        mtime_ms: mtime_ms(&meta),
        tags,
        wikilinks,
        size: meta.len(),
        extra: Some(Box::new(extra)),
    })
}

/// Roughly how many links a note's text holds, in any supported syntax (wikilinks,
/// Markdown links and reference definitions to notes, org links). Used by detection to
/// tell linked notes from plain Markdown.
pub fn count_links(content: &str) -> usize {
    // `[[…]]` and `[[…][…]]` in every flavour; org's counter skips URLs and internal targets.
    let text = markdown::blank_code(content);
    org::count_links(&text) + markdown::extract_path_links(&text, "").len()
}

/// Lowercased lookups from names to note indices.
#[derive(Default)]
struct Index {
    /// File-derived names, exact case.
    exact: HashMap<String, Vec<u32>>,
    /// File-derived names (and titles where they name the note), lowercased.
    lower: HashMap<String, Vec<u32>>,
    /// Aliases, lowercased.
    alias: HashMap<String, Vec<u32>>,
    /// Root-relative paths without the note extension, lowercased.
    path: HashMap<String, u32>,
    /// Org and block ids, lowercased.
    ident: HashMap<String, u32>,
}

impl Index {
    fn add(map: &mut HashMap<String, Vec<u32>>, key: String, i: u32) {
        let v = map.entry(key).or_default();
        if !v.contains(&i) {
            v.push(i);
        }
    }
}

/// Builds nodes and links, with ghosts made on demand.
struct Builder {
    nodes: Vec<Node>,
    by_id: HashMap<String, u32>,
    ghosts: HashMap<String, u32>,
    pairs: Vec<(u32, u32)>,
    seen: HashSet<(u32, u32)>,
    show_ghosts: bool,
}

impl Builder {
    fn link(&mut self, s: u32, t: u32) {
        if s != t && self.seen.insert((s, t)) {
            self.pairs.push((s, t));
        }
    }

    /// The node standing for a missing note `name` (matched case-insensitively), made on
    /// first use. `None` when unresolved links are hidden.
    fn ghost(&mut self, name: &str) -> Option<u32> {
        let name = name.trim();
        if !self.show_ghosts || name.is_empty() {
            return None;
        }
        let key = name.to_lowercase();
        if let Some(&g) = self.ghosts.get(&key) {
            return Some(g);
        }
        // A ghost named like an existing node's id is that node.
        if let Some(&i) = self.by_id.get(name) {
            return Some(i);
        }
        let g = self.nodes.len() as u32;
        self.nodes.push(Node { id: name.to_string(), ghost: true, ..Node::default() });
        self.by_id.insert(name.to_string(), g);
        self.ghosts.insert(key, g);
        Some(g)
    }

    /// Links `source` up a hierarchy (`parents`, nearest first, then `top` when given):
    /// to the nearest parent that exists, through ghosts for the missing ones when ghosts
    /// are shown.
    fn hierarchy<'p>(
        &mut self,
        source: u32,
        parents: impl Iterator<Item = &'p str>,
        top: Option<u32>,
        find: impl Fn(&str) -> Option<u32>,
    ) {
        let mut child = source;
        for p in parents {
            if let Some(i) = find(p) {
                self.link(child, i);
                return;
            }
            if let Some(g) = self.ghost(p) {
                self.link(child, g);
                child = g;
            }
        }
        if let Some(t) = top {
            self.link(child, t);
        }
    }

    fn finish(self, tagged: usize, discovered_tags: BTreeMap<String, usize>) -> Graph {
        let links = self
            .pairs
            .into_iter()
            .map(|(s, t)| Link { source: self.nodes[s as usize].id.clone(), target: self.nodes[t as usize].id.clone() })
            .collect();
        Graph { nodes: self.nodes, links, tagged, discovered_tags, capped: false }
    }
}

/// Unique node ids: each note's first candidate, falling back to the next candidate for
/// every note whose current one is shared.
fn assign_ids(candidates: Vec<Vec<String>>) -> Vec<String> {
    let mut level = vec![0usize; candidates.len()];
    loop {
        let ids: Vec<&str> =
            candidates.iter().zip(&level).map(|(c, &l)| c[l.min(c.len() - 1)].as_str()).collect();
        let mut counts: HashMap<&str, usize> = HashMap::default();
        for id in &ids {
            *counts.entry(id).or_default() += 1;
        }
        let mut bumped = false;
        for (i, id) in ids.iter().enumerate() {
            if counts[id] > 1 && level[i] + 1 < candidates[i].len() {
                level[i] += 1;
                bumped = true;
            }
        }
        if !bumped {
            return ids.into_iter().map(str::to_string).collect();
        }
    }
}

/// Builds the graph for notes of `kind`. Notes come first, in path order, then ghosts.
pub fn materialize<'a>(kind: SourceKind, entries: impl IntoIterator<Item = &'a Entry>, root: &Path, opts: &GraphOptions) -> Graph {
    let mut raw: Vec<&Entry> = entries.into_iter().collect();
    raw.sort_unstable_by(|a, b| a.path.as_os_str().cmp(b.path.as_os_str()));
    let empty = NoteExtra::default();
    let extra = |e: &'a Entry| -> &NoteExtra { e.extra.as_deref().unwrap_or(&empty) };
    let rels: Vec<String> = raw.iter().map(|e| rel_slash(root, &e.path).unwrap_or_else(|| e.basename.clone())).collect();
    let keys: Vec<String> = rels.iter().map(|r| format!("{}{}", dir_prefix(r), note_stem(file_part(r)))).collect();

    // The name each note goes by (and displays), and its id candidates.
    let names: Vec<String> = raw
        .iter()
        .map(|e| match kind {
            SourceKind::Logseq => extra(e).title.clone().unwrap_or_else(|| e.basename.clone()),
            _ => e.basename.clone(),
        })
        .collect();
    let candidates: Vec<Vec<String>> = names
        .iter()
        .zip(&keys)
        .zip(&rels)
        .map(|((n, k), r)| {
            let q = |s: &str| if s.contains('/') { s.to_string() } else { format!("./{s}") };
            vec![n.clone(), q(k), q(r)]
        })
        .collect();
    let ids = assign_ids(candidates);

    let mut index = Index::default();
    let mut b = Builder {
        nodes: Vec::with_capacity(raw.len()),
        by_id: HashMap::default(),
        ghosts: HashMap::default(),
        pairs: Vec::new(),
        seen: HashSet::default(),
        show_ghosts: opts.show_unresolved_links,
    };
    let mut tagged = 0;
    let mut discovered_tags: BTreeMap<String, usize> = BTreeMap::new();
    for (i, e) in raw.iter().enumerate() {
        let ix = i as u32;
        let x = extra(e);
        Index::add(&mut index.exact, names[i].clone(), ix);
        Index::add(&mut index.lower, names[i].to_lowercase(), ix);
        if kind == SourceKind::Notes && is_org(file_part(&rels[i])) {
            // org-roam names a note by its title.
            if let Some(t) = &x.title {
                Index::add(&mut index.lower, t.to_lowercase(), ix);
            }
        }
        for a in &x.aliases {
            Index::add(&mut index.alias, a.to_lowercase(), ix);
        }
        index.path.entry(keys[i].to_lowercase()).or_insert(ix);
        for id in &x.ids {
            index.ident.entry(id.to_lowercase()).or_insert(ix);
        }
        if !e.tags.is_empty() {
            tagged += 1;
            for t in &e.tags {
                *discovered_tags.entry(t.clone()).or_default() += 1;
            }
        }
        let display = match kind {
            SourceKind::Logseq => names[i].clone(),
            _ => x.title.clone().unwrap_or_else(|| e.basename.clone()),
        };
        b.by_id.insert(ids[i].clone(), ix);
        b.nodes.push(Node {
            label: (display != ids[i]).then_some(display),
            id: ids[i].clone(),
            tag: e.tags.first().cloned(),
            tags: (e.tags.len() > 1).then(|| e.tags.clone()),
            mtime: (e.mtime_ms != 0.0).then_some(e.mtime_ms),
            folder: rels[i].split_once('/').map(|(top, _)| top.to_string()),
            ghost: false,
            weight: None,
        });
    }

    let by_name = |name: &str| -> Option<&[u32]> {
        let lower = name.to_lowercase();
        index.lower.get(&lower).or_else(|| index.alias.get(&lower)).map(Vec::as_slice)
    };
    // Obsidian's order for wikilinks: exact name, path, last segment, any case, then aliases.
    let resolve_wikilink = |target: &str| -> Option<&[u32]> {
        if let Some(v) = index.exact.get(target) {
            return Some(v);
        }
        if target.contains('/') {
            if let Some(i) = index.path.get(&target.trim_start_matches('/').to_lowercase()) {
                return Some(std::slice::from_ref(i));
            }
            let last = &target[target.rfind('/').unwrap() + 1..];
            if let Some(v) = index.exact.get(last).or_else(|| index.lower.get(&last.to_lowercase())) {
                return Some(v);
            }
        }
        by_name(target)
    };
    let vault_of = |i: u32| extra(raw[i as usize]).vault.as_deref();

    for (s, e) in raw.iter().enumerate() {
        let s = s as u32;
        let x = extra(e);
        match kind {
            SourceKind::Logseq => {
                for target in &e.wikilinks {
                    match by_name(target) {
                        Some(v) => v.iter().for_each(|&t| b.link(s, t)),
                        None => {
                            if let Some(g) = b.ghost(target) {
                                b.link(s, g);
                            }
                        }
                    }
                }
                let name = &names[s as usize];
                let find = |p: &str| by_name(p).and_then(|v| v.first().copied());
                b.hierarchy(s, logseq::namespace_parents(name), None, find);
            }
            SourceKind::Dendron => {
                let here = vault_of(s);
                for target in &e.wikilinks {
                    let (vault, name) = match target.strip_prefix("dendron://").and_then(|r| r.split_once('/')) {
                        Some(("$this", n)) => (here, n),
                        Some((v, n)) => (Some(v), n),
                        None => (None, target.as_str()),
                    };
                    let found = by_name(name).and_then(|v| {
                        let pick = |want: Option<&str>| v.iter().copied().find(|&i| want.is_some() && vault_of(i) == want);
                        pick(vault).or_else(|| pick(here)).or_else(|| v.first().copied())
                    });
                    match found {
                        Some(t) => b.link(s, t),
                        None => {
                            if let Some(g) = b.ghost(name) {
                                b.link(s, g);
                            }
                        }
                    }
                }
                // Hierarchy, within the note's own vault.
                let name = &names[s as usize];
                let find = |p: &str| index.lower.get(&p.to_lowercase())?.iter().copied().find(|&i| vault_of(i) == here);
                let top = if name.eq_ignore_ascii_case("root") { None } else { find("root") };
                b.hierarchy(s, dendron::parents(name), top, find);
            }
            _ => {
                for target in &e.wikilinks {
                    match resolve_wikilink(target) {
                        Some(v) => v.iter().for_each(|&t| b.link(s, t)),
                        None => {
                            if let Some(g) = b.ghost(target) {
                                b.link(s, g);
                            }
                        }
                    }
                }
                for p in &x.path_links {
                    match index.path.get(&p.to_lowercase()) {
                        Some(&t) => b.link(s, t),
                        None => {
                            if let Some(g) = b.ghost(file_part(p)) {
                                b.link(s, g);
                            }
                        }
                    }
                }
                for p in &x.soft_path_links {
                    if let Some(&t) = index.path.get(&p.to_lowercase()) {
                        b.link(s, t);
                    }
                }
            }
        }
        for id in &x.id_links {
            if let Some(&t) = index.ident.get(&id.to_lowercase()) {
                b.link(s, t);
            }
        }
    }
    b.finish(tagged, discovered_tags)
}

fn file_part(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

fn dir_prefix(rel: &str) -> &str {
    rel.rfind('/').map_or("", |i| &rel[..=i])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    fn graph(kind: SourceKind, root: &Path, rels: &[&str], ghosts: bool) -> Graph {
        let entries: Vec<Entry> = rels.iter().filter_map(|r| parse(kind, root, &root.join(r))).collect();
        materialize(kind, &entries, root, &GraphOptions { show_unresolved_links: ghosts })
    }

    fn links(g: &Graph) -> Vec<(String, String)> {
        g.links.iter().map(|l| (l.source.clone(), l.target.clone())).collect()
    }

    fn pair(s: &str, t: &str) -> (String, String) {
        (s.to_string(), t.to_string())
    }

    #[test]
    fn ids_fall_back_until_unique() {
        let c = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let ids = assign_ids(vec![c(&["a", "./a", "./a.md"]), c(&["a", "./a", "./a.org"]), c(&["a", "x/a", "x/a.md"]), c(&["b", "./b", "./b.md"])]);
        assert_eq!(ids, ["./a.md", "./a.org", "x/a", "b"]);
    }

    #[test]
    fn notes_markdown_and_org() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        write(r, "a.md", "---\ntitle: Alpha\naliases: [first]\ntags: [t1]\n---\n[[b]] [c](sub/c.md) [m](missing.md) [s](nowhere) [[ghost]] #t2");
        write(r, "b.md", "[[First]] [up](./a) `[[ignored]]`");
        write(r, "sub/c.md", "[back](../a.md) [[sub/d]]");
        write(r, "sub/d.org", ":PROPERTIES:\n:ID: d-id\n:END:\n#+title: Delta\n#+filetags: :o:\n[[file:../a.md][A]] [[id:e-id]]");
        write(r, "e.org", ":PROPERTIES:\n:ID: e-id\n:END:\n[[roam:Delta]]");
        let rels = ["a.md", "b.md", "sub/c.md", "sub/d.org", "e.org"];
        let g = graph(SourceKind::Notes, r, &rels, true);
        let ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "e", "c", "d", "ghost", "missing"]);
        assert_eq!(g.nodes[0].label.as_deref(), Some("Alpha"));
        assert_eq!(g.nodes[0].tags.as_deref(), Some(&["t1".to_string(), "t2".to_string()][..]));
        assert_eq!(g.nodes[4].label.as_deref(), Some("Delta"));
        assert_eq!(g.nodes[4].folder.as_deref(), Some("sub"));
        assert!(g.nodes[5].ghost && g.nodes[6].ghost);
        assert_eq!(
            links(&g),
            [
                pair("a", "b"),
                pair("a", "ghost"),
                pair("a", "c"),
                pair("a", "missing"),
                pair("b", "a"),
                pair("e", "d"),
                pair("c", "d"),
                pair("c", "a"),
                pair("d", "a"),
                pair("d", "e"),
            ]
        );
        assert_eq!(g.tagged, 2);
        let g = graph(SourceKind::Notes, r, &rels, false);
        assert_eq!(g.nodes.len(), 5);
        assert_eq!(g.links.len(), 8);
    }

    #[test]
    fn duplicate_names_across_extensions_and_folders() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        write(r, "n.md", "[[n]]");
        write(r, "n.org", "x");
        write(r, "x/n.md", "[[x/n]]");
        let g = graph(SourceKind::Notes, r, &["n.md", "n.org", "x/n.md"], true);
        let ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["./n.md", "./n.org", "x/n"]);
        assert!(g.nodes.iter().all(|n| n.label.as_deref() == Some("n")));
        // [[n]] matches every note named n (as Obsidian), minus itself.
        assert_eq!(links(&g), [pair("./n.md", "./n.org"), pair("./n.md", "x/n")]);
    }

    #[test]
    fn logseq_pages_namespaces_and_refs() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        write(r, "pages/proj___web___ui.md", "tags:: work\n\n- see [[Proj]] and ((aaaaaaaa-0000-0000-0000-000000000001))");
        write(r, "pages/proj.md", "alias:: P\n\n- top\n  id:: aaaaaaaa-0000-0000-0000-000000000001");
        write(r, "pages/Work.md", "- #proj/web/ui");
        write(r, "journals/2024_01_31.md", "- met [[p]] #[[New Page]]");
        write(r, "pages/old%2Fname.md", "- [[Jan 31st, 2024]]");
        write(r, "logseq/bak/pages/proj.md", "- [[x]]");
        let rels = ["pages/proj___web___ui.md", "pages/proj.md", "pages/Work.md", "journals/2024_01_31.md", "pages/old%2Fname.md", "logseq/bak/pages/proj.md"];
        assert!(parse(SourceKind::Logseq, r, &r.join("logseq/bak/pages/proj.md")).is_none());
        let g = graph(SourceKind::Logseq, r, &rels, true);
        let ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["2024-01-31", "Work", "old/name", "proj", "proj/web/ui", "New Page", "old", "proj/web"]);
        assert_eq!(g.nodes[4].tag.as_deref(), Some("work"));
        assert_eq!(
            links(&g),
            [
                pair("2024-01-31", "proj"),
                pair("2024-01-31", "New Page"),
                pair("Work", "proj/web/ui"),
                pair("old/name", "2024-01-31"),
                pair("old/name", "old"),
                pair("proj/web/ui", "Work"),
                pair("proj/web/ui", "proj"),
                pair("proj/web/ui", "proj/web"),
                pair("proj/web", "proj"),
            ]
        );
        // Without ghosts, a namespace page links to its nearest existing ancestor.
        let g = graph(SourceKind::Logseq, r, &rels, false);
        assert_eq!(g.nodes.len(), 5);
        assert_eq!(links(&g).len(), 5);
        assert!(links(&g).contains(&pair("proj/web/ui", "proj")));
    }

    #[test]
    fn dendron_hierarchy_and_vaults() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        write(r, "dendron.yml", "workspace:\n  vaults:\n    - fsPath: main\n    - fsPath: other\n      name: o\n");
        write(r, "main/root.md", "---\ntitle: Root\n---\n");
        write(r, "main/a.md", "---\ntitle: A\n---\n");
        write(r, "main/a.b.c.md", "---\ntitle: C\ntags: [x]\n---\n[[see|other.note]] [[dendron://o/root]] #y");
        write(r, "other/root.md", "");
        write(r, "other/other.note.md", "[[a.b.c#sec]]");
        write(r, "main/assets/a.md", "not a note");
        write(r, "loose.md", "not in a vault");
        let rels = ["main/root.md", "main/a.md", "main/a.b.c.md", "other/root.md", "other/other.note.md", "main/assets/a.md", "loose.md"];
        let g = graph(SourceKind::Dendron, r, &rels, true);
        let ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["a.b.c", "a", "main/root", "other.note", "other/root", "a.b", "other"]);
        assert_eq!(g.nodes[0].label.as_deref(), Some("C"));
        assert_eq!(g.nodes[0].tags.as_deref(), Some(&["x".to_string(), "y".to_string()][..]));
        assert_eq!(g.nodes[2].label.as_deref(), Some("Root"));
        assert_eq!(
            links(&g),
            [
                pair("a.b.c", "other.note"),
                pair("a.b.c", "other/root"),
                pair("a.b.c", "a.b"),
                pair("a.b", "a"),
                pair("a", "main/root"),
                pair("other.note", "a.b.c"),
                pair("other.note", "other"),
                pair("other", "other/root"),
            ]
        );
        // Without ghosts, a note links to its nearest existing ancestor (or the root).
        let g = graph(SourceKind::Dendron, r, &rels, false);
        assert_eq!(g.nodes.len(), 5);
        assert_eq!(
            links(&g),
            [
                pair("a.b.c", "other.note"),
                pair("a.b.c", "other/root"),
                pair("a.b.c", "a"),
                pair("a", "main/root"),
                pair("other.note", "a.b.c"),
                pair("other.note", "other/root"),
            ]
        );
    }

    #[test]
    fn counts_links_in_every_syntax() {
        assert_eq!(count_links("[[a]] [b](b.md)\n[c]: c.md\n[[file:d.org][D]] [[id:x]] [w](https://w.org) ![i](i.png)"), 5);
        assert_eq!(count_links("plain text with a [web link](https://example.com)"), 0);
        assert_eq!(count_links("```\n[[in code]]\n```"), 0);
    }
}
