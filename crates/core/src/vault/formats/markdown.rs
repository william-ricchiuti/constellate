//! Markdown beyond wikilinks: standard links `[text](path.md)`, reference definitions
//! `[id]: path.md`, frontmatter `title:` and `aliases:`, and blanking out code so links
//! and tags inside it don't count.

use std::sync::LazyLock;

use regex::Regex;

use super::super::kind::{is_note_name, note_stem};

/// Links found by path, already resolved to root-relative, "/"-separated keys without
/// the note extension.
#[derive(Debug, Default, PartialEq)]
pub struct PathLinks {
    /// Links that name a note file (`x.md`, `x.org`): a ghost when it is missing.
    pub hard: Vec<String>,
    /// Links without an extension (`[x](other)`): count only when a note answers.
    pub soft: Vec<String>,
}

impl PathLinks {
    pub fn len(&self) -> usize {
        self.hard.len() + self.soft.len()
    }
}

static INLINE_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(!?)\[(?:[^\[\]]|\[[^\[\]]*\])*\]\([ \t]*(<[^<>\n]*>|[^\s()<>]+)(?:[ \t]+(?:"[^"\n]*"|'[^'\n]*'|\([^()\n]*\)))?[ \t]*\)"#)
        .expect("valid regex")
});
static REF_DEF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^[ ]{0,3}\[([^\]\n]+)\]:[ \t]*(<[^<>\n]*>|\S+)").expect("valid regex")
});
static SCHEME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9+.\-]*:").expect("valid regex"));
static EXTENSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.[A-Za-z0-9]{1,8}$").expect("valid regex"));

/// Decodes `%XX` escapes (as UTF-8); anything malformed is kept as written.
pub fn percent_decode(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|d| d as u8)
}

/// Resolves a link destination written in a note in folder `dir` (root-relative,
/// "/"-separated, "" at the root) to a root-relative key without the note extension.
/// `None` for URLs, anchors, links leaving the root and links to files that aren't notes.
/// The flag says whether the link named a note file (true) or had no extension (false).
pub fn resolve_link_path(dir: &str, dest: &str) -> Option<(String, bool)> {
    let dest = dest.trim();
    let dest = dest.strip_prefix('<').and_then(|d| d.strip_suffix('>')).unwrap_or(dest);
    if dest.starts_with("//") || SCHEME.is_match(dest) {
        return None;
    }
    let dest = dest.split(['#', '?']).next().unwrap_or("");
    let dest = percent_decode(dest);
    if dest.is_empty() {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    if !dest.starts_with('/') {
        parts.extend(dir.split('/').filter(|s| !s.is_empty()));
    }
    for seg in dest.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    let last = parts.pop()?;
    let hard = if is_note_name(last) {
        true
    } else if EXTENSION.is_match(last) {
        return None;
    } else {
        false
    };
    let stem = note_stem(last);
    let mut key = parts.join("/");
    if !key.is_empty() {
        key.push('/');
    }
    key.push_str(stem);
    Some((key, hard))
}

/// Markdown links and reference definitions to notes, in order. Images are skipped.
pub fn extract_path_links(content: &str, dir: &str) -> PathLinks {
    let mut out = PathLinks::default();
    let mut push = |dest: &str| {
        if let Some((key, hard)) = resolve_link_path(dir, dest) {
            if hard { out.hard.push(key) } else { out.soft.push(key) }
        }
    };
    for m in INLINE_LINK.captures_iter(content) {
        if &m[1] != "!" {
            push(&m[2]);
        }
    }
    for m in REF_DEF.captures_iter(content) {
        // `[^1]: …` is a footnote, not a link.
        if !m[1].starts_with('^') {
            push(&m[2]);
        }
    }
    out
}

/// The text with fenced code blocks and inline code spans replaced by spaces (newlines
/// kept), so links and tags inside code don't count.
pub fn blank_code(content: &str) -> String {
    if !content.contains('`') && !content.contains("~~~") {
        return content.to_string();
    }
    let mut out = String::with_capacity(content.len());
    let mut fence: Option<(char, usize)> = None;
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim_start_matches(' ');
        let indent = line.len() - trimmed.len();
        let run = |c: char| trimmed.chars().take_while(|&x| x == c).count();
        match fence {
            Some((c, n)) => {
                let r = run(c);
                if indent <= 3 && r >= n && trimmed[r * c.len_utf8()..].trim().is_empty() {
                    fence = None;
                }
                out.push_str(&blank(line));
            }
            None => {
                let opener = if indent <= 3 {
                    ['`', '~'].into_iter().map(|c| (c, run(c))).find(|&(_, r)| r >= 3)
                } else {
                    None
                };
                if let Some((c, r)) = opener
                    && !(c == '`' && trimmed[r..].contains('`'))
                {
                    fence = Some((c, r));
                    out.push_str(&blank(line));
                } else {
                    out.push_str(&blank_inline_code(line));
                }
            }
        }
    }
    out
}

fn blank(s: &str) -> String {
    s.chars().map(|c| if c == '\n' || c == '\r' { c } else { ' ' }).collect()
}

/// Blanks `code` spans on one line: a run of n backticks up to the next run of exactly n.
fn blank_inline_code(line: &str) -> String {
    if !line.contains('`') {
        return line.to_string();
    }
    let b = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    let mut copied = 0;
    while i < b.len() {
        if b[i] != b'`' {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && b[i] == b'`' {
            i += 1;
        }
        let n = i - start;
        // Find the closing run of exactly n backticks.
        let mut j = i;
        let mut close = None;
        while j < b.len() {
            if b[j] == b'`' {
                let s = j;
                while j < b.len() && b[j] == b'`' {
                    j += 1;
                }
                if j - s == n {
                    close = Some(j);
                    break;
                }
            } else {
                j += 1;
            }
        }
        if let Some(end) = close {
            out.push_str(&line[copied..start]);
            out.push_str(&" ".repeat(line[start..end].chars().count()));
            copied = end;
            i = end;
        }
    }
    out.push_str(&line[copied..]);
    out
}

/// The YAML frontmatter between the leading `---` lines, if any.
pub fn frontmatter(content: &str) -> Option<&str> {
    let rest = content.strip_prefix("---\n").or_else(|| content.strip_prefix("---\r\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some(&rest[..offset]);
        }
        offset += line.len();
    }
    None
}

/// A frontmatter key's value: a scalar, a flow list `[a, b]` or a block list of `- a`
/// lines. Quotes are removed; empty items dropped.
pub fn fm_values(fm: &str, key: &str) -> Vec<String> {
    let lines: Vec<&str> = fm.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let Some(rest) = line.strip_prefix(key).and_then(|r| r.strip_prefix(':')) else { continue };
        let rest = rest.trim();
        if let Some(inner) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            return inner.split(',').map(unquote).filter(|s| !s.is_empty()).collect();
        }
        if !rest.is_empty() {
            let v = unquote(rest);
            return if v.is_empty() { Vec::new() } else { vec![v] };
        }
        return lines[i + 1..]
            .iter()
            .map_while(|l| l.trim_start().strip_prefix("- ").or_else(|| (l.trim() == "-").then_some("")))
            .map(unquote)
            .filter(|s| !s.is_empty())
            .collect();
    }
    Vec::new()
}

pub fn unquote(s: &str) -> String {
    let s = s.trim();
    let s = s
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .or_else(|| s.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')))
        .unwrap_or(s);
    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn links(content: &str, dir: &str) -> (Vec<String>, Vec<String>) {
        let l = extract_path_links(content, dir);
        (l.hard, l.soft)
    }

    #[test]
    fn inline_links_resolve_against_the_note_folder() {
        let (hard, soft) = links(
            "[a](b.md) [up](../c.md) [root](/d/e.md) [sp](<f g.md>) [enc](h%20i.md) \
             [anchor](j.md#sec) [q](k.md?x=1) [title](l.md \"T\") [noext](m) [org](n.org) [mk](o.markdown)",
            "x/y",
        );
        assert_eq!(hard, ["x/y/b", "x/c", "d/e", "x/y/f g", "x/y/h i", "x/y/j", "x/y/k", "x/y/l", "x/y/n", "x/y/o"]);
        assert_eq!(soft, ["x/y/m"]);
    }

    #[test]
    fn skips_images_urls_files_and_escapes() {
        let (hard, soft) = links(
            "![img](p.md) [web](https://x.com/a.md) [mail](mailto:a@b.c) [pdf](doc.pdf) \
             [png](pic.PNG) [anchor](#top) [out](../../z.md) [proto](//cdn/x.md) [ok](ok.md)",
            "a",
        );
        assert_eq!(hard, ["a/ok"]);
        assert!(soft.is_empty());
    }

    #[test]
    fn reference_definitions_but_not_footnotes() {
        let (hard, soft) = links("text [x]\n\n[x]: other.md \"Other\"\n[y]: <sub/y z.md>\n[^1]: note.md\n[w]: wiki\n", "");
        assert_eq!(hard, ["other", "sub/y z"]);
        assert_eq!(soft, ["wiki"]);
    }

    #[test]
    fn nested_brackets_in_link_text() {
        assert_eq!(links("[see [this]](a.md)", "").0, ["a"]);
    }

    #[test]
    fn code_is_blanked() {
        let text = "a [x](x.md)\n```\n[y](y.md) #tag\n```\nb `[z](z.md)` c ``[[w]] ` `` [v](v.md)\n~~~md\n[[u]]\n~~~\n";
        let blanked = blank_code(text);
        assert_eq!(blanked.len(), text.len());
        assert_eq!(links(&blanked, "").0, ["x", "v"]);
        assert!(!blanked.contains("[[w]]") && !blanked.contains("[[u]]") && !blanked.contains("#tag"));
        // An unclosed backtick is just text.
        assert_eq!(blank_code("a ` [b](b.md)"), "a ` [b](b.md)");
    }

    #[test]
    fn percent_decoding() {
        assert_eq!(percent_decode("a%20b%2Fc"), "a b/c");
        assert_eq!(percent_decode("caf%C3%A9"), "café");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn frontmatter_values() {
        let fm = frontmatter("---\ntitle: \"My Note\"\naliases: [one, 'two']\nalias:\n  - three\n  - \"four\"\nx: y\n---\nbody").unwrap();
        assert_eq!(fm_values(fm, "title"), ["My Note"]);
        assert_eq!(fm_values(fm, "aliases"), ["one", "two"]);
        assert_eq!(fm_values(fm, "alias"), ["three", "four"]);
        assert!(fm_values(fm, "missing").is_empty());
        assert!(frontmatter("no\n---\n").is_none());
        assert_eq!(frontmatter("---\r\na: 1\r\n---\r\n"), Some("a: 1\r\n"));
    }
}
