use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Copy, Default)]
struct InlineStyle {
    bold: bool,
    italic: bool,
    code: bool,
    strikethrough: bool,
}

#[derive(Debug, Clone, Copy)]
enum ListKind {
    Bulleted,
    Numbered,
    Todo(bool),
}

#[derive(Debug, Clone)]
struct ListMarker {
    indent: usize,
    kind: ListKind,
    content: String,
}

pub fn markdown_to_blocks(markdown: &str) -> Vec<Value> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut index = 0;
    parse_blocks(&lines, &mut index, 0, 0)
}

pub fn blocks_to_markdown(blocks: &[Value]) -> String {
    render_blocks(blocks, 0).trim_end().to_string()
}

fn parse_blocks(lines: &[&str], index: &mut usize, indent: usize, depth: usize) -> Vec<Value> {
    let mut blocks = Vec::new();

    while *index < lines.len() {
        let line = lines[*index];
        if line.trim().is_empty() {
            *index += 1;
            continue;
        }

        let current_indent = count_indent(line);
        if current_indent < indent {
            break;
        }

        let trimmed = line[current_indent..].trim_end();
        if depth > 0
            && current_indent > indent
            && parse_list_marker(trimmed, current_indent).is_some()
        {
            break;
        }

        if let Some(language) = trimmed.strip_prefix("```") {
            blocks.push(parse_code_block(lines, index, language.trim()));
            continue;
        }
        if trimmed == "---" {
            blocks.push(json!({
                "object": "block",
                "type": "divider",
                "divider": {}
            }));
            *index += 1;
            continue;
        }
        if let Some(content) = trimmed.strip_prefix("# ") {
            blocks.push(rich_text_block("heading_1", parse_inline(content)));
            *index += 1;
            continue;
        }
        if let Some(content) = trimmed.strip_prefix("## ") {
            blocks.push(rich_text_block("heading_2", parse_inline(content)));
            *index += 1;
            continue;
        }
        if let Some(content) = trimmed.strip_prefix("### ") {
            blocks.push(rich_text_block("heading_3", parse_inline(content)));
            *index += 1;
            continue;
        }
        if trimmed.starts_with('>') {
            blocks.push(parse_quote_block(lines, index, indent));
            continue;
        }
        if parse_list_marker(trimmed, current_indent).is_some() {
            blocks.extend(parse_list(lines, index, current_indent, depth));
            continue;
        }

        blocks.push(parse_paragraph(lines, index, indent));
    }

    blocks
}

fn parse_code_block(lines: &[&str], index: &mut usize, language: &str) -> Value {
    *index += 1;
    let mut content = Vec::new();
    while *index < lines.len() {
        let line = lines[*index];
        if line.trim_start().starts_with("```") {
            *index += 1;
            break;
        }
        content.push(line.to_string());
        *index += 1;
    }

    json!({
        "object": "block",
        "type": "code",
        "code": {
            "rich_text": [rich_text_span(&content.join("\n"), InlineStyle::default(), None)],
            "language": if language.is_empty() { "plain text" } else { language }
        }
    })
}

fn parse_quote_block(lines: &[&str], index: &mut usize, indent: usize) -> Value {
    let mut content = Vec::new();
    while *index < lines.len() {
        let line = lines[*index];
        if line.trim().is_empty() {
            break;
        }
        let current_indent = count_indent(line);
        if current_indent < indent {
            break;
        }
        let trimmed = line[current_indent..].trim_end();
        let Some(rest) = trimmed.strip_prefix('>') else {
            break;
        };
        content.push(rest.trim_start().to_string());
        *index += 1;
    }

    rich_text_block("quote", parse_inline(&content.join("\n")))
}

fn parse_paragraph(lines: &[&str], index: &mut usize, indent: usize) -> Value {
    let mut parts = Vec::new();
    while *index < lines.len() {
        let line = lines[*index];
        if line.trim().is_empty() {
            break;
        }
        let current_indent = count_indent(line);
        if current_indent < indent {
            break;
        }
        let trimmed = line[current_indent..].trim_end();
        if trimmed == "---"
            || trimmed.starts_with("```")
            || trimmed.starts_with("# ")
            || trimmed.starts_with("## ")
            || trimmed.starts_with("### ")
            || trimmed.starts_with('>')
            || parse_list_marker(trimmed, current_indent).is_some()
        {
            break;
        }
        parts.push(trimmed.to_string());
        *index += 1;
    }

    rich_text_block("paragraph", parse_inline(&parts.join(" ")))
}

fn parse_list(lines: &[&str], index: &mut usize, indent: usize, depth: usize) -> Vec<Value> {
    let mut blocks = Vec::new();

    while *index < lines.len() {
        let line = lines[*index];
        if line.trim().is_empty() {
            *index += 1;
            break;
        }

        let current_indent = count_indent(line);
        if current_indent < indent {
            break;
        }
        let trimmed = line[current_indent..].trim_end();
        let Some(marker) = parse_list_marker(trimmed, current_indent) else {
            break;
        };
        if marker.indent != indent {
            break;
        }

        *index += 1;
        let mut content = marker.content;
        let mut children = Vec::new();

        while *index < lines.len() {
            let next = lines[*index];
            if next.trim().is_empty() {
                *index += 1;
                break;
            }
            let next_indent = count_indent(next);
            let trimmed_next = next[next_indent..].trim_end();
            if next_indent < indent {
                break;
            }
            if let Some(child_marker) = parse_list_marker(trimmed_next, next_indent) {
                if next_indent == indent {
                    break;
                }
                if next_indent > indent && depth == 0 {
                    children.extend(parse_list(lines, index, child_marker.indent, depth + 1));
                    continue;
                }
                if next_indent > indent {
                    if !content.is_empty() {
                        content.push(' ');
                    }
                    content.push_str(trimmed_next.trim());
                    *index += 1;
                    continue;
                }
            }
            if next_indent > indent {
                if !content.is_empty() {
                    content.push(' ');
                }
                content.push_str(trimmed_next.trim());
                *index += 1;
                continue;
            }
            break;
        }

        blocks.push(list_block(marker.kind, &content, children));
    }

    blocks
}

fn parse_list_marker(line: &str, indent: usize) -> Option<ListMarker> {
    if let Some(content) = line.strip_prefix("- [ ] ") {
        return Some(ListMarker {
            indent,
            kind: ListKind::Todo(false),
            content: content.to_string(),
        });
    }
    if let Some(content) = line
        .strip_prefix("- [x] ")
        .or_else(|| line.strip_prefix("- [X] "))
    {
        return Some(ListMarker {
            indent,
            kind: ListKind::Todo(true),
            content: content.to_string(),
        });
    }
    if let Some(content) = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
    {
        return Some(ListMarker {
            indent,
            kind: ListKind::Bulleted,
            content: content.to_string(),
        });
    }

    let digits = line.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits > 0 && line[digits..].starts_with(". ") {
        return Some(ListMarker {
            indent,
            kind: ListKind::Numbered,
            content: line[digits + 2..].to_string(),
        });
    }

    None
}

fn list_block(kind: ListKind, content: &str, children: Vec<Value>) -> Value {
    let (block_type, extra_field) = match kind {
        ListKind::Bulleted => ("bulleted_list_item", None),
        ListKind::Numbered => ("numbered_list_item", None),
        ListKind::Todo(checked) => ("to_do", Some(("checked", Value::Bool(checked)))),
    };

    let mut fields = Map::new();
    fields.insert("rich_text".to_string(), Value::Array(parse_inline(content)));
    if let Some((key, value)) = extra_field {
        fields.insert(key.to_string(), value);
    }
    if !children.is_empty() {
        fields.insert("children".to_string(), Value::Array(children));
    }

    let mut block = Map::new();
    block.insert("object".to_string(), Value::String("block".to_string()));
    block.insert("type".to_string(), Value::String(block_type.to_string()));
    block.insert(block_type.to_string(), Value::Object(fields));
    Value::Object(block)
}

fn rich_text_block(block_type: &str, rich_text: Vec<Value>) -> Value {
    json!({
        "object": "block",
        "type": block_type,
        block_type: {
            "rich_text": rich_text
        }
    })
}

fn parse_inline(text: &str) -> Vec<Value> {
    let mut spans = Vec::new();
    parse_inline_segment(text, InlineStyle::default(), None, &mut spans);
    if spans.is_empty() {
        spans.push(rich_text_span("", InlineStyle::default(), None));
    }
    spans
}

fn parse_inline_segment(
    text: &str,
    style: InlineStyle,
    href: Option<&str>,
    spans: &mut Vec<Value>,
) {
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];

        if let Some(inner) = delimited(rest, "**") {
            let mut next_style = style;
            next_style.bold = true;
            parse_inline_segment(inner.0, next_style, href, spans);
            index += inner.1;
            continue;
        }
        if let Some(inner) = delimited(rest, "~~") {
            let mut next_style = style;
            next_style.strikethrough = true;
            parse_inline_segment(inner.0, next_style, href, spans);
            index += inner.1;
            continue;
        }
        if let Some(inner) = delimited(rest, "`") {
            let mut next_style = style;
            next_style.code = true;
            push_text_span(spans, inner.0, next_style, href);
            index += inner.1;
            continue;
        }
        if let Some(inner) = delimited(rest, "*") {
            let mut next_style = style;
            next_style.italic = true;
            parse_inline_segment(inner.0, next_style, href, spans);
            index += inner.1;
            continue;
        }
        if let Some((label, url, consumed)) = parse_link(rest) {
            parse_inline_segment(label, style, Some(url), spans);
            index += consumed;
            continue;
        }

        let next_special = next_special_index(rest).unwrap_or(rest.len());
        let plain = &rest[..next_special.max(1)];
        push_text_span(spans, plain, style, href);
        index += plain.len();
    }
}

fn delimited<'a>(text: &'a str, delimiter: &str) -> Option<(&'a str, usize)> {
    if !text.starts_with(delimiter) {
        return None;
    }
    let remainder = &text[delimiter.len()..];
    let end = remainder.find(delimiter)?;
    Some((&remainder[..end], delimiter.len() + end + delimiter.len()))
}

fn parse_link(text: &str) -> Option<(&str, &str, usize)> {
    if !text.starts_with('[') {
        return None;
    }
    let label_end = text.find("](")?;
    let after = &text[label_end + 2..];
    let url_end = after.find(')')?;
    Some((
        &text[1..label_end],
        &after[..url_end],
        label_end + 2 + url_end + 1,
    ))
}

fn next_special_index(text: &str) -> Option<usize> {
    ["**", "~~", "`", "*", "["]
        .iter()
        .filter_map(|needle| text.find(needle))
        .min()
}

fn push_text_span(spans: &mut Vec<Value>, text: &str, style: InlineStyle, href: Option<&str>) {
    if text.is_empty() {
        return;
    }
    spans.push(rich_text_span(text, style, href));
}

fn rich_text_span(text: &str, style: InlineStyle, href: Option<&str>) -> Value {
    json!({
        "type": "text",
        "text": {
            "content": text,
            "link": href.map(|url| json!({ "url": url }))
        },
        "annotations": {
            "bold": style.bold,
            "italic": style.italic,
            "strikethrough": style.strikethrough,
            "underline": false,
            "code": style.code,
            "color": "default"
        },
        "plain_text": text,
        "href": href
    })
}

fn render_blocks(blocks: &[Value], indent: usize) -> String {
    let mut rendered = String::new();
    for block in blocks {
        rendered.push_str(&render_block(block, indent));
    }
    rendered
}

fn render_block(block: &Value, indent: usize) -> String {
    let block_type = block
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("paragraph");
    let prefix = " ".repeat(indent);

    match block_type {
        "heading_1" => format!(
            "{prefix}# {}\n\n",
            render_rich_text(block_data(block, block_type))
        ),
        "heading_2" => format!(
            "{prefix}## {}\n\n",
            render_rich_text(block_data(block, block_type))
        ),
        "heading_3" => format!(
            "{prefix}### {}\n\n",
            render_rich_text(block_data(block, block_type))
        ),
        "paragraph" => {
            let text = render_rich_text(block_data(block, block_type));
            if text.is_empty() {
                "\n".to_string()
            } else {
                format!("{prefix}{text}\n\n")
            }
        }
        "quote" => {
            let text = render_rich_text(block_data(block, block_type));
            let body = text
                .lines()
                .map(|line| format!("{prefix}> {line}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!("{body}\n\n")
        }
        "divider" => format!("{prefix}---\n\n"),
        "code" => {
            let data = block_data(block, block_type);
            let language = data
                .get("language")
                .and_then(Value::as_str)
                .filter(|value| *value != "plain text")
                .unwrap_or("");
            let text = render_rich_text(data);
            format!("{prefix}```{language}\n{text}\n{prefix}```\n\n")
        }
        "bulleted_list_item" => render_list_item(block, indent, "- "),
        "numbered_list_item" => render_list_item(block, indent, "1. "),
        "to_do" => {
            let checked = block_data(block, block_type)
                .get("checked")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let marker = if checked { "- [x] " } else { "- [ ] " };
            render_list_item(block, indent, marker)
        }
        _ => {
            let text = block_plain_text(block);
            if text.is_empty() {
                String::new()
            } else {
                format!("{prefix}{text}\n\n")
            }
        }
    }
}

fn render_list_item(block: &Value, indent: usize, marker: &str) -> String {
    let block_type = block
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("bulleted_list_item");
    let data = block_data(block, block_type);
    let prefix = " ".repeat(indent);
    let text = render_rich_text(data);
    let mut rendered = format!("{prefix}{marker}{text}\n");
    if let Some(children) = data.get("children").and_then(Value::as_array) {
        rendered.push_str(&render_blocks(children, indent + 2));
    }
    if indent == 0 {
        rendered.push('\n');
    }
    rendered
}

fn block_data<'a>(block: &'a Value, block_type: &str) -> &'a Value {
    block.get(block_type).unwrap_or(block)
}

fn render_rich_text(data: &Value) -> String {
    data.get("rich_text")
        .and_then(Value::as_array)
        .map(|rich_text| {
            rich_text
                .iter()
                .map(render_span)
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_else(|| block_plain_text(data))
}

fn render_span(span: &Value) -> String {
    let text = span
        .get("plain_text")
        .and_then(Value::as_str)
        .or_else(|| {
            span.get("text")
                .and_then(|text| text.get("content"))
                .and_then(Value::as_str)
        })
        .unwrap_or("");
    let annotations = span.get("annotations");
    let mut rendered = text.to_string();
    if annotations
        .and_then(|value| value.get("code"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        rendered = format!("`{rendered}`");
    }
    if annotations
        .and_then(|value| value.get("bold"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        rendered = format!("**{rendered}**");
    }
    if annotations
        .and_then(|value| value.get("italic"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        rendered = format!("*{rendered}*");
    }
    if annotations
        .and_then(|value| value.get("strikethrough"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        rendered = format!("~~{rendered}~~");
    }
    let href = span.get("href").and_then(Value::as_str).or_else(|| {
        span.get("text")
            .and_then(|text| text.get("link"))
            .and_then(|link| link.get("url"))
            .and_then(Value::as_str)
    });
    if let Some(url) = href {
        rendered = format!("[{rendered}]({url})");
    }
    rendered
}

fn block_plain_text(block: &Value) -> String {
    if let Some(text) = block.get("plain_text").and_then(Value::as_str) {
        return text.to_string();
    }
    if let Some(content) = block
        .get("text")
        .and_then(|text| text.get("content"))
        .and_then(Value::as_str)
    {
        return content.to_string();
    }
    let block_type = block
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let data = block.get(block_type).unwrap_or(block);
    data.get("rich_text")
        .and_then(Value::as_array)
        .map(|rich_text| {
            rich_text
                .iter()
                .filter_map(|span| span.get("plain_text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

fn count_indent(line: &str) -> usize {
    line.chars().take_while(|ch| *ch == ' ').count()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn markdown_round_trips_supported_blocks() {
        let markdown = "# Heading\n\nParagraph with **bold**, *italic*, `code`, ~~strike~~, and [link](https://example.com).\n\n- bullet\n  - nested bullet\n1. numbered\n- [x] done\n- [ ] todo\n\n> quoted text\n\n---\n\n```rust\nfn main() {}\n```\n";

        let round_trip = blocks_to_markdown(&markdown_to_blocks(markdown));
        assert!(round_trip.contains("# Heading"));
        assert!(round_trip.contains("**bold**"));
        assert!(round_trip.contains("*italic*"));
        assert!(round_trip.contains("`code`"));
        assert!(round_trip.contains("~~strike~~"));
        assert!(round_trip.contains("[link](https://example.com)"));
        assert!(round_trip.contains("- bullet"));
        assert!(round_trip.contains("  - nested bullet"));
        assert!(round_trip.contains("1. numbered"));
        assert!(round_trip.contains("- [x] done"));
        assert!(round_trip.contains("- [ ] todo"));
        assert!(round_trip.contains("> quoted text"));
        assert!(round_trip.contains("---"));
        assert!(round_trip.contains("```rust"));
    }

    #[test]
    fn blocks_to_markdown_handles_unknown_blocks_as_paragraphs() {
        let markdown = blocks_to_markdown(&[json!({
            "type": "mystery",
            "mystery": {
                "rich_text": [{ "plain_text": "Fallback text" }]
            }
        })]);

        assert_eq!(markdown, "Fallback text");
    }

    #[test]
    fn markdown_to_blocks_creates_nested_list_children_once() {
        let blocks = markdown_to_blocks("- parent\n  - child\n    - grandchild\n");
        let parent = &blocks[0];
        let children = parent["bulleted_list_item"]["children"]
            .as_array()
            .expect("children array");
        assert_eq!(children.len(), 1);
        assert_eq!(
            children[0]["bulleted_list_item"]["rich_text"][0]["plain_text"],
            "child - grandchild"
        );
    }

    #[test]
    fn parse_inline_preserves_link_annotations() {
        let rich_text = parse_inline("See [docs](https://example.com)");
        let link = rich_text.last().expect("link span");
        assert_eq!(link["href"], "https://example.com");
        assert_eq!(link["plain_text"], "docs");
    }
}
