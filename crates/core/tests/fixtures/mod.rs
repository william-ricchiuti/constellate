//! Fixture folders for each source kind, full of edge cases, in the spirit of
//! `parity/make-fixture.js`. Each builder writes into `dir` (creating it) and is
//! deterministic apart from file times. `tests/kinds.rs` asserts their exact node and link
//! counts; `examples/make_fixture.rs` writes them for running the app by hand.
//!
//! Shared by tests and an example, so not every function is used by each.
#![allow(dead_code)]

use std::fs;
use std::path::Path;

/// Every builder's name, as `make_fixture` takes it.
pub const KINDS: [&str; 8] = ["logseq", "dendron", "foam", "zettlr", "org", "plain", "tree", "huge"];

/// Builds the fixture called `kind` in `dir` (`n` files for `huge`).
pub fn build(kind: &str, dir: &Path, n: usize) -> Result<(), String> {
    match kind {
        "logseq" => logseq(dir),
        "dendron" => dendron(dir),
        "foam" => foam(dir),
        "zettlr" => zettlr(dir),
        "org" => org(dir),
        "plain" => plain(dir),
        "tree" => tree(dir),
        "huge" => huge(dir, n),
        _ => return Err(format!("unknown fixture {kind:?}; one of {}", KINDS.join(", "))),
    }
    Ok(())
}

fn w(root: &Path, rel: &str, content: impl AsRef<[u8]>) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

/// A Logseq graph: namespaces (triple-lowbar and legacy `%2F`), `title::`, `alias::`,
/// `tags::`, `#tag`, `#[[multi word]]`, block refs, journals linked by Logseq's date
/// title, code blocks, an org page, and folders that aren't pages (`logseq/bak`, assets).
///
/// 10 pages; with ghosts 13 nodes and 15 links, without 10 nodes and 12 links.
pub fn logseq(dir: &Path) {
    w(dir, "logseq/config.edn", "{:meta/version 1}\n");
    w(dir, "logseq/custom.css", "");
    w(dir, "logseq/bak/pages/Projects/2024-01-01T00_00_00.000Z.Desktop.md", "- [[Ghost From Backup]]\n");
    w(dir, "assets/image_1700000000000_0.png", [0x89, b'P', b'N', b'G']);
    w(dir, "draws/sketch.excalidraw", "{}");
    w(dir, "pages/Projects.md", "alias:: Proj\ntags:: area\n\n- Overview of [[Projects/Web]] and #reading\n");
    w(dir, "pages/Projects___Web.md", "- Web stuff; its parent is implied by the name\n- see ((64f0c1a2-0000-4000-8000-000000000001))\n");
    w(dir, "pages/Projects___Web___Frontend.md", "- [[proj]] work, by alias and in another case\n");
    w(dir, "pages/Recipes%2FPasta.md", "- legacy namespace file name; Recipes has no page\n");
    w(dir, "pages/Reading List.md", "title:: reading\n\n- books\n  id:: 64f0c1a2-0000-4000-8000-000000000001\n");
    w(dir, "pages/area.md", "- #[[Big Topic]] and [[Missing Page]]\n");
    w(dir, "pages/Code.md", "- ```clojure\n  [[not a link]] #not-a-tag\n  ```\n- `#nope [[nope]]`\n- real [[Projects]]\n");
    w(dir, "pages/Org Page.org", "#+title: Org Page\n#+alias: op\n* heading [[area]]\n");
    w(dir, "journals/2024_01_31.md", "- Met about [[Projects/Web/Frontend]]\n- #Projects\n");
    w(dir, "journals/2024_02_01.md", "- Follow up from [[Jan 31st, 2024]]\n");
}

/// A Dendron workspace with two vaults (one named in `dendron.yml`), dotted hierarchies
/// with missing levels, `[[label|target]]`, anchors, note refs, a cross-vault link, code,
/// and files outside the vaults or in their subfolders.
///
/// 9 notes; with ghosts 14 nodes and 16 links, without 9 nodes and 11 links.
pub fn dendron(dir: &Path) {
    w(
        dir,
        "dendron.yml",
        "version: 5\nworkspace:\n  vaults:\n    - fsPath: notes\n    - fsPath: work\n      name: job\n  journal:\n    dailyDomain: daily\n",
    );
    w(dir, "README.md", "[[proj]] outside every vault\n");
    w(dir, "notes/root.md", "---\nid: root\ntitle: root\n---\n");
    w(dir, "notes/root.schema.yml", "version: 1\nschemas: []\n");
    w(dir, "notes/proj.md", "---\ntitle: Proj\n---\n");
    w(dir, "notes/proj.alpha.md", "---\ntitle: Alpha\ntags: [active]\n---\nSee [[Beta|proj.beta]] and [[proj.gamma#intro]].\n");
    w(dir, "notes/proj.beta.md", "---\ntitle: Beta\n---\n![[proj.alpha]]\n");
    w(dir, "notes/proj.beta.deep.leaf.md", "---\ntitle: Leaf\n---\n#idea\n");
    w(dir, "notes/lang.rust.md", "[[dendron://job/meeting.2024]]\n");
    w(dir, "notes/code.md", "```\n[[proj.alpha]]\n```\n`[[proj]]`\n");
    w(dir, "notes/assets/readme.md", "[[proj]] in a subfolder\n");
    w(dir, "notes/assets/images/x.png", [0u8; 4]);
    w(dir, "work/root.md", "");
    w(dir, "work/meeting.2024.md", "[[lang.rust]] [[missing.note]]\n");
}

/// A Foam workspace: wikilinks, Markdown links (relative, `./`, `../`, `<with space>`,
/// `%20`, anchors), reference definitions, aliases, a duplicate basename, images, URLs,
/// links in code, and `.foam/` (hidden, so its template is skipped).
///
/// 6 notes; with ghosts 8 nodes and 12 links, without 6 nodes and 10 links.
pub fn foam(dir: &Path) {
    w(dir, ".foam/templates/new-note.md", "[[template-link]]\n");
    w(dir, ".vscode/settings.json", "{}\n");
    w(dir, "inbox.md", "# Inbox\n\nSee [[ideas]] and [Getting started](getting-started.md).\n\n[ideas]: ideas \"Ideas\"\n");
    w(
        dir,
        "ideas.md",
        "---\ntags: [brainstorm]\n---\n# Ideas\n- [Project A](projects/project-a.md)\n- [[missing-idea]]\n- ![diagram](attachments/diagram.png)\n- [Website](https://foambubble.github.io)\n",
    );
    w(
        dir,
        "getting-started.md",
        "---\ntitle: Getting Started\naliases: [start]\n---\nBack to [inbox](./inbox.md#top), [space](<docs/with space.md>), [enc](docs/with%20space.md)\n",
    );
    w(
        dir,
        "projects/project-a.md",
        "Parent: [ideas](../ideas.md)\nAlso [[start]] and [nowhere](../nowhere.md) and [soft](../not-there)\n```\n[[in-code]]\n```\n",
    );
    w(dir, "projects/inbox.md", "[[inbox]]\n");
    w(dir, "docs/with space.md", "#doc [[inbox]]\n");
    w(dir, "attachments/diagram.png", [0u8; 16]);
}

/// Zettlr-style notes: no marker folder, ids in file names, wikilinks, a `.markdown`
/// file with a title, URL-encoded Markdown links, and a bibliography file.
///
/// 4 notes; with ghosts 5 nodes and 8 links, without 4 nodes and 7 links.
pub fn zettlr(dir: &Path) {
    w(dir, "202101011200 Zettelkasten.md", "# Zettelkasten\n#method\nSee [[202101021200 Atomic notes]] and [[Linking]].\n");
    w(dir, "202101021200 Atomic notes.md", "Atomic. Related: [[202101011200 Zettelkasten]]\n");
    w(
        dir,
        "Linking.markdown",
        "---\ntitle: Linking notes\n---\nUse [[202101021200 Atomic notes]]. Also [md link](202101011200%20Zettelkasten.md)\n",
    );
    w(dir, "sub/Index.md", "- [Zettelkasten](../202101011200%20Zettelkasten.md)\n- [[Linking]]\n- [[Unwritten]]\n");
    w(dir, "refs/library.bib", "@book{x, title={X}}\n");
}

/// org-roam: file and heading `:ID:`s, `#+title:`, `#+filetags:`, heading tags,
/// `#+roam_aliases:`, `[[id:…]]`, `[[file:…]]`, `[[roam:…]]`, plain `[[Title]]`, a
/// missing id, a missing file, a URL, and a Markdown note in the same folder.
///
/// 4 notes; with ghosts 5 nodes and 7 links, without 4 nodes and 6 links.
pub fn org(dir: &Path) {
    w(
        dir,
        "20240101-emacs.org",
        ":PROPERTIES:\n:ID: 11111111-aaaa\n:END:\n#+title: Emacs\n#+filetags: :tools:editor:\n\nSee [[id:22222222-bbbb][Org mode]] and [[file:20240102-org.org][org file]].\n",
    );
    w(
        dir,
        "20240102-org.org",
        ":PROPERTIES:\n:ID: 22222222-bbbb\n:END:\n#+title: Org mode\n#+roam_aliases: \"Org\" orgmode\n\n* Babel :code:\n:PROPERTIES:\n:ID: 33333333-cccc\n:END:\nBack to [[id:11111111-aaaa][Emacs]]. Missing [[id:99999999-ffff][gone]].\n",
    );
    w(
        dir,
        "daily/2024-01-03.org",
        "#+title: 2024-01-03\n* Notes\n[[id:33333333-cccc][Babel]] [[file:../20240101-emacs.org]] [[file:../missing.org][m]] [[https://orgmode.org][web]]\n[[roam:Org]] [[Emacs]]\n",
    );
    w(dir, "notes.md", "Mixed folder: [[orgmode]] and [Emacs](20240101-emacs.org)\n");
    w(dir, "org-roam.db", [0u8; 32]);
}

/// Markdown without links (URLs don't count), plus an image.
///
/// As `folder`: 9 nodes (root, 3 folders, 5 files) and 8 links. As `notes`: 4 nodes, 0 links.
pub fn plain(dir: &Path) {
    w(dir, "README.md", "# Readme\nJust text. Visit https://example.com or [the site](https://example.com).\n");
    w(dir, "notes/todo.md", "- buy milk\n- call mom\n");
    w(dir, "notes/ideas.md", "Some #hashtag-like text but no links.\n");
    w(dir, "notes/image.png", [0u8; 8]);
    w(dir, "journal/2024/jan.md", "Cold.\n");
}

/// A mixed file tree: code, images, documents, nested folders, and folders the guard
/// skips (`node_modules`, `target`, `build`, `dist`, hidden `.git`).
///
/// As `folder`: 16 nodes (root, 5 folders, 10 files) and 15 links.
pub fn tree(dir: &Path) {
    w(dir, "README.md", "# Project\nSee docs/guide.md\n");
    w(dir, "Cargo.toml", "[package]\nname = \"x\"\n");
    w(dir, "package.json", "{}\n");
    w(dir, "src/main.rs", "fn main() {}\n");
    w(dir, "src/lib.rs", "pub fn f() {}\n");
    w(dir, "src/util/mod.rs", "\n");
    w(dir, "docs/guide.md", "Guide with a [[link]]\n");
    w(dir, "docs/manual.pdf", b"%PDF-1.4\n");
    w(dir, "assets/logo.png", [0u8; 64]);
    w(dir, "assets/icons/a.svg", "<svg/>");
    w(dir, "node_modules/pkg/index.js", "module.exports = 1\n");
    w(dir, "node_modules/pkg/README.md", "[[x]]\n");
    w(dir, "target/debug/app", [0u8; 8]);
    w(dir, "build/out.o", [0u8; 8]);
    w(dir, "dist/bundle.js", "\n");
    w(dir, ".git/HEAD", "ref: refs/heads/main\n");
}

/// `n` small notes in nested folders (`aNNN/bN/fNNNNNN.md`, ten per folder, a hundred per
/// top folder), each linking to the previous one: for the file cap.
pub fn huge(dir: &Path, n: usize) {
    for i in 0..n {
        let rel = format!("a{:03}/b{}/f{i:06}.md", i / 100, (i / 10) % 10);
        let text = if i == 0 { String::from("first\n") } else { format!("after [[f{:06}]]\n", i - 1) };
        w(dir, &rel, text);
    }
}
