//! Reading and writing `config.json`.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::{Config, ConfigError, Sanitized};

/// The app's folder name in `~/Library/Application Support` and `~/Pictures`.
pub const APP_DIR: &str = "Constellate";

/// What [`APP_DIR`] was called up to 1.0.0-beta.1, when the app was "Vault Wallpaper".
/// [`migrate_dir`] moves those folders over on the first launch after the rename.
pub const OLD_APP_DIR: &str = "Vault Wallpaper";

/// `~/Library/Application Support/Constellate/config.json`, or `None` without `$HOME`.
/// In the App Store build `$HOME` is the sandbox container's `Data` folder.
pub fn default_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty())?;
    Some(PathBuf::from(home).join("Library/Application Support").join(APP_DIR).join("config.json"))
}

/// The app's folders under `home` that the rename moves: `(old, new)` for Application
/// Support (settings, layout cache, custom presets) and for Pictures (exported PNGs).
pub fn renamed_dirs(home: &Path) -> [(PathBuf, PathBuf); 2] {
    [
        (home.join("Library/Application Support").join(OLD_APP_DIR), home.join("Library/Application Support").join(APP_DIR)),
        (home.join("Pictures").join(OLD_APP_DIR), home.join("Pictures").join(APP_DIR)),
    ]
}

/// Moves the folder `old` to `new` if `old` exists and `new` doesn't; otherwise does
/// nothing. Returns whether it moved. A rename that fails (another volume, say) falls back
/// to copying, and the old folder is then left as it was. `old` may be a symlink (the
/// sandbox's `~/Pictures` is one), which is followed.
pub fn migrate_dir(old: &Path, new: &Path) -> io::Result<bool> {
    if fs::symlink_metadata(new).is_ok() || !old.is_dir() {
        return Ok(false);
    }
    if let Some(parent) = new.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::rename(old, new).is_ok() {
        return Ok(true);
    }
    // Copy into a temp name first, so a copy cut short never looks like a finished one.
    let tmp = new.with_file_name(format!(".{}.migrating-{}", APP_DIR, std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let copied = copy_tree(old, &tmp).and_then(|()| fs::rename(&tmp, new));
    if copied.is_err() {
        let _ = fs::remove_dir_all(&tmp);
    }
    copied.map(|()| true)
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

/// Loads the config at `path`. A missing file is a first run and gives the defaults;
/// anything else that stops the file being read is an error.
pub fn load(path: &Path) -> Result<Sanitized, ConfigError> {
    match fs::read_to_string(path) {
        Ok(text) => Config::from_json(&text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Sanitized { config: Config::default(), issues: Vec::new() }),
        Err(e) => Err(ConfigError::Io(e)),
    }
}

/// Reads an old web-version `config.json`. Its web-only keys are dropped, and so is any
/// key that has since become invalid, so one stale value doesn't block the import.
pub fn import_legacy(path: &Path) -> Result<Sanitized, ConfigError> {
    Config::from_json(&fs::read_to_string(path).map_err(ConfigError::Io)?)
}

/// Writes `config` atomically: a temp file in the same folder, synced, then renamed over
/// the old one, so a crash or a reader (the watcher, a text editor) never sees half a file.
pub fn save(path: &Path, config: &Config) -> io::Result<()> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let name = path.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config path has no file name"))?;
    let tmp = dir.join(format!(".{}.tmp-{}", name.to_string_lossy(), std::process::id()));
    let mut text = serde_json::to_string_pretty(config).map_err(io::Error::other)?;
    text.push('\n');
    let written = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_moves_the_old_folder_when_the_new_one_is_missing() {
        let home = tempfile::tempdir().unwrap();
        let [(old, new), _] = renamed_dirs(home.path());
        assert!(old.ends_with("Library/Application Support/Vault Wallpaper"));
        assert!(new.ends_with("Library/Application Support/Constellate"));
        fs::create_dir_all(old.join("sub")).unwrap();
        fs::write(old.join("config.json"), "{\"accent\":\"#123456\"}").unwrap();
        fs::write(old.join("sub/layout.json"), "[]").unwrap();

        assert!(migrate_dir(&old, &new).unwrap());
        assert!(!old.exists());
        assert_eq!(fs::read_to_string(new.join("config.json")).unwrap(), "{\"accent\":\"#123456\"}");
        assert_eq!(fs::read_to_string(new.join("sub/layout.json")).unwrap(), "[]");
        // Runs again on every launch: now a no-op.
        assert!(!migrate_dir(&old, &new).unwrap());
    }

    #[test]
    fn migrate_leaves_an_existing_new_folder_alone() {
        let home = tempfile::tempdir().unwrap();
        let [(old, new), _] = renamed_dirs(home.path());
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(old.join("config.json"), "old").unwrap();
        fs::write(new.join("config.json"), "new").unwrap();

        assert!(!migrate_dir(&old, &new).unwrap());
        assert_eq!(fs::read_to_string(new.join("config.json")).unwrap(), "new");
        assert_eq!(fs::read_to_string(old.join("config.json")).unwrap(), "old");
    }

    #[test]
    fn migrate_without_an_old_folder_does_nothing() {
        let home = tempfile::tempdir().unwrap();
        let [_, (old, new)] = renamed_dirs(home.path());
        assert!(!migrate_dir(&old, &new).unwrap());
        assert!(!new.exists());
    }

    #[test]
    fn copy_fallback_copies_everything() {
        let from = tempfile::tempdir().unwrap();
        let to = tempfile::tempdir().unwrap();
        fs::create_dir_all(from.path().join("a/b")).unwrap();
        fs::write(from.path().join("a/b/c.json"), "x").unwrap();
        copy_tree(from.path(), &to.path().join("out")).unwrap();
        assert_eq!(fs::read_to_string(to.path().join("out/a/b/c.json")).unwrap(), "x");
    }

    #[test]
    fn default_path_uses_the_new_name() {
        let p = default_path();
        if let Some(p) = p {
            assert!(p.ends_with("Library/Application Support/Constellate/config.json"));
        }
    }
}
