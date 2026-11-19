//! The "any folder" reader: every file is a node linked to its folder, and every folder
//! a node linked to its parent, so subfolders form branches around the root.
//!
//! Files are never opened: only `lstat` is used (size, modification time). A node's tag
//! is its file type, so colouring by tag colours by type; its `folder` is the top-level
//! folder it sits in, as for notes.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use rustc_hash::FxHashMap as HashMap;

use super::formats::mtime_ms;
use super::graph::{Graph, Link, Node, size_weight};
use super::scan::Entry;

/// Id of the root folder's node.
pub const ROOT_ID: &str = "/";
/// Tag of folder nodes.
pub const FOLDER_TYPE: &str = "folder";

/// The file type shown as a node's tag, from the extension.
pub fn file_type(name: &str) -> &'static str {
    let ext = match name.rfind('.') {
        Some(i) if i > 0 => name[i + 1..].to_ascii_lowercase(),
        _ => return "other",
    };
    match ext.as_str() {
        "md" | "markdown" | "org" | "txt" | "rst" | "adoc" | "tex" => "text",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "heif" | "tif" | "tiff" | "bmp" | "svg" | "avif" | "raw"
        | "cr2" | "nef" | "dng" | "psd" | "ico" => "image",
        "mp4" | "mov" | "m4v" | "avi" | "mkv" | "webm" => "video",
        "mp3" | "m4a" | "wav" | "aiff" | "flac" | "ogg" | "aac" => "audio",
        "pdf" | "doc" | "docx" | "pages" | "rtf" | "odt" | "epub" | "key" | "ppt" | "pptx" | "numbers" | "xls"
        | "xlsx" | "ods" => "document",
        "json" | "csv" | "tsv" | "xml" | "yaml" | "yml" | "toml" | "sqlite" | "db" | "parquet" | "plist" => "data",
        "zip" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "dmg" | "tar" | "pkg" | "iso" => "archive",
        "rs" | "js" | "mjs" | "ts" | "tsx" | "jsx" | "py" | "rb" | "go" | "c" | "h" | "cc" | "cpp" | "hpp" | "m"
        | "mm" | "swift" | "java" | "kt" | "cs" | "php" | "sh" | "zsh" | "bash" | "lua" | "sql" | "html" | "css"
        | "scss" | "vue" | "svelte" | "zig" | "hs" | "ex" | "exs" | "clj" | "edn" | "el" | "wgsl" | "metal" => "code",
        _ => "other",
    }
}

/// One file's entry: name, size and time, no contents.
pub fn stat_entry(path: &Path) -> Option<Entry> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let name = path.file_name()?.to_str()?.to_string();
    Some(Entry {
        path: path.to_path_buf(),
        tags: vec![file_type(&name).to_string()],
        basename: name,
        mtime_ms: mtime_ms(&meta),
        wikilinks: Vec::new(),
        size: meta.len(),
        extra: None,
    })
}

/// The tree as a graph. Ids are root-relative paths with "/" (the root is [`ROOT_ID`]);
/// labels are file and folder names. Folders take the newest time and the total size
/// of what they hold.
pub fn materialize<'a>(entries: impl IntoIterator<Item = &'a Entry>, root: &Path, size_by_file: bool) -> Graph {
    let mut files: Vec<(String, &Entry)> = entries
        .into_iter()
        .filter_map(|e| {
            let rel = e.path.strip_prefix(root).ok()?.to_str()?;
            Some((rel.replace(std::path::MAIN_SEPARATOR, "/"), e))
        })
        .collect();
    files.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    struct Dir {
        mtime: f64,
        size: u64,
    }
    // Every folder that holds a file, with its aggregates; BTreeMap keeps output sorted.
    let mut dirs: BTreeMap<String, Dir> = BTreeMap::new();
    for (rel, e) in &files {
        let mut cut = rel.len();
        while let Some(i) = rel[..cut].rfind('/') {
            let d = dirs.entry(rel[..i].to_string()).or_insert(Dir { mtime: 0.0, size: 0 });
            d.mtime = d.mtime.max(e.mtime_ms);
            d.size += e.size;
            cut = i;
        }
    }
    let root_mtime = files.iter().map(|(_, e)| e.mtime_ms).fold(0.0, f64::max);
    let root_size: u64 = files.iter().map(|(_, e)| e.size).sum();

    let top = |rel: &str| rel.split_once('/').map(|(t, _)| t.to_string());
    let parent = |rel: &str| rel.rfind('/').map_or(ROOT_ID.to_string(), |i| rel[..i].to_string());
    let name = |rel: &str| rel.rsplit('/').next().unwrap_or(rel).to_string();
    let weight = |bytes: u64| size_by_file.then(|| size_weight(bytes));

    let root_name = root.file_name().and_then(|n| n.to_str()).unwrap_or("Folder").to_string();
    let mut nodes = Vec::with_capacity(files.len() + dirs.len() + 1);
    let mut links = Vec::with_capacity(files.len() + dirs.len());
    let mut discovered_tags: BTreeMap<String, usize> = BTreeMap::new();
    nodes.push(Node {
        id: ROOT_ID.into(),
        label: Some(root_name),
        tag: Some(FOLDER_TYPE.into()),
        mtime: (root_mtime != 0.0).then_some(root_mtime),
        weight: weight(root_size),
        ..Node::default()
    });
    for (rel, d) in &dirs {
        nodes.push(Node {
            id: rel.clone(),
            label: Some(name(rel)),
            tag: Some(FOLDER_TYPE.into()),
            mtime: (d.mtime != 0.0).then_some(d.mtime),
            // A top-level folder colours like its contents.
            folder: top(rel).or_else(|| Some(rel.clone())),
            weight: weight(d.size),
            ..Node::default()
        });
        links.push(Link { source: rel.clone(), target: parent(rel) });
    }
    if !dirs.is_empty() {
        *discovered_tags.entry(FOLDER_TYPE.into()).or_default() += dirs.len() + 1;
    }
    let mut by_type: HashMap<&str, usize> = HashMap::default();
    for (rel, e) in &files {
        let t = e.tags.first().map_or("other", String::as_str);
        *by_type.entry(t).or_default() += 1;
        nodes.push(Node {
            id: rel.clone(),
            label: Some(e.basename.clone()),
            tag: Some(t.to_string()),
            mtime: (e.mtime_ms != 0.0).then_some(e.mtime_ms),
            folder: top(rel),
            weight: weight(e.size),
            ..Node::default()
        });
        links.push(Link { source: rel.clone(), target: parent(rel) });
    }
    for (t, n) in by_type {
        *discovered_tags.entry(t.to_string()).or_default() += n;
    }
    Graph { tagged: nodes.len(), nodes, links, discovered_tags, capped: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_by_extension() {
        assert_eq!(file_type("a.JPG"), "image");
        assert_eq!(file_type("main.rs"), "code");
        assert_eq!(file_type("notes.md"), "text");
        assert_eq!(file_type("Makefile"), "other");
        assert_eq!(file_type(".hidden"), "other");
        assert_eq!(file_type("x.tar.gz"), "archive");
    }

    #[test]
    fn tree_becomes_branches() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("photos/2024")).unwrap();
        fs::write(root.join("readme.md"), "hi").unwrap();
        fs::write(root.join("photos/2024/a.jpg"), vec![0u8; 2048]).unwrap();
        fs::write(root.join("photos/b.png"), "x").unwrap();
        let entries: Vec<Entry> =
            ["readme.md", "photos/2024/a.jpg", "photos/b.png"].iter().map(|r| stat_entry(&root.join(r)).unwrap()).collect();
        let g = materialize(&entries, root, true);
        let ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["/", "photos", "photos/2024", "photos/2024/a.jpg", "photos/b.png", "readme.md"]);
        let link = |s: &str, t: &str| g.links.iter().any(|l| l.source == s && l.target == t);
        assert!(link("photos", "/") && link("photos/2024", "photos") && link("photos/2024/a.jpg", "photos/2024"));
        assert!(link("readme.md", "/"));
        assert_eq!(g.links.len(), 5);
        let node = |id: &str| g.nodes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(node("photos/2024/a.jpg").tag.as_deref(), Some("image"));
        assert_eq!(node("photos/2024/a.jpg").folder.as_deref(), Some("photos"));
        assert_eq!(node("photos").folder.as_deref(), Some("photos"));
        assert_eq!(node("readme.md").folder, None);
        assert_eq!(node("photos/2024/a.jpg").label.as_deref(), Some("a.jpg"));
        // Folders weigh what they hold.
        assert!(node("photos").weight.unwrap() > node("photos/b.png").weight.unwrap());
        assert_eq!(g.discovered_tags.get("folder"), Some(&3));
        assert!(materialize(&entries, root, false).nodes.iter().all(|n| n.weight.is_none()));
    }
}
