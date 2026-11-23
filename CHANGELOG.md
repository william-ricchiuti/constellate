# Changelog

All notable changes to Constellate. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Other notes apps: Logseq (pages, journals, tags, aliases, namespaces), Dendron (dotted
  hierarchies, multi-vault), and Foam, Zettlr, org-roam or plain Markdown and org notes
  (wikilinks, Markdown links, reference links, org `file:` and `id:` links).
- Any folder: every file is a dot linked to its folder, coloured by file type, optionally
  sized by file size (`nodeSize: "fileSize"`). Files are listed, never opened.
- Detection: the app works out what kind of folder it was given (`.obsidian/`,
  `logseq/config.edn`, `dendron.yml`, `.foam/`, else a small sample of the files) and shows
  the result and the reason in Settings → Folder, where **Read as** overrides it
  (`sourceKind`).
- Guardrails outside Obsidian vaults: at most 35,000 files in the graph (the menu says when a
  folder has more), `node_modules`, build folders, caches and `~/Library` skipped, and iCloud
  placeholders skipped without being downloaded.
- A built-in sample graph shows while no folder is chosen.
- Settings → App: **Show in Dock** and **Hide menu-bar icon**. Opening the app again while
  it runs (Finder, Spotlight, the Dock) opens Settings, so a hidden icon is never a dead end.

### Changed

- The app is now called **Constellate** (`Constellate.app`, `Constellate-<version>.dmg`,
  Homebrew cask `constellate`, Obsidian plugin `constellate`), with a new icon.
- **Choose Vault…** is now **Choose Folder…**, and the Open panel asks for "a notes folder or
  any folder".
- Obsidian vaults are read exactly as before; existing configs and layouts are unchanged.

## [1.0.0-beta.1] - unreleased

The first beta of the native macOS app, a rewrite of the web version
([obsidian-live-wallpaper](https://github.com/william-ricchiuti/obsidian-live-wallpaper)).

### Added

- Your Obsidian vault's graph drawn behind the desktop icons, on every Space and display,
  by the GPU (wgpu on Metal). No Node, local server or wallpaper host.
- Vault reader with the web version's rules (wikilinks and Markdown links, aliases, tags, ignored paths),
  checked against `parser.js` by parity tests. It watches the vault and updates the picture
  when notes change.
- Layout engine on a background thread (force, rings, timeline, galaxy), with positions
  cached to disk.
- Themes: Default (the web renderer and its presets), Night Sky, Aurora Lite and
  Ink & Paper, with Ink & Paper's ink bleed when a note is saved (motion on).
- Menu-bar app (no Dock icon): Pause, Settings, Choose Vault, Import Web Version Settings,
  Export PNG, Open Config File, Start at Login.
- First run asks for the vault; new installs start with motion off.
- Settings window with the web version's settings page: themes, presets, custom presets,
  colours, labels, layout and motion. Changes apply live.
- Import of the web version's `config.json` and `custom-presets.json`.
- Config at `~/Library/Application Support/Constellate/config.json`, in the web format,
  reloaded when edited.
- Draws nothing while covered or still; glow breathing runs in the window server.
- Universal (Apple Silicon and Intel) app bundle for macOS 13+ and app icon, as a signed and
  notarized DMG and a Homebrew cask.
- Automatic updates for the DMG and Homebrew versions (Sparkle 2): one small HTTPS check a
  day, and Check for Updates… in the menu. The Mac App Store build has no updater.

### Not yet

- Native versions of most web themes (they draw with Default for now).

[Unreleased]: https://github.com/william-ricchiuti/constellate/compare/v1.0.0-beta.1...HEAD
[1.0.0-beta.1]: https://github.com/william-ricchiuti/constellate/releases/tag/v1.0.0-beta.1
