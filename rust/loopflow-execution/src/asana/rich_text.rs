//! Markdown ↔ Asana rich text, ported from the provider removed in 43a1f85ce3.
//!
//! Asana stores `html_notes` and `html_text` byte for byte, newlines included, so
//! the conversion is line-faithful: a text line stays a text line, a block element
//! takes the place of its source lines and their final newline, and reading back
//! inverts that. Markdown Asana cannot show (h3+, `* ` bullets, fence languages)
//! normalizes once; converting the normalized text again changes nothing.

/// Render Loopflow markdown as an Asana `<body>` document.
pub(crate) fn to_html(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.trim_end().lines().collect();
    let mut html = String::from("<body>");
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            index += 1;
            let mut code = Vec::new();
            while index < lines.len() && !lines[index].trim_start().starts_with("```") {
                code.push(lines[index]);
                index += 1;
            }
            index += 1;
            html.push_str(&format!("<pre>{}</pre>", escape(&code.join("\n"))));
            continue;
        }
        if let Some((level, content)) = heading(line) {
            html.push_str(&format!("<h{level}>{}</h{level}>", inline(content)));
            index += 1;
            continue;
        }
        if matches!(line.trim_end(), "---" | "***" | "___") {
            html.push_str("<hr/>");
            index += 1;
            continue;
        }
        if line.starts_with('>') {
            let mut quoted = Vec::new();
            while index < lines.len() && lines[index].starts_with('>') {
                let content = &lines[index][1..];
                quoted.push(inline(content.strip_prefix(' ').unwrap_or(content)));
                index += 1;
            }
            html.push_str(&format!("<blockquote>{}</blockquote>", quoted.join("\n")));
            continue;
        }
        if list_marker(trimmed).is_some() {
            let indent = indent_of(line);
            html.push_str(&render_list(&parse_list(&lines, &mut index, indent)));
            continue;
        }
        html.push_str(&inline(line));
        index += 1;
        if index < lines.len() {
            html.push('\n');
        }
    }
    html.push_str("</body>");
    html
}

/// Read Asana rich text back as Loopflow markdown.
pub(crate) fn to_markdown(html: &str) -> String {
    let nodes = HtmlParser {
        input: html,
        index: 0,
    }
    .parse_nodes(None);
    let mut markdown = String::new();
    render_blocks(&nodes, &mut markdown);
    markdown.trim_end().to_string()
}

// ── Markdown blocks ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListKind {
    Unordered,
    Ordered,
}

#[derive(Debug)]
struct List {
    kind: ListKind,
    items: Vec<ListItem>,
}

#[derive(Debug)]
struct ListItem {
    lines: Vec<String>,
    children: Vec<List>,
}

fn heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.chars().take_while(|ch| *ch == '#').count();
    let content = line[hashes..].strip_prefix(' ')?;
    // Asana has h1 and h2; deeper headings read as h2.
    (hashes > 0).then_some((hashes.min(2) as u8, content))
}

fn list_marker(line: &str) -> Option<(ListKind, &str)> {
    if let Some(content) = ["- ", "* ", "+ "]
        .iter()
        .find_map(|marker| line.strip_prefix(marker))
    {
        return Some((ListKind::Unordered, content));
    }
    let digits = line.chars().take_while(|ch| ch.is_ascii_digit()).count();
    let content = line[digits..].strip_prefix(". ")?;
    (digits > 0).then_some((ListKind::Ordered, content))
}

fn indent_of(line: &str) -> usize {
    line.chars().take_while(|ch| *ch == ' ').count()
}

/// Consecutive list lines at `indent` and of one kind. Deeper markers nest under
/// the item above; deeper plain lines continue it.
fn parse_list(lines: &[&str], index: &mut usize, indent: usize) -> List {
    let kind = list_marker(lines[*index].trim_start())
        .expect("caller found a list marker")
        .0;
    let mut items: Vec<ListItem> = Vec::new();
    while *index < lines.len() {
        let line = lines[*index];
        let current = indent_of(line);
        let trimmed = line.trim_start();
        if trimmed.is_empty() || current < indent {
            break;
        }
        match list_marker(trimmed) {
            Some((marker_kind, content)) if current == indent => {
                if marker_kind != kind {
                    break;
                }
                items.push(ListItem {
                    lines: vec![content.to_string()],
                    children: Vec::new(),
                });
                *index += 1;
            }
            Some(_) => match items.last_mut() {
                Some(item) => item.children.push(parse_list(lines, index, current)),
                None => break,
            },
            None if current > indent => match items.last_mut() {
                Some(item) => {
                    item.lines.push(trimmed.to_string());
                    *index += 1;
                }
                None => break,
            },
            None => break,
        }
    }
    List { kind, items }
}

fn render_list(list: &List) -> String {
    let tag = match list.kind {
        ListKind::Unordered => "ul",
        ListKind::Ordered => "ol",
    };
    let items: String = list
        .items
        .iter()
        .map(|item| {
            let content: Vec<String> = item.lines.iter().map(|line| inline(line)).collect();
            let children: String = item.children.iter().map(render_list).collect();
            format!("<li>{}{children}</li>", content.join("\n"))
        })
        .collect();
    format!("<{tag}>{items}</{tag}>")
}

// ── Markdown inline ─────────────────────────────────────────────────

fn inline(text: &str) -> String {
    let mut rendered = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some((inner, consumed)) = delimited(rest, "`") {
            rendered.push_str(&format!("<code>{}</code>", escape(inner)));
            rest = &rest[consumed..];
            continue;
        }
        let wrapped = [("**", "strong"), ("~~", "s"), ("*", "em")]
            .iter()
            .find_map(|(marker, tag)| delimited(rest, marker).map(|found| (tag, found)));
        if let Some((tag, (inner, consumed))) = wrapped {
            rendered.push_str(&format!("<{tag}>{}</{tag}>", inline(inner)));
            rest = &rest[consumed..];
            continue;
        }
        if let Some((label, url, consumed)) = link(rest) {
            rendered.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                escape(url).replace('"', "&quot;"),
                inline(label)
            ));
            rest = &rest[consumed..];
            continue;
        }
        let first = rest.chars().next().map_or(1, char::len_utf8);
        let plain = ["**", "~~", "`", "*", "["]
            .iter()
            .filter_map(|needle| rest[first..].find(needle).map(|at| at + first))
            .min()
            .unwrap_or(rest.len());
        rendered.push_str(&escape(&rest[..plain]));
        rest = &rest[plain..];
    }
    rendered
}

fn delimited<'a>(text: &'a str, delimiter: &str) -> Option<(&'a str, usize)> {
    let remainder = text.strip_prefix(delimiter)?;
    let end = remainder.find(delimiter)?;
    (end > 0).then(|| (&remainder[..end], 2 * delimiter.len() + end))
}

fn link(text: &str) -> Option<(&str, &str, usize)> {
    let label_end = text.strip_prefix('[')?.find("](")? + 1;
    let after = &text[label_end + 2..];
    let url_end = after.find(')')?;
    Some((
        &text[1..label_end],
        &after[..url_end],
        label_end + 3 + url_end,
    ))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ── Asana HTML ──────────────────────────────────────────────────────

#[derive(Debug)]
enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug)]
struct Element {
    name: String,
    href: Option<String>,
    children: Vec<Node>,
}

/// Text and block elements in document order. A block element ends its line.
fn render_blocks(nodes: &[Node], out: &mut String) {
    for node in nodes {
        match node {
            Node::Text(text) => out.push_str(text),
            Node::Element(element) if element.name == "body" => {
                render_blocks(&element.children, out)
            }
            Node::Element(element) if !is_block(&element.name) => {
                out.push_str(&render_inline_element(element))
            }
            Node::Element(element) => {
                // Asana-authored text can run straight into a block element.
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                render_block(element, out);
            }
        }
    }
}

fn is_block(name: &str) -> bool {
    matches!(
        name,
        "h1" | "h2" | "hr" | "pre" | "blockquote" | "ul" | "ol"
    )
}

fn render_block(element: &Element, out: &mut String) {
    match element.name.as_str() {
        "h1" | "h2" => {
            let marker = if element.name == "h1" { "#" } else { "##" };
            out.push_str(&format!("{marker} {}\n", render_inline(&element.children)));
        }
        "hr" => out.push_str("---\n"),
        "pre" => out.push_str(&format!("```\n{}\n```\n", plain_text(&element.children))),
        "blockquote" => {
            for line in render_inline(&element.children).split('\n') {
                out.push_str(&format!("> {line}\n"));
            }
        }
        _ => render_markdown_list(element, 0, out),
    }
}

fn render_markdown_list(list: &Element, indent: usize, out: &mut String) {
    let mut number = 0;
    for item in &list.children {
        let Node::Element(item) = item else {
            continue;
        };
        if item.name != "li" {
            continue;
        }
        number += 1;
        let marker = if list.name == "ol" {
            format!("{number}. ")
        } else {
            "- ".to_string()
        };
        let (nested, content): (Vec<&Node>, Vec<&Node>) = item.children.iter().partition(
            |child| matches!(child, Node::Element(e) if e.name == "ul" || e.name == "ol"),
        );
        let text = content
            .into_iter()
            .map(|child| match child {
                Node::Text(text) => text.clone(),
                Node::Element(element) => render_inline_element(element),
            })
            .collect::<String>();
        let continuation = format!("\n{}", " ".repeat(indent + marker.len()));
        out.push_str(&format!(
            "{}{marker}{}\n",
            " ".repeat(indent),
            text.trim().replace('\n', &continuation)
        ));
        for child in nested {
            if let Node::Element(child) = child {
                render_markdown_list(child, indent + marker.len(), out);
            }
        }
    }
}

fn render_inline(nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(|node| match node {
            Node::Text(text) => text.clone(),
            Node::Element(element) => render_inline_element(element),
        })
        .collect()
}

fn render_inline_element(element: &Element) -> String {
    let wrap = |marker: &str| {
        let content = render_inline(&element.children);
        if content.is_empty() {
            content
        } else {
            format!("{marker}{content}{marker}")
        }
    };
    match element.name.as_str() {
        "strong" => wrap("**"),
        "em" => wrap("*"),
        "s" => wrap("~~"),
        "code" => format!("`{}`", plain_text(&element.children)),
        "a" => {
            let text = render_inline(&element.children);
            match &element.href {
                Some(href) if text.is_empty() || text == *href => href.clone(),
                Some(href) => format!("[{text}]({href})"),
                None => text,
            }
        }
        "br" => "\n".to_string(),
        _ => render_inline(&element.children),
    }
}

fn plain_text(nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(|node| match node {
            Node::Text(text) => text.clone(),
            Node::Element(element) => plain_text(&element.children),
        })
        .collect()
}

/// A forgiving reader for the tag subset Asana emits; unknown tags keep their text.
struct HtmlParser<'a> {
    input: &'a str,
    index: usize,
}

impl HtmlParser<'_> {
    fn parse_nodes(&mut self, end_tag: Option<&str>) -> Vec<Node> {
        let mut nodes = Vec::new();
        while self.index < self.input.len() {
            let rest = &self.input[self.index..];
            if rest.starts_with("<!--") {
                self.index = rest
                    .find("-->")
                    .map_or(self.input.len(), |end| self.index + end + 3);
            } else if rest.starts_with("</") {
                self.index += 2;
                let name = self.name();
                self.skip_past('>');
                if end_tag == Some(name.as_str()) {
                    break;
                }
            } else if rest.starts_with('<') {
                if let Some(element) = self.element() {
                    nodes.push(Node::Element(element));
                }
            } else {
                let end = rest.find('<').unwrap_or(rest.len());
                nodes.push(Node::Text(decode_entities(&rest[..end])));
                self.index += end;
            }
        }
        nodes
    }

    fn element(&mut self) -> Option<Element> {
        self.index += 1;
        let name = self.name();
        if name.is_empty() {
            self.skip_past('>');
            return None;
        }
        let mut href = None;
        let mut self_closing = false;
        while self.index < self.input.len() {
            self.skip_whitespace();
            let rest = &self.input[self.index..];
            if rest.starts_with("/>") {
                self.index += 2;
                self_closing = true;
                break;
            }
            if rest.starts_with('>') {
                self.index += 1;
                break;
            }
            let attribute = self.name();
            if attribute.is_empty() {
                self.index += rest.chars().next().map_or(1, char::len_utf8);
                continue;
            }
            self.skip_whitespace();
            if self.input[self.index..].starts_with('=') {
                self.index += 1;
                self.skip_whitespace();
                let value = self.attribute_value();
                if attribute == "href" {
                    href = Some(decode_entities(&value));
                }
            }
        }
        let children = if self_closing || matches!(name.as_str(), "hr" | "br" | "img") {
            Vec::new()
        } else {
            self.parse_nodes(Some(&name))
        };
        Some(Element {
            name,
            href,
            children,
        })
    }

    fn name(&mut self) -> String {
        let rest = &self.input[self.index..];
        let end = rest
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, ':' | '_' | '-')))
            .unwrap_or(rest.len());
        self.index += end;
        rest[..end].to_ascii_lowercase()
    }

    fn attribute_value(&mut self) -> String {
        let rest = &self.input[self.index..];
        let (value, consumed) = match rest.chars().next() {
            Some(quote @ ('"' | '\'')) => match rest[1..].find(quote) {
                Some(end) => (&rest[1..=end], end + 2),
                None => (&rest[1..], rest.len()),
            },
            _ => {
                let end = rest
                    .find(|ch: char| ch.is_whitespace() || ch == '>')
                    .unwrap_or(rest.len());
                (&rest[..end], end)
            }
        };
        self.index += consumed;
        value.to_string()
    }

    fn skip_whitespace(&mut self) {
        let rest = &self.input[self.index..];
        self.index += rest.len() - rest.trim_start().len();
    }

    fn skip_past(&mut self, needle: char) {
        self.index = self.input[self.index..]
            .find(needle)
            .map_or(self.input.len(), |end| self.index + end + 1);
    }
}

fn decode_entities(text: &str) -> String {
    let mut decoded = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        decoded.push_str(&rest[..start]);
        rest = &rest[start..];
        let entity = rest[1..]
            .find(';')
            .filter(|end| *end <= 10)
            .and_then(|end| decode_entity(&rest[1..=end]).map(|ch| (ch, end + 2)));
        match entity {
            Some((ch, consumed)) => {
                decoded.push(ch);
                rest = &rest[consumed..];
            }
            None => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);
    decoded
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let number = entity.strip_prefix('#')?;
            let codepoint = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(codepoint)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RICH: &str = "# Plan\n\nShip **bold**, *italic*, `a<b`, ~~old~~ and [docs](https://example.com?a=1&b=2).\nSecond line.\n\n- one\n  - nested\n- two\n  continued\n1. first\n2. second\nafter\n\n```\nfn main() {\n    x < 1\n}\n```\n\n> quoted\n---\n## Sub\nend";

    #[test]
    fn markdown_renders_as_asana_rich_text() {
        assert_eq!(
            to_html(RICH),
            "<body><h1>Plan</h1>\nShip <strong>bold</strong>, <em>italic</em>, <code>a&lt;b</code>, <s>old</s> and <a href=\"https://example.com?a=1&amp;b=2\">docs</a>.\nSecond line.\n\n<ul><li>one<ul><li>nested</li></ul></li><li>two\ncontinued</li></ul><ol><li>first</li><li>second</li></ol>after\n\n<pre>fn main() {\n    x &lt; 1\n}</pre>\n<blockquote>quoted</blockquote><hr/><h2>Sub</h2>end</body>"
        );
    }

    #[test]
    fn supported_markdown_round_trips_exactly() {
        // Asana echoes `<hr/>` as `<hr />`.
        let stored = to_html(RICH).replace("<hr/>", "<hr />");
        assert_eq!(to_markdown(&stored), RICH);
    }

    #[test]
    fn unsupported_markdown_normalizes_once() {
        let markdown = "### Deep\n* star\n10. ten\n```bash\nls\n```";
        let normalized = to_markdown(&to_html(markdown));
        assert_eq!(normalized, "## Deep\n- star\n1. ten\n```\nls\n```");
        assert_eq!(to_markdown(&to_html(&normalized)), normalized);
    }

    #[test]
    fn asana_authored_html_reads_as_markdown() {
        assert_eq!(
            to_markdown("<body>Fish &amp; chips <u>now</u> <a href=\"https://app.asana.com/0/1/2\" data-asana-gid=\"2\">https://app.asana.com/0/1/2</a><custom> kept ü</custom></body>"),
            "Fish & chips now https://app.asana.com/0/1/2 kept ü"
        );
        assert_eq!(to_markdown("<body>a<hr>b</body>"), "a\n---\nb");
    }

    #[test]
    fn plain_text_is_escaped_not_interpreted() {
        assert_eq!(
            to_html("2 * 3 <tag> & [x]"),
            "<body>2 * 3 &lt;tag&gt; &amp; [x]</body>"
        );
        assert_eq!(
            to_markdown(&to_html("2 * 3 <tag> & [x]")),
            "2 * 3 <tag> & [x]"
        );
    }
}
