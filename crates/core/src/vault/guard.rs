//! Guardrails for reading folders that aren't Obsidian vaults: default skips, iCloud
//! placeholders and a file cap. See `docs/beyond-obsidian.md#guardrails`.
//!
//! Obsidian vaults never go through these: their reading stays exactly as it was.

use std::fs::Metadata;
use std::path::{Path, PathBuf};

/// At most this many files go into a graph.
pub const MAX_FILES: usize = 35_000;
/// The walk also stops after looking at this many directory entries, so a huge folder
/// with few matching files (a home folder read as notes) still ends quickly.
pub const MAX_VISITED: usize = MAX_FILES * 4;

/// Folder names skipped at any depth: dependencies, build output and caches. None of
/// them hold notes, and they are where most of a big tree's files are.
pub const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "bower_components",
    "target",
    "build",
    "dist",
    "out",
    "DerivedData",
    "Pods",
    "__pycache__",
    "venv",
    "Caches",
    "cache",
];

/// Files whose presence next to a `vendor` folder marks it as dependencies.
const VENDOR_MARKERS: &[&str] = &["go.mod", "composer.json", "Gemfile"];

/// `SF_DATALESS` from `<sys/stat.h>`: the file's contents (or a folder's listing) live in
/// the cloud; reading it would start a download.
const SF_DATALESS: u32 = 0x4000_0000;

#[derive(Debug, Clone)]
pub struct Guard {
    /// The user's home folder, to skip `~/Library`.
    library: Option<PathBuf>,
    pub max_files: usize,
    pub max_visited: usize,
}

impl Default for Guard {
    fn default() -> Self {
        let library = std::env::var_os("HOME").filter(|h| !h.is_empty()).map(|h| PathBuf::from(h).join("Library"));
        Guard { library, max_files: MAX_FILES, max_visited: MAX_VISITED }
    }
}

impl Guard {
    /// A guard with a smaller cap, for tests.
    pub fn with_cap(max_files: usize) -> Guard {
        Guard { max_files, max_visited: max_files * 4, ..Guard::default() }
    }

    /// Whether to leave out the folder `path` (named `name`, inside `parent`).
    pub fn skip_dir(&self, parent: &Path, name: &str, path: &Path) -> bool {
        if SKIP_DIRS.contains(&name) {
            return true;
        }
        if name == "vendor" && VENDOR_MARKERS.iter().any(|m| parent.join(m).is_file()) {
            return true;
        }
        self.library.as_deref() == Some(path)
    }
}

/// True when the entry is an iCloud placeholder whose contents aren't on this Mac.
/// Uses the metadata already fetched (`lstat`), which never triggers a download.
pub fn is_dataless(meta: &Metadata) -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        meta.st_flags() & SF_DATALESS != 0
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (meta, SF_DATALESS);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_build_dirs_library_and_go_vendor() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let g = Guard { library: Some(root.join("Library")), ..Guard::default() };
        assert!(g.skip_dir(root, "node_modules", &root.join("node_modules")));
        assert!(g.skip_dir(root, "target", &root.join("target")));
        assert!(g.skip_dir(root, "Library", &root.join("Library")));
        // Only ~/Library, not any folder called Library.
        assert!(!g.skip_dir(&root.join("x"), "Library", &root.join("x/Library")));
        assert!(!g.skip_dir(root, "notes", &root.join("notes")));
        assert!(!g.skip_dir(root, "vendor", &root.join("vendor")));
        std::fs::write(root.join("go.mod"), "module x").unwrap();
        assert!(g.skip_dir(root, "vendor", &root.join("vendor")));
    }

    #[test]
    fn plain_files_are_not_dataless() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.md");
        std::fs::write(&f, "x").unwrap();
        assert!(!is_dataless(&std::fs::symlink_metadata(&f).unwrap()));
    }
}
