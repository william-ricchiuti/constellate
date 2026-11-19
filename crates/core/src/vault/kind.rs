//! Source kinds: how a folder is read. See `docs/beyond-obsidian.md`.

use std::fmt;

use serde::{Deserialize, Serialize};

/// How a folder is read. `Auto` is only a setting: [`Vault::scan`](super::Vault::scan)
/// resolves it with [`detect`](super::detect) before reading anything.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// Detect from the folder's contents.
    #[default]
    Auto,
    /// An Obsidian vault: exactly the web version's rules (`parser.js`).
    Obsidian,
    /// Generic notes: Markdown and org files with wikilinks, Markdown links and org links
    /// (Foam, Zettlr, org-roam, plain Markdown).
    Notes,
    /// A Logseq graph: `pages/` and `journals/`.
    Logseq,
    /// A Dendron workspace: dotted hierarchies.
    Dendron,
    /// Any folder: every file, linked to its parent folder.
    Folder,
}

impl SourceKind {
    /// Every value, in the order the settings page lists them.
    pub const ALL: [SourceKind; 6] = [
        SourceKind::Auto,
        SourceKind::Obsidian,
        SourceKind::Notes,
        SourceKind::Logseq,
        SourceKind::Dendron,
        SourceKind::Folder,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SourceKind::Auto => "auto",
            SourceKind::Obsidian => "obsidian",
            SourceKind::Notes => "notes",
            SourceKind::Logseq => "logseq",
            SourceKind::Dendron => "dendron",
            SourceKind::Folder => "folder",
        }
    }

    pub fn parse(name: &str) -> Option<SourceKind> {
        SourceKind::ALL.into_iter().find(|k| k.name() == name)
    }

    /// Words for the menu and settings page.
    pub fn label(self) -> &'static str {
        match self {
            SourceKind::Auto => "Detect automatically",
            SourceKind::Obsidian => "Obsidian vault",
            SourceKind::Notes => "Markdown or org notes",
            SourceKind::Logseq => "Logseq graph",
            SourceKind::Dendron => "Dendron workspace",
            SourceKind::Folder => "Any folder",
        }
    }

    /// Whether the guardrails (default skips, iCloud placeholders, the file cap) apply.
    /// Not for Obsidian, whose reading stays exactly as it was.
    pub fn guarded(self) -> bool {
        !matches!(self, SourceKind::Obsidian | SourceKind::Auto)
    }

    /// Whether a file with this name is part of the graph (before path rules).
    pub fn wants_name(self, name: &str) -> bool {
        match self {
            // Exactly ".md", case-sensitive, as in parser.js.
            SourceKind::Obsidian | SourceKind::Auto => name.ends_with(".md"),
            SourceKind::Notes | SourceKind::Logseq => is_note_name(name),
            SourceKind::Dendron => name.ends_with(".md"),
            SourceKind::Folder => true,
        }
    }
}

impl fmt::Display for SourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// `.md`, `.markdown` or `.org`, any case.
pub fn is_note_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown") || lower.ends_with(".org")
}

/// The file name without its note extension (`.md`, `.markdown`, `.org`).
pub fn note_stem(name: &str) -> &str {
    for ext in [".md", ".markdown", ".org"] {
        if name.len() > ext.len() && name[name.len() - ext.len()..].eq_ignore_ascii_case(ext) {
            return &name[..name.len() - ext.len()];
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for k in SourceKind::ALL {
            assert_eq!(SourceKind::parse(k.name()), Some(k));
            assert_eq!(serde_json::to_value(k).unwrap(), serde_json::Value::from(k.name()));
        }
        assert_eq!(SourceKind::parse("Obsidian"), None);
    }

    #[test]
    fn file_filters() {
        assert!(SourceKind::Obsidian.wants_name("a.md"));
        assert!(!SourceKind::Obsidian.wants_name("a.MD"));
        assert!(!SourceKind::Obsidian.wants_name("a.org"));
        assert!(SourceKind::Notes.wants_name("a.MD") && SourceKind::Notes.wants_name("a.org"));
        assert!(SourceKind::Notes.wants_name("a.markdown"));
        assert!(!SourceKind::Notes.wants_name("a.txt"));
        assert!(SourceKind::Folder.wants_name("photo.jpg"));
        assert_eq!(note_stem("Note.Markdown"), "Note");
        assert_eq!(note_stem("x.org"), "x");
        assert_eq!(note_stem(".md"), ".md");
    }

    #[test]
    fn only_obsidian_is_unguarded() {
        assert!(!SourceKind::Obsidian.guarded());
        for k in [SourceKind::Notes, SourceKind::Logseq, SourceKind::Dendron, SourceKind::Folder] {
            assert!(k.guarded());
        }
    }
}
