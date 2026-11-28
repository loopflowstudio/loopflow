//! Minimal HTML tag/attribute scanning, escaping, and path utilities for `lf page build`.

use std::collections::BTreeMap;
use std::path::{Component, Path};
use std::sync::LazyLock;

use anyhow::{bail, Result};
use regex::Regex;

static ATTR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)([a-zA-Z0-9_:-]+)\s*=\s*(?:"([^"]*)"|'([^']*)')"#).expect("static attr regex")
});
static H1_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<h1[^>]*>(.*?)</h1>").expect("static h1 regex"));
static HTML_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[^>]+>").expect("static tag strip regex"));

pub(super) struct ElementMatch {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) attrs: BTreeMap<String, String>,
    pub(super) inner: String,
}

pub(super) fn line_at(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset.min(text.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

pub(super) fn validate_repo_relative(path: &str, elem: &str, line: usize) -> Result<()> {
    let p = Path::new(path);
    if p.is_absolute()
        || p.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("line {line}: <{elem} path=\"{path}\"> must be a repository-relative path");
    }
    Ok(())
}

pub(super) fn find_next_element(
    input: &str,
    from: usize,
    tag_name: &str,
) -> Result<Option<ElementMatch>> {
    let close_needle = format!("</{tag_name}>");
    let open_len = tag_name.len() + 1;
    let Some(start) = find_open_tag_start(input, from, tag_name) else {
        return Ok(None);
    };
    let Some((open_end, self_closing, attr_str)) = scan_opening_tag_end(input, start + open_len)
    else {
        let line = line_at(input, start);
        bail!("line {line}: unclosed `<{tag_name}>` opening tag");
    };
    let attrs = parse_attributes(attr_str);
    if self_closing {
        return Ok(Some(ElementMatch {
            start,
            end: open_end,
            attrs,
            inner: String::new(),
        }));
    }

    // Track nesting depth in case of nested same-named elements (e.g. components).
    let mut depth = 1usize;
    let mut scan = open_end;
    while depth > 0 {
        let next_open = find_open_tag_start(input, scan, tag_name);
        let next_close = input[scan..].find(&close_needle).map(|off| scan + off);
        match (next_open, next_close) {
            (Some(o), Some(c)) if o < c => {
                let Some((nested_end, nested_self_closing, _)) =
                    scan_opening_tag_end(input, o + open_len)
                else {
                    let line = line_at(input, o);
                    bail!("line {line}: unclosed `<{tag_name}>` opening tag");
                };
                if !nested_self_closing {
                    depth += 1;
                }
                scan = nested_end;
            }
            (_, Some(c)) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(Some(ElementMatch {
                        start,
                        end: c + close_needle.len(),
                        attrs,
                        inner: input[open_end..c].to_owned(),
                    }));
                }
                scan = c + close_needle.len();
            }
            (_, None) => {
                let line = line_at(input, start);
                bail!("line {line}: missing closing `</{tag_name}>` tag");
            }
        }
    }
    Ok(None)
}

fn find_open_tag_start(input: &str, from: usize, tag_name: &str) -> Option<usize> {
    let needle = format!("<{tag_name}");
    let mut search = from;
    while let Some(rel) = input[search..].find(&needle) {
        let start = search + rel;
        let after = start + needle.len();
        if matches!(
            input.as_bytes().get(after).copied(),
            Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')
        ) {
            return Some(start);
        }
        search = after;
    }
    None
}

fn scan_opening_tag_end(input: &str, from: usize) -> Option<(usize, bool, &str)> {
    let bytes = input.as_bytes();
    let mut quote: Option<u8> = None;
    for (offset, &b) in bytes[from..].iter().enumerate() {
        let i = from + offset;
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
        } else if b == b'\'' || b == b'"' {
            quote = Some(b);
        } else if b == b'>' {
            let prev = input[from..i].trim_end();
            let self_closing = prev.ends_with('/');
            let attr_str = prev.strip_suffix('/').unwrap_or(prev).trim();
            return Some((i + 1, self_closing, attr_str));
        }
    }
    None
}

fn parse_attributes(attr_str: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for caps in ATTR_RE.captures_iter(attr_str) {
        let val = caps
            .get(2)
            .or_else(|| caps.get(3))
            .map(|m| unescape_html_attr(m.as_str()))
            .unwrap_or_default();
        map.insert(caps[1].to_owned(), val);
    }
    map
}

pub(super) fn enclosing_tag_name(input: &str, pos: usize) -> Option<&str> {
    let lt = input[..pos].rfind('<')?;
    let rest = input[lt + 1..pos].trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(rest.len());
    let name = &rest[..end];
    (!name.is_empty()).then_some(name)
}

pub(super) fn extract_title(body: &str) -> Option<String> {
    let caps = H1_RE.captures(body)?;
    let plain = HTML_TAG_RE.replace_all(&caps[1], "");
    let trimmed = plain.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub(super) fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(super) fn escape_single_quoted_attr(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('\'', "&#39;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(super) fn unescape_html_attr(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}
