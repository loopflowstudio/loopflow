//! Markdown ↔ Notion blocks and rich text, ported from the provider removed in
//! 63b39a6cd2 (`notion_blocks.rs`).
//!
//! Notion stores blocks exactly as sent, so the conversion is line-faithful: each
//! markdown line or construct is one block, a blank line is an empty paragraph,
//! and reading back joins blocks with newlines. Markdown Notion cannot hold
//! (`* ` bullets, list numbering, h4+, unknown code languages, nesting deeper
//! than one request accepts) normalizes once; converting again changes nothing.

use serde_json::{json, Value};

/// Notion caps one text object's content.
const TEXT_LIMIT: usize = 2000;
/// One request accepts two levels of nested children below a top-level block.
const NESTING_LIMIT: usize = 2;

pub(crate) fn to_blocks(markdown: &str) -> Vec<Value> {
    let lines: Vec<&str> = markdown.trim_end().lines().collect();
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        if let Some(language) = trimmed.strip_prefix("```") {
            index += 1;
            let mut code = Vec::new();
            while index < lines.len() && !lines[index].trim_start().starts_with("```") {
                code.push(lines[index]);
                index += 1;
            }
            index += 1;
            blocks.push(json!({
                "type": "code",
                "code": {
                    "language": code_language(language.trim()),
                    "rich_text": plain(&code.join("\n")),
                }
            }));
            continue;
        }
        if let Some((level, content)) = heading(line) {
            blocks.push(text_block(&format!("heading_{level}"), spans(content)));
            index += 1;
            continue;
        }
        if matches!(line.trim_end(), "---" | "***" | "___") {
            blocks.push(json!({"type": "divider", "divider": {}}));
            index += 1;
            continue;
        }
        if line.starts_with('>') {
            let mut quoted = Vec::new();
            while index < lines.len() && lines[index].starts_with('>') {
                let content = &lines[index][1..];
                quoted.push(content.strip_prefix(' ').unwrap_or(content));
                index += 1;
            }
            blocks.push(text_block("quote", spans(&quoted.join("\n"))));
            continue;
        }
        if list_marker(trimmed).is_some() {
            blocks.extend(parse_items(&lines, &mut index, indent_of(line), 0));
            continue;
        }
        blocks.push(text_block("paragraph", spans(line)));
        index += 1;
    }
    blocks
}

/// Read a page's block tree (children attached under each block's `children`).
pub(crate) fn to_markdown(blocks: &[Value]) -> String {
    let mut lines = Vec::new();
    render_blocks(blocks, 0, &mut lines);
    lines.join("\n").trim_end().to_string()
}

/// Inline markdown as Notion rich text: comments and block content.
pub(crate) fn spans(text: &str) -> Vec<Value> {
    let mut out = Vec::new();
    push_inline(text, Style::default(), None, &mut out);
    out
}

/// Rich text read back as inline markdown.
pub(crate) fn rich_text_markdown(rich_text: &[Value]) -> String {
    rich_text.iter().map(render_span).collect()
}

/// Rich text as its plain characters: titles and code.
pub(crate) fn plain_text(rich_text: &[Value]) -> String {
    rich_text
        .iter()
        .filter_map(|span| {
            span["plain_text"]
                .as_str()
                .or(span["text"]["content"].as_str())
        })
        .collect()
}

/// Literal text, split to Notion's per-object limit.
pub(crate) fn plain(text: &str) -> Vec<Value> {
    let mut out = Vec::new();
    push_text(text, Style::default(), None, &mut out);
    out
}

// ── Markdown blocks ─────────────────────────────────────────────────

fn heading(line: &str) -> Option<(usize, &str)> {
    let hashes = line.chars().take_while(|ch| *ch == '#').count();
    let content = line[hashes..].strip_prefix(' ')?;
    // Notion has three heading levels.
    (hashes > 0).then_some((hashes.min(3), content))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemKind {
    Bulleted,
    Numbered,
    Todo(bool),
}

fn list_marker(line: &str) -> Option<(ItemKind, &str)> {
    for (marker, checked) in [("- [ ] ", false), ("- [x] ", true), ("- [X] ", true)] {
        if let Some(content) = line.strip_prefix(marker) {
            return Some((ItemKind::Todo(checked), content));
        }
    }
    if let Some(content) = ["- ", "* ", "+ "]
        .iter()
        .find_map(|marker| line.strip_prefix(marker))
    {
        return Some((ItemKind::Bulleted, content));
    }
    let digits = line.chars().take_while(|ch| ch.is_ascii_digit()).count();
    let content = line[digits..].strip_prefix(". ")?;
    (digits > 0).then_some((ItemKind::Numbered, content))
}

fn indent_of(line: &str) -> usize {
    line.chars().take_while(|ch| *ch == ' ').count()
}

/// Items at `indent`. Deeper markers nest under the item above, up to Notion's
/// nesting limit; deeper plain lines, and markers past the limit, continue it.
fn parse_items(lines: &[&str], index: &mut usize, indent: usize, depth: usize) -> Vec<Value> {
    let mut items: Vec<(ItemKind, Vec<String>, Vec<Value>)> = Vec::new();
    while *index < lines.len() {
        let line = lines[*index];
        let current = indent_of(line);
        let trimmed = line.trim_start();
        if trimmed.is_empty() || current < indent {
            break;
        }
        match (list_marker(trimmed), items.last_mut()) {
            (Some((kind, content)), _) if current == indent => {
                items.push((kind, vec![content.to_string()], Vec::new()));
                *index += 1;
            }
            (Some(_), Some(item)) if depth < NESTING_LIMIT => {
                let children = parse_items(lines, index, current, depth + 1);
                item.2.extend(children);
            }
            (_, Some(item)) if current > indent => {
                item.1.push(trimmed.to_string());
                *index += 1;
            }
            _ => break,
        }
    }
    items
        .into_iter()
        .map(|(kind, lines, children)| {
            let (block_type, checked) = match kind {
                ItemKind::Bulleted => ("bulleted_list_item", None),
                ItemKind::Numbered => ("numbered_list_item", None),
                ItemKind::Todo(checked) => ("to_do", Some(checked)),
            };
            let mut block = text_block(block_type, spans(&lines.join("\n")));
            if let Some(checked) = checked {
                block[block_type]["checked"] = json!(checked);
            }
            if !children.is_empty() {
                block[block_type]["children"] = json!(children);
            }
            block
        })
        .collect()
}

fn text_block(block_type: &str, rich_text: Vec<Value>) -> Value {
    json!({"type": block_type, block_type: {"rich_text": rich_text}})
}

/// Notion accepts a fixed language list; the rest is plain text.
fn code_language(language: &str) -> &'static str {
    const LANGUAGES: &[&str] = &[
        "bash",
        "c",
        "c#",
        "c++",
        "css",
        "dart",
        "diff",
        "docker",
        "elixir",
        "go",
        "graphql",
        "haskell",
        "html",
        "java",
        "javascript",
        "json",
        "kotlin",
        "lua",
        "makefile",
        "markdown",
        "nix",
        "python",
        "ruby",
        "rust",
        "scala",
        "shell",
        "sql",
        "swift",
        "typescript",
        "xml",
        "yaml",
    ];
    let language = language.to_ascii_lowercase();
    let language = match language.as_str() {
        "sh" | "zsh" | "console" => "shell",
        "js" => "javascript",
        "ts" => "typescript",
        "py" => "python",
        "rs" => "rust",
        "yml" => "yaml",
        other => other,
    };
    LANGUAGES
        .iter()
        .find(|known| **known == language)
        .copied()
        .unwrap_or("plain text")
}

// ── Markdown inline ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default)]
struct Style {
    bold: bool,
    italic: bool,
    strikethrough: bool,
    code: bool,
}

fn push_inline(text: &str, style: Style, link: Option<&str>, out: &mut Vec<Value>) {
    let mut rest = text;
    while !rest.is_empty() {
        if let Some((inner, consumed)) = delimited(rest, "`") {
            push_text(
                inner,
                Style {
                    code: true,
                    ..style
                },
                link,
                out,
            );
            rest = &rest[consumed..];
            continue;
        }
        if let Some((inner, consumed)) = delimited(rest, "**") {
            push_inline(
                inner,
                Style {
                    bold: true,
                    ..style
                },
                link,
                out,
            );
            rest = &rest[consumed..];
            continue;
        }
        if let Some((inner, consumed)) = delimited(rest, "~~") {
            let strike = Style {
                strikethrough: true,
                ..style
            };
            push_inline(inner, strike, link, out);
            rest = &rest[consumed..];
            continue;
        }
        if let Some((inner, consumed)) = delimited(rest, "*") {
            push_inline(
                inner,
                Style {
                    italic: true,
                    ..style
                },
                link,
                out,
            );
            rest = &rest[consumed..];
            continue;
        }
        if let Some((label, url, consumed)) = markdown_link(rest) {
            push_inline(label, style, Some(url), out);
            rest = &rest[consumed..];
            continue;
        }
        let first = rest.chars().next().map_or(1, char::len_utf8);
        let end = ["**", "~~", "`", "*", "["]
            .iter()
            .filter_map(|needle| rest[first..].find(needle).map(|at| at + first))
            .min()
            .unwrap_or(rest.len());
        push_text(&rest[..end], style, link, out);
        rest = &rest[end..];
    }
}

fn push_text(text: &str, style: Style, link: Option<&str>, out: &mut Vec<Value>) {
    let mut rest = text;
    while !rest.is_empty() {
        let mut end = rest.len().min(TEXT_LIMIT);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        out.push(json!({
            "type": "text",
            "text": {"content": &rest[..end], "link": link.map(|url| json!({"url": url}))},
            "annotations": {
                "bold": style.bold,
                "italic": style.italic,
                "strikethrough": style.strikethrough,
                "underline": false,
                "code": style.code,
                "color": "default",
            },
        }));
        rest = &rest[end..];
    }
}

fn delimited<'a>(text: &'a str, delimiter: &str) -> Option<(&'a str, usize)> {
    let remainder = text.strip_prefix(delimiter)?;
    let end = remainder.find(delimiter)?;
    (end > 0).then(|| (&remainder[..end], 2 * delimiter.len() + end))
}

fn markdown_link(text: &str) -> Option<(&str, &str, usize)> {
    let label_end = text.strip_prefix('[')?.find("](")? + 1;
    let after = &text[label_end + 2..];
    let url_end = after.find(')')?;
    Some((
        &text[1..label_end],
        &after[..url_end],
        label_end + 3 + url_end,
    ))
}

fn render_span(span: &Value) -> String {
    let text = span["plain_text"]
        .as_str()
        .or(span["text"]["content"].as_str())
        .unwrap_or_default();
    if text.is_empty() {
        return String::new();
    }
    let annotations = &span["annotations"];
    let on = |name: &str| annotations[name].as_bool().unwrap_or(false);
    let mut rendered = text.to_string();
    if on("code") {
        rendered = format!("`{rendered}`");
    }
    for (name, marker) in [("bold", "**"), ("italic", "*"), ("strikethrough", "~~")] {
        if on(name) {
            rendered = format!("{marker}{rendered}{marker}");
        }
    }
    let href = span["href"]
        .as_str()
        .or(span["text"]["link"]["url"].as_str());
    match href {
        Some(url) if url != text => format!("[{rendered}]({url})"),
        _ => rendered,
    }
}

// ── Notion blocks ───────────────────────────────────────────────────

fn render_blocks(blocks: &[Value], indent: usize, lines: &mut Vec<String>) {
    let pad = " ".repeat(indent);
    let mut number = 0;
    for block in blocks {
        let block_type = block["type"].as_str().unwrap_or_default();
        let data = &block[block_type];
        let rich_text = data["rich_text"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let text = rich_text_markdown(rich_text);
        number = if block_type == "numbered_list_item" {
            number + 1
        } else {
            0
        };
        let marker = match block_type {
            "bulleted_list_item" => Some("- ".to_string()),
            "numbered_list_item" => Some(format!("{number}. ")),
            "to_do" if data["checked"].as_bool() == Some(true) => Some("- [x] ".to_string()),
            "to_do" => Some("- [ ] ".to_string()),
            _ => None,
        };
        if let Some(marker) = marker {
            let continuation = format!("\n{pad}{}", " ".repeat(marker.len()));
            lines.push(format!(
                "{pad}{marker}{}",
                text.replace('\n', &continuation)
            ));
            if let Some(children) = data["children"].as_array() {
                render_blocks(children, indent + marker.len(), lines);
            }
            continue;
        }
        match block_type {
            "heading_1" => lines.push(format!("{pad}# {text}")),
            "heading_2" => lines.push(format!("{pad}## {text}")),
            "heading_3" => lines.push(format!("{pad}### {text}")),
            "divider" => lines.push(format!("{pad}---")),
            "code" => {
                let language = data["language"]
                    .as_str()
                    .filter(|language| *language != "plain text")
                    .unwrap_or_default();
                lines.push(format!(
                    "{pad}```{language}\n{}\n{pad}```",
                    plain_text(rich_text)
                ));
            }
            "quote" => lines.push(
                text.split('\n')
                    .map(|line| format!("{pad}> {line}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            // Paragraphs, and any other block's text, read as text lines;
            // blocks without text (images, embeds) have no markdown form.
            _ if block_type == "paragraph" || !rich_text.is_empty() => {
                lines.push(format!("{pad}{text}"))
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RICH: &str = "# Plan\n\nShip **bold**, *italic*, `a<b`, ~~old~~ and [docs](https://example.com?a=1&b=2).\nSecond line.\n\n- one\n  - nested\n    - [x] deep\n- two\n  continued\n1. first\n2. second\n- [ ] todo\nafter\n\n```rust\nfn main() {\n    x < 1\n}\n```\n\n> quoted\n> again\n---\n### Sub\nend";

    /// What Notion returns: the blocks as sent, with `plain_text` and `href` filled.
    fn stored(blocks: &[Value]) -> Vec<Value> {
        blocks
            .iter()
            .map(|block| {
                let mut block = block.clone();
                let block_type = block["type"].as_str().unwrap().to_string();
                if let Some(rich_text) = block[&block_type]["rich_text"].as_array_mut() {
                    for span in rich_text {
                        span["plain_text"] = span["text"]["content"].clone();
                        span["href"] = span["text"]["link"]["url"].clone();
                    }
                }
                if let Some(children) = block[&block_type]["children"].as_array() {
                    block[&block_type]["children"] = json!(stored(children));
                }
                block
            })
            .collect()
    }

    #[test]
    fn supported_markdown_round_trips_exactly() {
        let blocks = to_blocks(RICH);
        assert_eq!(blocks[0]["type"], "heading_1");
        assert_eq!(blocks[1]["paragraph"]["rich_text"], json!([]));
        assert_eq!(
            blocks[5]["bulleted_list_item"]["children"][0]["bulleted_list_item"]["children"][0]
                ["to_do"]["checked"],
            true
        );
        assert_eq!(to_markdown(&stored(&blocks)), RICH);
    }

    #[test]
    fn unsupported_markdown_normalizes_once() {
        let markdown =
            "#### Deep\n* star\n10. ten\n```toml\nx = 1\n```\n- a\n  - b\n    - c\n      - d";
        let normalized = to_markdown(&stored(&to_blocks(markdown)));
        // `d` is past the nesting limit, so it continues `c` and reads back unchanged.
        assert_eq!(
            normalized,
            "### Deep\n- star\n1. ten\n```\nx = 1\n```\n- a\n  - b\n    - c\n      - d"
        );
        assert_eq!(to_markdown(&stored(&to_blocks(&normalized))), normalized);
    }

    #[test]
    fn long_text_splits_at_the_notion_limit() {
        let text = "é".repeat(1500);
        let rich_text = plain(&text);
        assert_eq!(rich_text.len(), 2);
        assert_eq!(plain_text(&rich_text), text);
    }

    #[test]
    fn plain_text_stays_literal() {
        let text = "2 * 3 <tag> & [x]";
        assert_eq!(rich_text_markdown(&spans(text)), text);
    }
}
