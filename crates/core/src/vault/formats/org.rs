//! Org files (org-roam and plain org): `#+title:`, `#+filetags:`, heading tags,
//! `#+roam_aliases:`, `:ID:` properties and `[[id:…]]`, `[[file:…]]` and `[[Other]]` links.

use std::sync::LazyLock;

use regex::Regex;

use super::markdown::{PathLinks, resolve_link_path, unquote};

/// What an org file says about itself and links to.
#[derive(Debug, Default, PartialEq)]
pub struct Org {
    pub title: Option<String>,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    /// Every `:ID:` in the file (the file's own and its headings').
    pub ids: Vec<String>,
    pub id_links: Vec<String>,
    pub path_links: PathLinks,
    /// Plain `[[Other]]` and `[[roam:Other]]` links, by name.
    pub wikilinks: Vec<String>,
}

static KEYWORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?mi)^[ \t]*#\+([a-z_]+):[ \t]*(.*?)[ \t]*$").expect("valid regex"));
static HEADING_TAGS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\*+[ \t]+.*?[ \t]+(:[\w@#%:]+:)[ \t]*$").expect("valid regex"));
static ID_PROP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?mi)^[ \t]*:ID:[ \t]+(\S+)[ \t]*$").expect("valid regex"));
static LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\[\]]+)\](?:\[([^\[\]]*)\])?\]").expect("valid regex"));
static QUOTED_OR_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""([^"]*)"|(\S+)"#).expect("valid regex"));
static SCHEME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9+.\-]*:").expect("valid regex"));

/// Link targets that aren't other notes: headings, custom ids, code refs, bare paths.
const INTERNAL: [char; 6] = ['*', '#', '(', '/', '.', '~'];

fn push_unique(v: &mut Vec<String>, s: &str) {
    let s = s.trim();
    if !s.is_empty() && !v.iter().any(|x| x == s) {
        v.push(s.to_string());
    }
}

/// Parses an org file in folder `dir` (root-relative, "/"-separated).
pub fn parse(content: &str, dir: &str) -> Org {
    let mut org = Org::default();
    for m in KEYWORD.captures_iter(content) {
        let value = &m[2];
        match m[1].to_ascii_lowercase().as_str() {
            "title" if org.title.is_none() && !value.is_empty() => org.title = Some(value.to_string()),
            "filetags" => value.split([':', ' ', '\t']).for_each(|t| push_unique(&mut org.tags, t)),
            "roam_aliases" => {
                for a in QUOTED_OR_WORD.captures_iter(value) {
                    push_unique(&mut org.aliases, a.get(1).or(a.get(2)).map_or("", |m| m.as_str()));
                }
            }
            _ => {}
        }
    }
    for m in HEADING_TAGS.captures_iter(content) {
        m[1].split(':').for_each(|t| push_unique(&mut org.tags, t));
    }
    for m in ID_PROP.captures_iter(content) {
        push_unique(&mut org.ids, &m[1]);
    }
    for m in LINK.captures_iter(content) {
        let target = m[1].trim();
        let lower = target.to_ascii_lowercase();
        if let Some(id) = lower.strip_prefix("id:").map(|_| &target[3..]) {
            push_unique(&mut org.id_links, id);
        } else if lower.starts_with("file:") {
            // `file:x.org::search` → x.org
            let path = target[5..].split("::").next().unwrap_or("");
            if let Some((key, hard)) = resolve_link_path(dir, path) {
                if hard { org.path_links.hard.push(key) } else { org.path_links.soft.push(key) }
            }
        } else if lower.starts_with("roam:") {
            push_unique(&mut org.wikilinks, &target[5..]);
        } else if target.starts_with(INTERNAL) || SCHEME.is_match(target) {
            // Internal heading/custom-id/coderef targets, bare paths, and URLs.
        } else {
            org.wikilinks.push(target.to_string());
        }
    }
    if let Some(t) = &org.title {
        org.title = Some(unquote(t));
    }
    org
}

/// Links an org text holds (for detection): `[[…]]` links that aren't URLs.
pub fn count_links(content: &str) -> usize {
    LINK.captures_iter(content)
        .filter(|m| {
            let t = m[1].trim().to_ascii_lowercase();
            t.starts_with("id:")
                || t.starts_with("file:")
                || t.starts_with("roam:")
                || !(t.starts_with(INTERNAL) || SCHEME.is_match(&t))
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn org_roam_file() {
        let text = "\
:PROPERTIES:
:ID:       1111-aaaa
:END:
#+title: My Topic
#+FILETAGS: :project:work:
#+roam_aliases: \"Topic Alias\" Other
* Heading :urgent:work:
:PROPERTIES:
:ID: 2222-bbbb
:END:
See [[id:3333-cccc][Linked]] and [[file:sub/other.org][Other]] and [[file:../up.md]].
Also [[Plain Name]], [[roam:Roam Name]], [[https://example.com][web]], [[*Heading]], [[#custom]].
[[file:image.png]] [[file:notes::*Head]] [[id:3333-cccc]]
";
        let org = parse(text, "dir");
        assert_eq!(org.title.as_deref(), Some("My Topic"));
        assert_eq!(org.tags, ["project", "work", "urgent"]);
        assert_eq!(org.aliases, ["Topic Alias", "Other"]);
        assert_eq!(org.ids, ["1111-aaaa", "2222-bbbb"]);
        assert_eq!(org.id_links, ["3333-cccc"]);
        assert_eq!(org.path_links.hard, ["dir/sub/other", "up"]);
        assert_eq!(org.path_links.soft, ["dir/notes"]);
        assert_eq!(org.wikilinks, ["Plain Name", "Roam Name"]);
        assert_eq!(count_links(text), 8);
    }

    #[test]
    fn filetags_with_spaces_and_no_title() {
        let org = parse("#+filetags: a b\n* No tags here\n* Tagged   :x:\n", "");
        assert_eq!(org.title, None);
        assert_eq!(org.tags, ["a", "b", "x"]);
    }
}
