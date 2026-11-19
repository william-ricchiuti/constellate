//! Logseq graphs: pages in `pages/` and `journals/`, named by file (`a___b.md` is the
//! page `a/b`) or `title::`, linked by `[[page]]`, `#tag`, `#[[multi word]]`, `tags::`
//! and block references `((uuid))`.

use std::sync::LazyLock;

use regex::Regex;

use super::markdown::{blank_code, percent_decode};

/// What a Logseq page says about itself and links to.
#[derive(Debug, Default, PartialEq)]
pub struct Page {
    /// `title::` (or `#+title:`), when set.
    pub title: Option<String>,
    pub aliases: Vec<String>,
    /// `tags::`, in order; also links.
    pub tags: Vec<String>,
    /// Pages linked by `[[…]]`, `#tag`, `#[[…]]` and `tags::`, by name, in order.
    pub links: Vec<String>,
    /// Block `id::` values on this page.
    pub ids: Vec<String>,
    /// Block references `((uuid))`.
    pub block_refs: Vec<String>,
}

static PROPERTY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[ \t]*(?:- )?([A-Za-z0-9_\-]+)::[ \t]*(.*?)[ \t]*$").expect("valid regex"));
static ORG_PROPERTY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^[ \t]*#\+([a-z_]+):[ \t]*(.*?)[ \t]*$").expect("valid regex"));
static BLOCK_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?mi)^[ \t]*(?:- )?(?:id::|:id:)[ \t]+([0-9a-f\-]{8,})[ \t]*$").expect("valid regex"));
static PAGE_REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(#?)\[\[([^\[\]]+)\]\]").expect("valid regex"));
static HASH_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?:^|[\s(])#([^\s#+\[\](),;!?"'{}][^\s#\[\](),;!?"'{}]*)"#).expect("valid regex"));
static BLOCK_REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(\(([0-9a-fA-F\-]{8,})\)\)").expect("valid regex"));

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// The page name a file stands for: `a___b` → `a/b`, legacy `a%2Fb` → `a/b`.
pub fn page_name(stem: &str) -> String {
    percent_decode(&stem.replace("___", "/"))
}

/// A journal file's date, `2024_01_31` → (2024, 1, 31).
pub fn journal_date(stem: &str) -> Option<(u32, u32, u32)> {
    let mut it = stem.split(['_', '-']);
    let (y, m, d) = (it.next()?, it.next()?, it.next()?);
    if it.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let (y, m, d) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
}

/// Names a journal page answers to: `2024-01-31` (its label) and Logseq's default
/// title format, `Jan 31st, 2024`.
pub fn journal_names((y, m, d): (u32, u32, u32)) -> [String; 2] {
    let suffix = match (d % 10, d) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    [format!("{y:04}-{m:02}-{d:02}"), format!("{} {d}{suffix}, {y}", MONTHS[m as usize - 1])]
}

/// Splits a property value like `a, [[b c]], #d` into names.
fn property_items(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    // `[[x, y]]` may hold commas, so take bracketed names out first.
    let mut rest = value.to_string();
    for m in PAGE_REF.captures_iter(value) {
        out.push(m[2].trim().to_string());
        rest = rest.replacen(&m[0], ",", 1);
    }
    for item in rest.split(',') {
        let item = item.trim().trim_start_matches('#').trim();
        if !item.is_empty() {
            out.push(item.to_string());
        }
    }
    out
}

fn push_unique(v: &mut Vec<String>, s: String) {
    if !s.is_empty() && !v.iter().any(|x| x.to_lowercase() == s.to_lowercase()) {
        v.push(s);
    }
}

/// Parses a page's text.
pub fn parse(content: &str) -> Page {
    let mut page = Page::default();
    let text = blank_code(content);
    // Page properties: the leading `key:: value` (or org `#+key:`) lines of the first block.
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            if page.title.is_none() && page.aliases.is_empty() && page.tags.is_empty() && i == 0 {
                continue;
            }
            break;
        }
        let caps = PROPERTY.captures(line).filter(|_| i == 0 || !line.trim_start().starts_with("- "));
        let Some(c) = caps.or_else(|| ORG_PROPERTY.captures(line)) else { break };
        let value = c[2].to_string();
        match c[1].to_ascii_lowercase().as_str() {
            "title" if !value.is_empty() => page.title = Some(value),
            "alias" | "aliases" => property_items(&value).into_iter().for_each(|a| push_unique(&mut page.aliases, a)),
            "tags" | "filetags" => property_items(&value).into_iter().for_each(|t| push_unique(&mut page.tags, t)),
            _ => {}
        }
    }
    let mut links = page.tags.clone();
    // `[[page]]` and `#[[page]]` in order, then `#tag`; both after `tags::`.
    for m in PAGE_REF.captures_iter(&text) {
        push_unique(&mut links, m[2].trim().to_string());
    }
    let no_refs = PAGE_REF.replace_all(&text, " ");
    for m in HASH_TAG.captures_iter(&no_refs) {
        let tag = m[1].trim_end_matches(['.', ':']);
        push_unique(&mut links, tag.to_string());
    }
    page.links = links;
    for m in BLOCK_ID.captures_iter(&text) {
        page.ids.push(m[1].to_ascii_lowercase());
    }
    for m in BLOCK_REF.captures_iter(&text) {
        page.block_refs.push(m[1].to_ascii_lowercase());
    }
    page
}

/// The namespace parents of a page name, nearest first: `a/b/c` → `a/b`, `a`.
pub fn namespace_parents(name: &str) -> impl Iterator<Item = &str> {
    name.char_indices().rev().filter(|&(_, c)| c == '/').map(move |(i, _)| &name[..i]).filter(|p| !p.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_names_from_files() {
        assert_eq!(page_name("a___b___c"), "a/b/c");
        assert_eq!(page_name("a%2Fb"), "a/b");
        assert_eq!(page_name("What%3F"), "What?");
        assert_eq!(page_name("plain"), "plain");
        assert_eq!(journal_date("2024_01_31"), Some((2024, 1, 31)));
        assert_eq!(journal_date("2024_13_01"), None);
        assert_eq!(journal_date("notes"), None);
        assert_eq!(journal_names((2024, 1, 31)), ["2024-01-31", "Jan 31st, 2024"]);
        assert_eq!(journal_names((2024, 3, 12))[1], "Mar 12th, 2024");
        assert_eq!(journal_names((2024, 3, 22))[1], "Mar 22nd, 2024");
    }

    #[test]
    fn properties_links_and_refs() {
        let text = "\
title:: My Page
alias:: Mine, [[My, Page]]
tags:: project, [[big idea]], #later
type:: note

- First block links [[Other Page]] and #tag and #[[multi word]].
  id:: 6512a3b0-1111-2222-3333-444455556666
- Ref ((6512A3B0-aaaa-bbbb-cccc-ddddeeeeffff)) and #done. and a url https://x.com/#frag
- `#not-a-tag [[not a link]]`
```
[[nor this]]
```
- props:: in a later block [[Later]]
";
        let p = parse(text);
        assert_eq!(p.title.as_deref(), Some("My Page"));
        assert_eq!(p.aliases, ["My, Page", "Mine"]);
        assert_eq!(p.tags, ["big idea", "project", "later"]);
        assert_eq!(
            p.links,
            ["big idea", "project", "later", "My, Page", "Other Page", "multi word", "tag", "done"]
        );
        assert_eq!(p.ids, ["6512a3b0-1111-2222-3333-444455556666"]);
        assert_eq!(p.block_refs, ["6512a3b0-aaaa-bbbb-cccc-ddddeeeeffff"]);
    }

    #[test]
    fn first_block_properties_and_org_pages() {
        let p = parse("- title:: In Block\n  tags:: x\n- body [[y]]");
        assert_eq!(p.title.as_deref(), Some("In Block"));
        assert_eq!(p.tags, ["x"]);
        let p = parse("#+title: Org Page\n#+alias: o\n* heading [[z]] #+BEGIN_SRC");
        assert_eq!(p.title.as_deref(), Some("Org Page"));
        assert_eq!(p.aliases, ["o"]);
        assert_eq!(p.links, ["z"]);
    }

    #[test]
    fn namespaces() {
        assert_eq!(namespace_parents("a/b/c").collect::<Vec<_>>(), ["a/b", "a"]);
        assert_eq!(namespace_parents("solo").count(), 0);
    }
}
