# Beyond Obsidian: other notes apps and any folder

Status: design, 2026-11-19. Builds the five agreed groups: standard Markdown links and other
notes apps (Logseq, Dendron, Foam, Zettlr, org-roam), an "any folder" mode, folder-type
detection, guardrails, and wording plus onboarding.

The reader (`crates/core/src/vault/`) is the only part that knows about Obsidian. Everything
downstream takes a generic `Graph`, so this work stays almost entirely inside the reader, the
config store and the settings page.

## Source kinds

A **source kind** says how a folder is read. It is one value:

| Kind | Reads | Nodes | Links |
|---|---|---|---|
| `obsidian` | `*.md` | one per note | `[[wikilinks]]` (today's rules, unchanged) |
| `notes` | `*.md`, `*.markdown`, `*.org` | one per note | wikilinks, `[text](path.md)`, reference definitions, org `[[file:…]]` and `[[id:…]]` |
| `logseq` | `pages/` and `journals/` (`.md`, `.org`) | one per page | `[[page]]`, `#tag`, `#[[tag]]`, `alias::`, namespaces `a/b` → `a` |
| `dendron` | `*.md` in the vaults listed in `dendron.yml` | one per note | wikilinks (Dendron's `[[label\|target]]` order), `a.b.c` → `a.b` → `a` → `root` |
| `folder` | every file | one per file and one per folder | each file and folder → its parent folder |

Foam, Zettlr and org-roam are all `notes`: they differ in syntax, not in shape, and one
reader that understands all of their syntax is simpler than three.

In config it is `sourceKind`: `auto` (the default) or one of the five kinds. `auto` is
resolved once per open (see [Detection](#detection)); the resolved kind is what the vault
and the settings page report.

## How readers plug in

`Vault` keeps its shape: a map of path → `Entry`, a full `scan`, and `refresh_paths` for
watcher batches. What changes is that a `Vault` now carries its resolved `SourceKind`, and
the four places that were hard-wired to Obsidian dispatch on it:

| Step | Obsidian (unchanged) | Others |
|---|---|---|
| which files count (`wants`) | name ends in `.md` | per kind (`kind.rs`) |
| walking (`collect_files`) | today's walk, no guard | same walk, plus the [guardrails](#guardrails) |
| reading one file (`parse_entry`) | `scan::parse_entry` | `formats::parse` (notes, logseq, dendron), `folder::stat_entry` (folder) |
| building the graph (`graph`) | `graph::materialize` | `formats::materialize`, `folder::materialize` |

`Entry` gains fields the other kinds need (`size`, and a boxed `NoteExtra` with title,
aliases, path links, org ids and id links). Obsidian entries leave them empty. `Node` gains
one optional field, `weight`, and `Graph` gains `capped`; both are left out of the JSON when
unset, so `vw-dump` output for an Obsidian vault does not change.

Module layout:

- `vault/kind.rs`: `SourceKind`, the per-kind file filter.
- `vault/detect.rs`: `detect(root) -> Detection`.
- `vault/guard.rs`: skip rules, iCloud placeholders, the file cap.
- `vault/formats/`: `markdown.rs` (Markdown links, reference definitions), `org.rs`,
  `logseq.rs`, `dendron.rs`, and `mod.rs` with `parse` and `materialize` for the notes kinds.
- `vault/folder.rs`: the "any folder" reader.
- `vault/sample.rs`: the built-in sample graph.

## Obsidian stays byte-identical

- `kind = obsidian` runs exactly today's code: `scan::collect_markdown_files`,
  `scan::parse_entry`, `graph::materialize`. No guard, no cap, no new skips, no Markdown
  links. `vw-dump` defaults to `obsidian` (pass `--kind` for others), so `parity/run.sh` is
  unchanged and still passes.
- `auto` picks `obsidian` whenever the folder or any parent up to the home folder has a
  `.obsidian/` folder, so an existing user who points at a vault (or a folder inside one)
  gets today's graph.
- New config keys are left out of `config.json` while they hold their default
  (`skip_serializing_if`), so an existing config saves byte for byte as before, and the
  config parity cases (which never use the new keys) are unaffected.
- Settings that only matter to other kinds are hidden in the settings page for Obsidian
  vaults.

Not changed for Obsidian, on purpose (William decided 2026-11-19 to keep both as today): Obsidian's own graph counts
`[text](note.md)` links, and reading an evicted iCloud note triggers a download. Both stay
as today for `obsidian`; turning either on changes existing users' graphs or behaviour.

## Config keys

| Key | Values | Default | Notes |
|---|---|---|---|
| `sourceKind` | `auto`, `obsidian`, `notes`, `logseq`, `dendron`, `folder` | `auto` | changing it rescans |
| `nodeSize` | `links`, `fileSize` | `links` | `fileSize` sizes nodes by bytes on disk; changing it rescans |

Validation follows the existing `Enum` rule: anything else is an invalid key (dropped from a
file with an issue, rejecting a patch). Both keys are native only; the web version rejects
them as unknown, like the native themes.

Colour needs no new key. In `folder` mode a node's tag is its file type (`markdown`,
`image`, `code`, `document`, `data`, `audio`, `video`, `archive`, `folder`, `other`), so
`nodeColorMode: "tag"` colours by file type and `"folder"` by top-level folder; the settings
page labels them that way. Recency glow uses `mtime` as before.

`nodeSize: "fileSize"` sets `Node.weight` to a degree-equivalent of the file size,
`(log2(1 + bytes / 1024))² / 4` (1 KB ≈ 0.25, 1 MB ≈ 25, 1 GB ≈ 100), which the layout
uses in place of the link count for the node's radius. Folder nodes weigh the sum of
their files.

## Detection

`auto` resolves in this order, first match wins:

1. `.obsidian/` in the folder → `obsidian`
2. `logseq/config.edn` → `logseq`
3. `dendron.yml` → `dendron`
4. `.foam/` → `notes`
5. `.obsidian/` in a parent folder, up to the home folder → `obsidian`
6. A bounded sample, using the guardrails: walk up to 2,000 entries, read up to 200 notes
   (each up to 256 KB, never an iCloud placeholder). If notes make up at least a quarter of
   the files and average at least 0.2 links each → `notes`. Otherwise (Markdown without
   links, a code tree, photos, anything else) → `folder`.

Detection costs a few `stat` calls in the common case and well under 100 ms in the worst.
The result and the reason ("found .obsidian", "Markdown with links", "no notes") are logged
and shown in the settings page next to the override.

## Guardrails

Applied by every kind except `obsidian`, and inside the App Store sandbox (they are plain
path and `stat` rules, no extra entitlements):

- **Cap:** at most 35,000 files in the graph. The walk stops at the cap, or after visiting
  140,000 entries, whichever comes first. The graph is marked `capped` and the menu and
  settings say "Showing the first 35,000 files. Choose a smaller folder or skip some
  subfolders." Nothing crashes, nothing hangs.
- **Default skips** (folder names, at any depth): `node_modules`, `bower_components`,
  `target`, `build`, `dist`, `out`, `DerivedData`, `Pods`, `__pycache__`, `venv`,
  `.venv`, `vendor` (only next to a `go.mod`, `composer.json` or `Gemfile`), `Caches`,
  `cache`, and `~/Library` itself. Hidden names (starting with `.`) are already skipped by
  every kind. `ignorePaths` still applies on top.
- **iCloud placeholders:** a file or folder with the `SF_DATALESS` flag (from `lstat`,
  which never downloads) is skipped, as are `.icloud` stubs (hidden, so already skipped).
  Nothing is opened or listed unless it is already on disk.
- **Bounded reads:** a note over 4 MB is read only for its first 4 MB, and the folder
  reader never opens files at all; it only uses `lstat`.

## Onboarding and wording

- The menu says **Choose Folder…**; the panel title says "Choose a notes folder or any
  folder".
- With no folder chosen, the wallpaper shows a built-in sample graph (about 150 nodes,
  several tags and folders, recent edits for the glow), and the menu status says
  "Sample graph: choose a folder to show yours".
- The settings page gains a "Folder" section: the path, the detected kind with its reason,
  the override, and in folder mode the node-size option.
- README, `packaging/appstore/listing.md` and the first-run text say "your notes or any
  folder", with Obsidian still named first.

## Testing

- Unit tests per format (Markdown links, org, Logseq, Dendron, folder, guard, detect).
- Fixture folders are built by `crates/core/tests/fixtures/mod.rs` (one builder per kind,
  in the spirit of `parity/make-fixture.js`); `cargo run -p vw-core --example make_fixture
  -- <kind> <dir>` writes the same folders for running the app by hand. Never real folders.
- `parity/run.sh` unchanged and identical.
- The app runs against each fixture with a scratch `HOME` and logs the kind, node and link
  counts; a generated 100,000-file tree checks the cap.
