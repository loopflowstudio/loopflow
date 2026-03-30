#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkdownListKind {
    Unordered,
    Ordered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarkdownList {
    kind: MarkdownListKind,
    items: Vec<MarkdownListItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarkdownListItem {
    content: String,
    children: Vec<MarkdownList>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MarkdownBlock {
    Heading(u8, String),
    Paragraph(String),
    HorizontalRule,
    List(MarkdownList),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarkdownListMarker {
    kind: MarkdownListKind,
    content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HtmlNode {
    Element(HtmlElement),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HtmlElement {
    name: String,
    href: Option<String>,
    children: Vec<HtmlNode>,
}

pub fn markdown_to_asana_html(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut index = 0;
    let blocks = parse_markdown_blocks(&lines, &mut index, 0, 0);
    format!("<body>{}</body>", render_markdown_blocks_html(&blocks))
}

pub fn asana_html_to_markdown(html: &str) -> String {
    let mut parser = HtmlParser::new(html);
    let nodes = parser.parse_nodes(None);
    render_html_nodes_markdown(&nodes, 0).trim_end().to_string()
}

fn parse_markdown_blocks(
    lines: &[&str],
    index: &mut usize,
    indent: usize,
    depth: usize,
) -> Vec<MarkdownBlock> {
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
        if depth > 0 && current_indent > indent && parse_markdown_list_marker(trimmed).is_some() {
            break;
        }

        if let Some(block) = parse_markdown_scalar_block(trimmed) {
            blocks.push(block);
            *index += 1;
            continue;
        }
        if parse_markdown_list_marker(trimmed).is_some() {
            blocks.extend(parse_markdown_list_blocks(
                lines,
                index,
                current_indent,
                depth,
            ));
            continue;
        }

        blocks.push(parse_markdown_paragraph(lines, index, indent));
    }

    blocks
}

fn parse_markdown_paragraph(lines: &[&str], index: &mut usize, indent: usize) -> MarkdownBlock {
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
        if is_markdown_block_boundary(trimmed) {
            break;
        }

        parts.push(trimmed.to_string());
        *index += 1;
    }

    MarkdownBlock::Paragraph(parts.join(" "))
}

fn parse_markdown_scalar_block(line: &str) -> Option<MarkdownBlock> {
    if line == "---" {
        return Some(MarkdownBlock::HorizontalRule);
    }

    for (level, prefix) in [(3, "### "), (2, "## "), (1, "# ")] {
        if let Some(content) = line.strip_prefix(prefix) {
            return Some(MarkdownBlock::Heading(level, content.to_string()));
        }
    }

    None
}

fn is_markdown_block_boundary(line: &str) -> bool {
    parse_markdown_scalar_block(line).is_some() || parse_markdown_list_marker(line).is_some()
}

fn push_markdown_line(content: &mut String, line: &str) {
    if !content.is_empty() {
        content.push(' ');
    }
    content.push_str(line);
}

fn parse_markdown_list_blocks(
    lines: &[&str],
    index: &mut usize,
    indent: usize,
    depth: usize,
) -> Vec<MarkdownBlock> {
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
        let Some(marker) = parse_markdown_list_marker(trimmed) else {
            break;
        };
        if current_indent != indent {
            break;
        }

        let group_kind = marker.kind;
        let mut items = Vec::new();

        loop {
            let current_line = lines[*index];
            let current_indent = count_indent(current_line);
            let current_trimmed = current_line[current_indent..].trim_end();
            let Some(marker) = parse_markdown_list_marker(current_trimmed) else {
                break;
            };
            if current_indent != indent || marker.kind != group_kind {
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
                if next_indent < indent {
                    break;
                }

                let trimmed_next = next[next_indent..].trim_end();
                if parse_markdown_list_marker(trimmed_next).is_some() {
                    if next_indent == indent {
                        break;
                    }
                    if next_indent > indent && depth == 0 {
                        let child_blocks =
                            parse_markdown_list_blocks(lines, index, next_indent, depth + 1);
                        for block in child_blocks {
                            if let MarkdownBlock::List(list) = block {
                                children.push(list);
                            }
                        }
                        continue;
                    }
                    if next_indent > indent {
                        push_markdown_line(&mut content, trimmed_next.trim());
                        *index += 1;
                        continue;
                    }
                }

                if next_indent > indent {
                    push_markdown_line(&mut content, trimmed_next.trim());
                    *index += 1;
                    continue;
                }

                break;
            }

            items.push(MarkdownListItem { content, children });

            if *index >= lines.len() {
                break;
            }
            let next = lines[*index];
            if next.trim().is_empty() {
                *index += 1;
                break;
            }
            let next_indent = count_indent(next);
            if next_indent != indent {
                break;
            }
            let trimmed_next = next[next_indent..].trim_end();
            let Some(next_marker) = parse_markdown_list_marker(trimmed_next) else {
                break;
            };
            if next_marker.kind != group_kind {
                break;
            }
        }

        blocks.push(MarkdownBlock::List(MarkdownList {
            kind: group_kind,
            items,
        }));
    }

    blocks
}

fn parse_markdown_list_marker(line: &str) -> Option<MarkdownListMarker> {
    if let Some(content) = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
    {
        return Some(MarkdownListMarker {
            kind: MarkdownListKind::Unordered,
            content: content.to_string(),
        });
    }

    let digits = line.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits > 0 && line[digits..].starts_with(". ") {
        return Some(MarkdownListMarker {
            kind: MarkdownListKind::Ordered,
            content: line[digits + 2..].to_string(),
        });
    }

    None
}

fn render_markdown_blocks_html(blocks: &[MarkdownBlock]) -> String {
    blocks
        .iter()
        .map(render_markdown_block_html)
        .collect::<Vec<_>>()
        .join("")
}

fn render_markdown_block_html(block: &MarkdownBlock) -> String {
    match block {
        MarkdownBlock::Heading(level, content) => {
            // Asana supports h1 and h2; h3 maps to h2
            let tag = if *level <= 2 { *level } else { 2 };
            format!("<h{tag}>{}</h{tag}>", render_markdown_inline_html(content))
        }
        MarkdownBlock::Paragraph(content) => {
            // Asana doesn't support <p> — use inline text with trailing newline
            format!("{}\n", render_markdown_inline_html(content))
        }
        MarkdownBlock::HorizontalRule => "<hr/>".to_string(),
        MarkdownBlock::List(list) => render_markdown_list_html(list),
    }
}

fn render_markdown_list_html(list: &MarkdownList) -> String {
    let tag = markdown_list_tag(list.kind);
    let items = list
        .items
        .iter()
        .map(render_markdown_list_item_html)
        .collect::<Vec<_>>()
        .join("");
    format!("<{tag}>{items}</{tag}>")
}

fn render_markdown_list_item_html(item: &MarkdownListItem) -> String {
    let children = item
        .children
        .iter()
        .map(render_markdown_list_html)
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<li>{}{children}</li>",
        render_markdown_inline_html(&item.content)
    )
}

fn markdown_list_tag(kind: MarkdownListKind) -> &'static str {
    match kind {
        MarkdownListKind::Unordered => "ul",
        MarkdownListKind::Ordered => "ol",
    }
}

fn render_markdown_inline_html(text: &str) -> String {
    let mut rendered = String::new();
    render_markdown_inline_segment(text, &mut rendered);
    rendered
}

fn render_markdown_inline_segment(text: &str, rendered: &mut String) {
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];

        if let Some((inner, consumed)) = delimited(rest, "**") {
            rendered.push_str("<strong>");
            render_markdown_inline_segment(inner, rendered);
            rendered.push_str("</strong>");
            index += consumed;
            continue;
        }
        if let Some((inner, consumed)) = delimited(rest, "~~") {
            rendered.push_str("<s>");
            render_markdown_inline_segment(inner, rendered);
            rendered.push_str("</s>");
            index += consumed;
            continue;
        }
        if let Some((inner, consumed)) = delimited(rest, "`") {
            rendered.push_str("<code>");
            rendered.push_str(&escape_html_text(inner));
            rendered.push_str("</code>");
            index += consumed;
            continue;
        }
        if let Some((inner, consumed)) = delimited(rest, "*") {
            rendered.push_str("<em>");
            render_markdown_inline_segment(inner, rendered);
            rendered.push_str("</em>");
            index += consumed;
            continue;
        }
        if let Some((label, url, consumed)) = parse_markdown_link(rest) {
            rendered.push_str("<a href=\"");
            rendered.push_str(&escape_html_attribute(url));
            rendered.push_str("\">");
            render_markdown_inline_segment(label, rendered);
            rendered.push_str("</a>");
            index += consumed;
            continue;
        }

        let next_special = next_markdown_special_index(rest).unwrap_or(rest.len());
        let plain = &rest[..next_special.max(1)];
        rendered.push_str(&escape_html_text(plain));
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

fn parse_markdown_link(text: &str) -> Option<(&str, &str, usize)> {
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

fn next_markdown_special_index(text: &str) -> Option<usize> {
    ["**", "~~", "`", "*", "["]
        .iter()
        .filter_map(|needle| text.find(needle))
        .min()
}

struct HtmlParser<'a> {
    input: &'a str,
    index: usize,
}

impl<'a> HtmlParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, index: 0 }
    }

    fn parse_nodes(&mut self, end_tag: Option<&str>) -> Vec<HtmlNode> {
        let mut nodes = Vec::new();

        while self.index < self.input.len() {
            if self.starts_with("<!--") {
                self.skip_comment();
                continue;
            }
            if self.starts_with("</") {
                let name = self.parse_closing_tag();
                if end_tag.is_some_and(|tag| tag.eq_ignore_ascii_case(&name)) {
                    break;
                }
                continue;
            }
            if self.current_char() == Some('<') {
                if let Some(node) = self.parse_element() {
                    nodes.push(node);
                }
                continue;
            }

            let text = self.parse_text();
            if !text.is_empty() {
                nodes.push(HtmlNode::Text(decode_html_entities(&text)));
            }
        }

        nodes
    }

    fn parse_element(&mut self) -> Option<HtmlNode> {
        self.consume_char('<')?;
        if self.current_char() == Some('!') {
            self.skip_until('>');
            self.consume_if('>');
            return None;
        }

        let name = self.parse_name();
        if name.is_empty() {
            self.skip_until('>');
            self.consume_if('>');
            return None;
        }

        let mut href = None;
        let mut self_closing = false;
        loop {
            self.skip_whitespace();
            if self.starts_with("/>") {
                self.index += 2;
                self_closing = true;
                break;
            }
            if self.consume_if('>') {
                break;
            }
            if self.index >= self.input.len() {
                break;
            }

            let attribute_name = self.parse_name();
            if attribute_name.is_empty() {
                self.index += 1;
                continue;
            }
            self.skip_whitespace();
            let value = if self.consume_if('=') {
                self.skip_whitespace();
                self.parse_attribute_value()
            } else {
                String::new()
            };
            if attribute_name.eq_ignore_ascii_case("href") {
                href = Some(decode_html_entities(&value));
            }
        }

        if self_closing || is_void_html_element(&name) {
            return Some(HtmlNode::Element(HtmlElement {
                name,
                href,
                children: Vec::new(),
            }));
        }

        let children = self.parse_nodes(Some(&name));
        Some(HtmlNode::Element(HtmlElement {
            name,
            href,
            children,
        }))
    }

    fn parse_closing_tag(&mut self) -> String {
        self.index += 2;
        let name = self.parse_name();
        self.skip_until('>');
        self.consume_if('>');
        name
    }

    fn parse_name(&mut self) -> String {
        let start = self.index;
        while let Some(ch) = self.current_char() {
            if ch.is_ascii_alphanumeric() || matches!(ch, ':' | '_' | '-') {
                self.index += ch.len_utf8();
            } else {
                break;
            }
        }
        self.input[start..self.index].to_ascii_lowercase()
    }

    fn parse_attribute_value(&mut self) -> String {
        match self.current_char() {
            Some('"') | Some('\'') => {
                let quote = self.current_char().expect("quote should exist");
                self.index += quote.len_utf8();
                let start = self.index;
                while let Some(ch) = self.current_char() {
                    if ch == quote {
                        let value = self.input[start..self.index].to_string();
                        self.index += quote.len_utf8();
                        return value;
                    }
                    self.index += ch.len_utf8();
                }
                self.input[start..].to_string()
            }
            _ => {
                let start = self.index;
                while let Some(ch) = self.current_char() {
                    if ch.is_whitespace() || matches!(ch, '>' | '/') {
                        break;
                    }
                    self.index += ch.len_utf8();
                }
                self.input[start..self.index].to_string()
            }
        }
    }

    fn parse_text(&mut self) -> String {
        let start = self.index;
        while let Some(ch) = self.current_char() {
            if ch == '<' {
                break;
            }
            self.index += ch.len_utf8();
        }
        self.input[start..self.index].to_string()
    }

    fn skip_comment(&mut self) {
        if let Some(end) = self.input[self.index + 4..].find("-->") {
            self.index += 4 + end + 3;
        } else {
            self.index = self.input.len();
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char() {
            if ch.is_whitespace() {
                self.index += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn skip_until(&mut self, needle: char) {
        while let Some(ch) = self.current_char() {
            if ch == needle {
                break;
            }
            self.index += ch.len_utf8();
        }
    }

    fn starts_with(&self, needle: &str) -> bool {
        self.input[self.index..].starts_with(needle)
    }

    fn current_char(&self) -> Option<char> {
        self.input[self.index..].chars().next()
    }

    fn consume_char(&mut self, expected: char) -> Option<char> {
        let current = self.current_char()?;
        if current == expected {
            self.index += current.len_utf8();
            Some(current)
        } else {
            None
        }
    }

    fn consume_if(&mut self, expected: char) -> bool {
        self.consume_char(expected).is_some()
    }
}

fn render_html_nodes_markdown(nodes: &[HtmlNode], indent: usize) -> String {
    let mut rendered = String::new();
    let mut inline_buffer = String::new();

    for node in nodes {
        match node {
            HtmlNode::Text(text) => inline_buffer.push_str(&collapse_html_whitespace(text)),
            HtmlNode::Element(element) if is_block_html_element(&element.name) => {
                flush_inline_buffer(&mut rendered, &mut inline_buffer, indent);
                rendered.push_str(&render_html_block_element_markdown(element, indent));
            }
            HtmlNode::Element(element) if element_has_block_children(element) => {
                flush_inline_buffer(&mut rendered, &mut inline_buffer, indent);
                rendered.push_str(&render_html_nodes_markdown(&element.children, indent));
            }
            HtmlNode::Element(element) => {
                inline_buffer.push_str(&render_html_inline_element_markdown(element));
            }
        }
    }

    flush_inline_buffer(&mut rendered, &mut inline_buffer, indent);
    rendered
}

fn render_html_block_element_markdown(element: &HtmlElement, indent: usize) -> String {
    let prefix = " ".repeat(indent);
    match element.name.as_str() {
        name if matches!(name, "h1" | "h2" | "h3") => {
            let marker = markdown_heading_marker(name).expect("matched heading should have marker");
            format!(
                "{prefix}{marker} {}\n\n",
                render_html_inline_nodes_markdown(&element.children).trim()
            )
        }
        "p" => {
            let text = render_html_inline_nodes_markdown(&element.children)
                .trim()
                .to_string();
            if text.is_empty() {
                String::new()
            } else {
                format!("{prefix}{text}\n\n")
            }
        }
        "ul" => render_html_list_markdown(&element.children, MarkdownListKind::Unordered, indent),
        "ol" => render_html_list_markdown(&element.children, MarkdownListKind::Ordered, indent),
        "li" => render_html_list_item_markdown(element, MarkdownListKind::Unordered, indent, 1),
        "hr" => format!("{prefix}---\n\n"),
        _ => render_html_nodes_markdown(&element.children, indent),
    }
}

fn render_html_list_markdown(
    children: &[HtmlNode],
    kind: MarkdownListKind,
    indent: usize,
) -> String {
    let mut rendered = String::new();
    let mut index = 1usize;

    for child in children {
        match child {
            HtmlNode::Text(text) if text.trim().is_empty() => {}
            HtmlNode::Element(element) if element.name == "li" => {
                rendered.push_str(&render_html_list_item_markdown(
                    element, kind, indent, index,
                ));
                index += 1;
            }
            HtmlNode::Element(element) => {
                rendered.push_str(&render_html_nodes_markdown(
                    std::slice::from_ref(child),
                    indent,
                ));
                if !rendered.ends_with('\n') {
                    rendered.push('\n');
                }
                if element.name == "ul" || element.name == "ol" {
                    index += 1;
                }
            }
            HtmlNode::Text(text) => {
                let text = text.trim();
                if !text.is_empty() {
                    let marker = markdown_list_item_marker(kind, index);
                    rendered.push_str(&format!("{}{}{}\n", " ".repeat(indent), marker, text));
                    index += 1;
                }
            }
        }
    }

    if indent == 0 && !rendered.is_empty() {
        rendered.push('\n');
    }
    rendered
}

fn render_html_list_item_markdown(
    element: &HtmlElement,
    kind: MarkdownListKind,
    indent: usize,
    index: usize,
) -> String {
    let marker = markdown_list_item_marker(kind, index);

    let mut content = String::new();
    let mut nested = String::new();

    for child in &element.children {
        match child {
            HtmlNode::Element(child_element) if child_element.name == "ul" => {
                nested.push_str(&render_html_list_markdown(
                    &child_element.children,
                    MarkdownListKind::Unordered,
                    indent + 2,
                ));
            }
            HtmlNode::Element(child_element) if child_element.name == "ol" => {
                nested.push_str(&render_html_list_markdown(
                    &child_element.children,
                    MarkdownListKind::Ordered,
                    indent + 2,
                ));
            }
            HtmlNode::Element(child_element) if is_block_html_element(&child_element.name) => {
                content.push_str(&render_html_inline_nodes_markdown(&child_element.children));
            }
            HtmlNode::Element(child_element) => {
                content.push_str(&render_html_inline_element_markdown(child_element));
            }
            HtmlNode::Text(text) => content.push_str(&collapse_html_whitespace(text)),
        }
    }

    let mut rendered = format!("{}{}{}\n", " ".repeat(indent), marker, content.trim());
    rendered.push_str(&nested);
    rendered
}

fn markdown_heading_marker(name: &str) -> Option<&'static str> {
    match name {
        "h1" => Some("#"),
        "h2" => Some("##"),
        "h3" => Some("###"),
        _ => None,
    }
}

fn markdown_list_item_marker(kind: MarkdownListKind, index: usize) -> String {
    match kind {
        MarkdownListKind::Ordered => format!("{index}. "),
        MarkdownListKind::Unordered => "- ".to_string(),
    }
}

fn render_html_inline_nodes_markdown(children: &[HtmlNode]) -> String {
    children
        .iter()
        .map(|child| match child {
            HtmlNode::Text(text) => collapse_html_whitespace(text),
            HtmlNode::Element(element) => render_html_inline_element_markdown(element),
        })
        .collect::<Vec<_>>()
        .join("")
}

fn render_html_inline_element_markdown(element: &HtmlElement) -> String {
    match element.name.as_str() {
        "strong" => wrap_markdown("**", render_html_inline_nodes_markdown(&element.children)),
        "em" => wrap_markdown("*", render_html_inline_nodes_markdown(&element.children)),
        "code" => wrap_markdown("`", render_html_plain_text(&element.children)),
        "s" => wrap_markdown("~~", render_html_inline_nodes_markdown(&element.children)),
        "a" => {
            let text = render_html_inline_nodes_markdown(&element.children);
            if let Some(href) = &element.href {
                if text.is_empty() {
                    href.clone()
                } else {
                    format!("[{text}]({href})")
                }
            } else {
                text
            }
        }
        _ => render_html_inline_nodes_markdown(&element.children),
    }
}

fn render_html_plain_text(children: &[HtmlNode]) -> String {
    children
        .iter()
        .map(|child| match child {
            HtmlNode::Text(text) => text.clone(),
            HtmlNode::Element(element) => render_html_plain_text(&element.children),
        })
        .collect::<Vec<_>>()
        .join("")
}

fn wrap_markdown(marker: &str, content: String) -> String {
    if content.is_empty() {
        String::new()
    } else {
        format!("{marker}{content}{marker}")
    }
}

fn flush_inline_buffer(rendered: &mut String, inline_buffer: &mut String, indent: usize) {
    let text = inline_buffer.trim();
    if !text.is_empty() {
        rendered.push_str(&format!("{}{}\n\n", " ".repeat(indent), text));
    }
    inline_buffer.clear();
}

fn is_void_html_element(name: &str) -> bool {
    matches!(name, "hr" | "br")
}

fn is_block_html_element(name: &str) -> bool {
    matches!(name, "h1" | "h2" | "h3" | "p" | "ul" | "ol" | "li" | "hr")
}

fn element_has_block_children(element: &HtmlElement) -> bool {
    element.children.iter().any(|child| match child {
        HtmlNode::Element(child) => {
            is_block_html_element(&child.name) || element_has_block_children(child)
        }
        HtmlNode::Text(_) => false,
    })
}

fn escape_html_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_html_attribute(text: &str) -> String {
    escape_html_text(text)
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn decode_html_entities(text: &str) -> String {
    let mut decoded = String::new();
    let mut index = 0;

    while index < text.len() {
        let rest = &text[index..];
        if !rest.starts_with('&') {
            let ch = rest.chars().next().expect("rest is not empty");
            decoded.push(ch);
            index += ch.len_utf8();
            continue;
        }

        let Some(end) = rest.find(';') else {
            decoded.push('&');
            index += 1;
            continue;
        };

        let entity = &rest[1..end];
        if let Some(value) = decode_entity(entity) {
            decoded.push_str(&value);
            index += end + 1;
        } else {
            decoded.push('&');
            index += 1;
        }
    }

    decoded
}

fn decode_entity(entity: &str) -> Option<String> {
    match entity {
        "amp" => Some("&".to_string()),
        "lt" => Some("<".to_string()),
        "gt" => Some(">".to_string()),
        "quot" => Some("\"".to_string()),
        "apos" | "#39" => Some("'".to_string()),
        "nbsp" => Some(" ".to_string()),
        _ => {
            let number = entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"));
            if let Some(number) = number {
                let codepoint = u32::from_str_radix(number, 16).ok()?;
                return char::from_u32(codepoint).map(|ch| ch.to_string());
            }
            let number = entity.strip_prefix('#')?;
            let codepoint = number.parse::<u32>().ok()?;
            char::from_u32(codepoint).map(|ch| ch.to_string())
        }
    }
}

fn collapse_html_whitespace(text: &str) -> String {
    let mut collapsed = String::new();
    let mut in_whitespace = false;

    for ch in text.chars() {
        if ch.is_whitespace() {
            if !in_whitespace {
                collapsed.push(' ');
                in_whitespace = true;
            }
        } else {
            collapsed.push(ch);
            in_whitespace = false;
        }
    }

    collapsed
}

fn count_indent(line: &str) -> usize {
    line.chars().take_while(|ch| *ch == ' ').count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_round_trips_supported_asana_formatting() {
        let markdown = "# Heading\n\nParagraph with **bold**, *italic*, `code`, ~~strike~~, and [link](https://example.com).\n\n- bullet\n  - nested bullet\n1. numbered\n\n---\n";

        let html = markdown_to_asana_html(markdown);
        let round_trip = asana_html_to_markdown(&html);

        assert!(html.contains("<h1>Heading</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
        assert!(html.contains("<code>code</code>"));
        assert!(html.contains("<s>strike</s>"));
        assert!(html.contains("<a href=\"https://example.com\">link</a>"));
        assert!(html.contains("<ul><li>bullet<ul><li>nested bullet</li></ul></li></ul>"));
        assert!(html.contains("<ol><li>numbered</li></ol>"));
        assert!(html.contains("<hr/>"));
        assert!(!html.contains("<p>"));
        assert!(round_trip.contains("# Heading"));
        assert!(round_trip.contains("**bold**"));
        assert!(round_trip.contains("*italic*"));
        assert!(round_trip.contains("`code`"));
        assert!(round_trip.contains("~~strike~~"));
        assert!(round_trip.contains("[link](https://example.com)"));
        assert!(round_trip.contains("- bullet"));
        assert!(round_trip.contains("  - nested bullet"));
        assert!(round_trip.contains("1. numbered"));
        assert!(round_trip.contains("---"));
    }

    #[test]
    fn html_to_markdown_strips_unknown_tags_to_text() {
        let markdown = asana_html_to_markdown(
            "<section><p>Hello <u>world</u></p><custom-tag>and <strong>friends</strong></custom-tag></section>",
        );

        assert_eq!(markdown, "Hello world\n\nand **friends**");
    }

    #[test]
    fn html_to_markdown_decodes_entities_and_preserves_links() {
        let markdown = asana_html_to_markdown(
            "<p>Fish &amp; Chips &lt;3 <a href=\"https://example.com?a=1&amp;b=2\">docs &amp; guides</a></p>",
        );

        assert_eq!(
            markdown,
            "Fish & Chips <3 [docs & guides](https://example.com?a=1&b=2)"
        );
    }

    #[test]
    fn markdown_to_html_escapes_text_and_attributes() {
        let html = markdown_to_asana_html(
            "Paragraph with <angle brackets> and [quote](https://example.com?a=1&b=\"two\")",
        );

        assert_eq!(
            html,
            "<body>Paragraph with &lt;angle brackets&gt; and <a href=\"https://example.com?a=1&amp;b=&quot;two&quot;\">quote</a>\n</body>"
        );
    }

    #[test]
    fn markdown_nested_lists_stop_at_one_level() {
        let html = markdown_to_asana_html("- parent\n  - child\n    - grandchild\n");
        let markdown = asana_html_to_markdown(&html);

        assert_eq!(
            html,
            "<body><ul><li>parent<ul><li>child - grandchild</li></ul></li></ul></body>"
        );
        assert_eq!(markdown, "- parent\n  - child - grandchild");
    }

    #[test]
    fn html_to_markdown_handles_self_closing_hr() {
        assert_eq!(asana_html_to_markdown("<hr><hr/>"), "---\n\n---");
    }

    #[test]
    fn markdown_to_html_wraps_in_body_and_uses_html_hr() {
        let markdown = "# Title\n\nSome **bold** text.\n\n---\n\n- item one\n- item two\n";
        let html = markdown_to_asana_html(markdown);

        assert!(html.starts_with("<body>"));
        assert!(html.ends_with("</body>"));
        assert!(html.contains("<hr/>"));
        assert!(!html.contains("<p>"));

        let round_trip = asana_html_to_markdown(&html);
        assert!(round_trip.contains("# Title"));
        assert!(round_trip.contains("**bold**"));
        assert!(round_trip.contains("---"));
        assert!(round_trip.contains("- item one"));
    }
}
