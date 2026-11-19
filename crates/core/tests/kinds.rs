//! Each fixture folder, read as its kind and with detection: exact node and link counts.

mod fixtures;

use std::path::Path;

use vw_core::vault::{Graph, GraphOptions, Guard, SourceKind, Vault, VaultOptions};

fn opts(kind: SourceKind, ghosts: bool) -> VaultOptions {
    VaultOptions { kind, graph: GraphOptions { show_unresolved_links: ghosts }, ..Default::default() }
}

fn scan(dir: &Path, kind: SourceKind, ghosts: bool) -> (Vault, Graph) {
    let vault = Vault::scan(dir, opts(kind, ghosts));
    let graph = vault.graph();
    (vault, graph)
}

/// Asserts the counts as `kind`, with and without ghosts, and that detection picks `kind`
/// and gives the same graph.
fn check(build: fn(&Path), kind: SourceKind, notes: usize, with: (usize, usize), without: (usize, usize)) -> Graph {
    let dir = tempfile::tempdir().unwrap();
    build(dir.path());
    let (vault, g) = scan(dir.path(), kind, true);
    assert_eq!(vault.len(), notes, "{kind} notes");
    assert_eq!((g.nodes.len(), g.links.len()), with, "{kind} nodes and links with ghosts");
    assert_unique_ids(&g);
    assert!(!g.capped);
    let (_, g2) = scan(dir.path(), kind, false);
    assert_eq!((g2.nodes.len(), g2.links.len()), without, "{kind} nodes and links without ghosts");
    assert!(g2.nodes.iter().all(|n| !n.ghost));
    let (auto, ga) = scan(dir.path(), SourceKind::Auto, true);
    assert_eq!(auto.kind(), kind, "detected kind ({})", auto.reason());
    assert_eq!(ga, g);
    g
}

fn assert_unique_ids(g: &Graph) {
    let mut ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
    ids.sort_unstable();
    let len = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), len, "node ids are unique");
    for l in &g.links {
        assert!(g.nodes.iter().any(|n| n.id == l.source) && g.nodes.iter().any(|n| n.id == l.target), "{l:?}");
    }
}

fn node<'a>(g: &'a Graph, id: &str) -> &'a vw_core::vault::Node {
    g.nodes.iter().find(|n| n.id == id).unwrap_or_else(|| panic!("no node {id:?}"))
}

fn linked(g: &Graph, s: &str, t: &str) -> bool {
    g.links.iter().any(|l| l.source == s && l.target == t)
}

#[test]
fn logseq() {
    let g = check(fixtures::logseq, SourceKind::Logseq, 10, (13, 15), (10, 12));
    // Only pages/ and journals/: nothing from logseq/bak.
    assert!(g.nodes.iter().all(|n| !n.id.contains("Backup") && !n.id.contains("Desktop")));
    assert!(linked(&g, "Projects/Web/Frontend", "Projects/Web") && linked(&g, "Projects/Web", "Projects"));
    assert!(linked(&g, "Recipes/Pasta", "Recipes") && node(&g, "Recipes").ghost);
    assert!(linked(&g, "2024-02-01", "2024-01-31"));
    assert!(linked(&g, "Projects/Web", "reading"), "block ref");
    assert!(linked(&g, "Projects", "reading"), "#tag by title::");
    assert_eq!(node(&g, "Projects").tag.as_deref(), Some("area"));
    assert_eq!(node(&g, "Projects").folder.as_deref(), Some("pages"));
    assert_eq!(g.tagged, 1);
    assert!(!g.nodes.iter().any(|n| n.id == "not a link" || n.id == "nope"));
}

#[test]
fn logseq_refresh_ignores_other_folders() {
    let dir = tempfile::tempdir().unwrap();
    fixtures::logseq(dir.path());
    let mut vault = Vault::scan(dir.path(), opts(SourceKind::Logseq, true));
    let bak = dir.path().join("logseq/bak/pages/New.md");
    std::fs::write(&bak, "- [[x]]").unwrap();
    assert!(!vault.refresh_paths(&[bak]));
    let page = dir.path().join("pages/New.md");
    std::fs::write(&page, "- [[Projects]]").unwrap();
    assert!(vault.refresh_paths(&[page]));
    assert_eq!(vault.len(), 11);
}

#[test]
fn dendron() {
    let g = check(fixtures::dendron, SourceKind::Dendron, 9, (14, 16), (9, 11));
    assert!(g.nodes.iter().all(|n| n.id != "README" && n.id != "readme"));
    assert!(linked(&g, "proj.beta.deep.leaf", "proj.beta.deep") && linked(&g, "proj.beta.deep", "proj.beta"));
    assert!(linked(&g, "lang.rust", "meeting.2024"), "cross-vault link");
    assert!(linked(&g, "proj.alpha", "proj.beta"), "label|target order");
    assert!(linked(&g, "meeting", "work/root") && linked(&g, "proj", "notes/root"));
    assert_eq!(node(&g, "proj.alpha").label.as_deref(), Some("Alpha"));
    assert_eq!(g.tagged, 2);
}

#[test]
fn foam() {
    let g = check(fixtures::foam, SourceKind::Notes, 6, (8, 12), (6, 10));
    assert!(g.nodes.iter().all(|n| n.id != "template-link" && n.id != "in-code" && n.id != "not-there"));
    assert!(linked(&g, "getting-started", "with space"), "angle brackets and %20");
    assert!(linked(&g, "project-a", "getting-started"), "alias");
    assert!(linked(&g, "project-a", "nowhere") && node(&g, "nowhere").ghost);
    assert!(linked(&g, "projects/inbox", "./inbox"));
    assert_eq!(node(&g, "getting-started").label.as_deref(), Some("Getting Started"));
    assert_eq!(g.tagged, 2);
}

#[test]
fn zettlr() {
    let g = check(fixtures::zettlr, SourceKind::Notes, 4, (5, 8), (4, 7));
    assert!(linked(&g, "Index", "202101011200 Zettelkasten"));
    assert_eq!(node(&g, "Linking").label.as_deref(), Some("Linking notes"));
}

#[test]
fn org_roam() {
    let g = check(fixtures::org, SourceKind::Notes, 4, (5, 7), (4, 6));
    assert!(linked(&g, "20240102-org", "20240101-emacs"), "id link");
    assert!(linked(&g, "2024-01-03", "20240102-org"), "roam: alias and heading id");
    assert!(linked(&g, "notes", "20240102-org"), "wikilink to an org alias");
    assert_eq!(node(&g, "20240101-emacs").tags.as_deref().map(<[String]>::len), Some(2));
    assert_eq!(node(&g, "20240102-org").tag.as_deref(), Some("code"));
    assert_eq!(node(&g, "20240101-emacs").label.as_deref(), Some("Emacs"));
}

#[test]
fn plain_markdown_is_a_folder() {
    check(fixtures::plain, SourceKind::Folder, 5, (9, 8), (9, 8));
    let dir = tempfile::tempdir().unwrap();
    fixtures::plain(dir.path());
    let (_, g) = scan(dir.path(), SourceKind::Notes, true);
    assert_eq!((g.nodes.len(), g.links.len()), (4, 0));
}

#[test]
fn file_tree_is_a_folder() {
    let g = check(fixtures::tree, SourceKind::Folder, 10, (16, 15), (16, 15));
    assert!(g.nodes.iter().all(|n| !n.id.starts_with("node_modules") && !n.id.starts_with("target")));
    assert_eq!(node(&g, "src/util/mod.rs").tag.as_deref(), Some("code"));
}

#[test]
fn huge_folder_is_capped() {
    let dir = tempfile::tempdir().unwrap();
    fixtures::huge(dir.path(), 200);
    let (auto, g) = scan(dir.path(), SourceKind::Auto, true);
    assert_eq!(auto.kind(), SourceKind::Notes);
    assert_eq!((g.nodes.len(), g.links.len()), (200, 199));

    // Breadth first in name order: the first 50 files are a000/b0 … a000/b4.
    let capped = |kind| {
        let v = Vault::scan_with(dir.path(), opts(kind, true), Guard::with_cap(50));
        assert!(v.capped());
        v.graph()
    };
    let g = capped(SourceKind::Notes);
    assert!(g.capped);
    assert_eq!((g.nodes.len(), g.links.len()), (50, 49));
    let g = capped(SourceKind::Folder);
    assert_eq!((g.nodes.len(), g.links.len()), (57, 56));
}

#[test]
fn file_size_weights_follow_notes_first() {
    let dir = tempfile::tempdir().unwrap();
    fixtures::foam(dir.path());
    let vault = Vault::scan(dir.path(), VaultOptions { size_by_file: true, ..opts(SourceKind::Notes, true) });
    let g = vault.graph();
    assert!(g.nodes.iter().all(|n| n.ghost == n.weight.is_none()));
}
