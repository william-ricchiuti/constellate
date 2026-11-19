//! Folder mode and the guardrails, end to end through `Vault`.

use std::fs;
use std::path::Path;

use super::*;

fn write(root: &Path, rel: &str, bytes: usize) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, vec![b'x'; bytes]).unwrap();
}

fn opts(kind: SourceKind) -> VaultOptions {
    VaultOptions { kind, ..VaultOptions::default() }
}

fn ids(g: &Graph) -> Vec<&str> {
    let mut v: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
    v.sort_unstable();
    v
}

/// A small tree with things the guard must skip.
fn tree(root: &Path) {
    write(root, "readme.md", 10);
    write(root, "src/main.rs", 100);
    write(root, "src/lib/util.rs", 100);
    write(root, "photos/a.jpg", 5000);
    write(root, ".git/HEAD", 10);
    write(root, "node_modules/pkg/index.js", 10);
    write(root, "target/debug/app", 10);
    write(root, "src/__pycache__/x.pyc", 10);
    write(root, ".hidden/secret.txt", 10);
}

#[test]
fn folder_mode_reads_every_file_but_skips_build_dirs_and_hidden() {
    let d = tempfile::tempdir().unwrap();
    tree(d.path());
    let v = Vault::scan(d.path(), opts(SourceKind::Folder));
    assert_eq!(v.kind(), SourceKind::Folder);
    assert_eq!(v.len(), 4);
    let g = v.graph();
    assert_eq!(ids(&g), ["/", "photos", "photos/a.jpg", "readme.md", "src", "src/lib", "src/lib/util.rs", "src/main.rs"]);
    // A tree: every node but the root has exactly one parent link.
    assert_eq!(g.links.len(), g.nodes.len() - 1);
    assert!(!g.capped);
    assert!(g.nodes.iter().all(|n| n.weight.is_none()));
}

#[test]
fn auto_detects_a_plain_tree_as_a_folder() {
    let d = tempfile::tempdir().unwrap();
    tree(d.path());
    let v = Vault::scan(d.path(), VaultOptions { kind: SourceKind::Auto, ..VaultOptions::default() });
    assert_eq!(v.kind(), SourceKind::Folder);
    assert_eq!(v.reason(), "notes without links");
}

#[test]
fn obsidian_kind_is_not_guarded() {
    // An Obsidian vault reads exactly as before: notes in a "build" folder still count.
    let d = tempfile::tempdir().unwrap();
    write(d.path(), "build/Note.md", 5);
    write(d.path(), "node_modules/Other.md", 5);
    assert_eq!(Vault::scan(d.path(), VaultOptions::default()).len(), 2);
    assert_eq!(Vault::scan(d.path(), opts(SourceKind::Notes)).len(), 0);
}

#[test]
fn the_cap_stops_the_walk_and_marks_the_graph() {
    let d = tempfile::tempdir().unwrap();
    for i in 0..30 {
        write(d.path(), &format!("d{}/f{i:02}.txt", i % 3), 1);
    }
    let v = Vault::scan_with(d.path(), opts(SourceKind::Folder), Guard::with_cap(10));
    assert_eq!(v.len(), 10);
    assert!(v.capped());
    let g = v.graph();
    assert!(g.capped);
    // Deterministic: the same files every time (breadth first, by name).
    let again = Vault::scan_with(d.path(), opts(SourceKind::Folder), Guard::with_cap(10));
    let mut a: Vec<_> = v.paths().collect();
    let mut b: Vec<_> = again.paths().collect();
    a.sort();
    b.sort();
    assert_eq!(a, b);
    // Under the cap: not capped.
    assert!(!Vault::scan_with(d.path(), opts(SourceKind::Folder), Guard::with_cap(30)).capped());
}

#[test]
fn the_visit_limit_ends_a_walk_with_few_matches() {
    // Many files, none of them notes: the notes walk gives up after max_visited.
    let d = tempfile::tempdir().unwrap();
    for i in 0..50 {
        write(d.path(), &format!("f{i:02}.bin"), 1);
    }
    let v = Vault::scan_with(d.path(), opts(SourceKind::Notes), Guard::with_cap(5));
    assert_eq!(v.len(), 0);
    assert!(v.capped());
}

#[test]
fn refresh_respects_the_cap_and_the_skips() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    for i in 0..3 {
        write(root, &format!("f{i}.txt"), 1);
    }
    let mut v = Vault::scan_with(root, opts(SourceKind::Folder), Guard::with_cap(4));
    assert_eq!(v.len(), 3);
    write(root, "node_modules/x.js", 1);
    assert!(!v.refresh_paths(&[root.join("node_modules/x.js")]));
    assert!(!v.refresh_paths(&[root.join("node_modules")]));
    write(root, "new.txt", 1);
    assert!(v.refresh_paths(&[root.join("new.txt")]));
    assert_eq!(v.len(), 4);
    assert!(!v.capped());
    write(root, "one-too-many.txt", 1);
    assert!(v.refresh_paths(&[root.join("one-too-many.txt")]));
    assert_eq!(v.len(), 4);
    assert!(v.capped() && v.graph().capped);
    // A new folder past the cap is left out too.
    write(root, "more/a.txt", 1);
    v.refresh_paths(&[root.join("more")]);
    assert_eq!(v.len(), 4);
    // Deleting is always seen.
    fs::remove_file(root.join("f0.txt")).unwrap();
    assert!(v.refresh_paths(&[root.join("f0.txt")]));
    assert_eq!(v.len(), 3);
}

#[test]
fn file_size_weights_notes_and_files() {
    let d = tempfile::tempdir().unwrap();
    write(d.path(), "small.md", 10);
    write(d.path(), "big.md", 4 * 1024 * 1024);
    write(d.path(), "small.md.txt", 1);
    let o = VaultOptions { size_by_file: true, ..VaultOptions::default() };
    let g = Vault::scan(d.path(), o).graph();
    let w = |id: &str| g.nodes.iter().find(|n| n.id == id).unwrap().weight.unwrap();
    assert!(w("big") > 25.0 && w("small") < 1.0, "{} {}", w("big"), w("small"));
    // Off by default: Obsidian output is unchanged.
    assert!(Vault::scan(d.path(), VaultOptions::default()).graph().nodes.iter().all(|n| n.weight.is_none()));
    let f = Vault::scan(d.path(), VaultOptions { kind: SourceKind::Folder, size_by_file: true, ..VaultOptions::default() });
    let g = f.graph();
    assert!(g.nodes.iter().all(|n| n.weight.is_some()));
}

#[test]
fn size_weight_curve() {
    assert!(size_weight(0) == 0.0);
    assert!((size_weight(1024 * 1024) - 25.0).abs() < 0.1);
    assert!((size_weight(1 << 30) - 100.0).abs() < 0.1);
}
