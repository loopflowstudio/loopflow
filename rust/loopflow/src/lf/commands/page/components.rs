//! `<lf-component>` lookup and template expansion (`{{attr}}`, `<slot>`).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{bail, Context, Result};
use loopflow_definitions::builtins::get_builtin_page_component;
use regex::{Captures, Regex};

use super::html::{escape_html, find_next_element, line_at};

static PLACEHOLDER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{\s*([a-zA-Z0-9_-]+)\s*\}\}").expect("static regex"));
static NAMED_SLOT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)<slot\s+name\s*=\s*(?:"([^"]*)"|'([^']*)')\s*(?:/>|>\s*</slot>)"#)
        .expect("static named slot regex")
});
static DEFAULT_SLOT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<slot\s*(?:/>|>\s*</slot>)").expect("static default slot regex")
});
static SLOT_ATTR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)<([a-zA-Z0-9_-]+)\b([^>]*\bslot\s*=\s*(?:"([^"]*)"|'([^']*)')[^>]*)>"#)
        .expect("static slot attr regex")
});

/// Expand `<lf-component>` elements iteratively (up to depth 4) so nested
/// components resolve cleanly.
pub(super) fn expand(
    repo_root: &Path,
    input: &str,
    component_styles: &mut BTreeMap<String, String>,
) -> Result<String> {
    let mut current = input.to_owned();
    for _ in 0..4 {
        let mut changed = false;
        let mut out = String::with_capacity(current.len());
        let mut cursor = 0;
        while let Some(tag) = find_next_element(&current, cursor, "lf-component")? {
            changed = true;
            out.push_str(&current[cursor..tag.start]);
            let line = line_at(&current, tag.start);
            let name = tag.attrs.get("name").cloned().ok_or_else(|| {
                anyhow::anyhow!("line {line}: <lf-component> requires a `name` attribute")
            })?;
            let (template, style) = load_component(repo_root, &name, line)?;
            if let Some(style) = style {
                component_styles.entry(name).or_insert(style);
            }
            out.push_str(&render_component(&template, &tag.attrs, &tag.inner)?);
            cursor = tag.end;
        }
        out.push_str(&current[cursor..]);
        current = out;
        if !changed {
            break;
        }
    }
    Ok(current)
}

fn load_component(repo_root: &Path, name: &str, line: usize) -> Result<(String, Option<String>)> {
    if name.is_empty() || name.contains('/') || name.contains("..") {
        bail!("line {line}: invalid component name `<lf-component name=\"{name}\">`");
    }
    let repo_dir = repo_root.join(".lf/components").join(name);
    let repo_template = repo_dir.join("template.html");
    if repo_template.is_file() {
        let html = std::fs::read_to_string(&repo_template).with_context(|| {
            format!(
                "line {line}: failed to read component template {}",
                repo_template.display()
            )
        })?;
        let style_path = repo_dir.join("style.css");
        let style = style_path
            .is_file()
            .then(|| std::fs::read_to_string(&style_path).ok())
            .flatten();
        return Ok((html, style));
    }
    if let Some((html, style)) = get_builtin_page_component(name) {
        return Ok((html.to_owned(), style.map(str::to_owned)));
    }
    bail!(
        "line {line}: unknown component `<lf-component name=\"{name}\">` (not in `.lf/components/{name}/template.html` or builtins)"
    )
}

fn render_component(
    template: &str,
    attrs: &BTreeMap<String, String>,
    inner: &str,
) -> Result<String> {
    let (named_slots, default_slot) = extract_slots(inner)?;

    let with_attrs = PLACEHOLDER_RE.replace_all(template, |caps: &Captures<'_>| {
        attrs
            .get(&caps[1])
            .map(|val| escape_html(val))
            .unwrap_or_default()
    });
    let with_named = NAMED_SLOT_RE.replace_all(&with_attrs, |caps: &Captures<'_>| {
        let name = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| m.as_str())
            .unwrap_or("");
        named_slots.get(name).cloned().unwrap_or_default()
    });
    let rendered =
        DEFAULT_SLOT_RE.replace_all(&with_named, |_caps: &Captures<'_>| default_slot.clone());
    Ok(rendered.into_owned())
}

fn extract_slots(inner: &str) -> Result<(BTreeMap<String, String>, String)> {
    let mut named: BTreeMap<String, String> = BTreeMap::new();
    let mut cursor = 0;

    // First handle `<slot name="X">...</slot>` elements inside component children.
    let mut remaining = String::new();
    while let Some(el) = find_next_element(inner, cursor, "slot")? {
        remaining.push_str(&inner[cursor..el.start]);
        if let Some(name) = el.attrs.get("name") {
            named.entry(name.clone()).or_default().push_str(&el.inner);
        } else {
            remaining.push_str(&el.inner);
        }
        cursor = el.end;
    }
    remaining.push_str(&inner[cursor..]);

    // Also support `<div slot="before">...</div>` or `<template slot="before">...`.
    let mut default_parts = String::new();
    let mut pos = 0;
    while let Some(caps) = SLOT_ATTR_RE.captures(&remaining[pos..]) {
        let full = caps.get(0).expect("full match");
        let start = pos + full.start();
        let tag_name = &caps[1];
        let slot_name = caps
            .get(3)
            .or_else(|| caps.get(4))
            .map(|m| m.as_str())
            .unwrap_or("")
            .to_owned();
        if let Some(el) = find_next_element(&remaining, start, tag_name)? {
            default_parts.push_str(&remaining[pos..el.start]);
            named.entry(slot_name).or_default().push_str(&el.inner);
            pos = el.end;
        } else {
            default_parts.push_str(&remaining[pos..start + full.len()]);
            pos = start + full.len();
        }
    }
    default_parts.push_str(&remaining[pos..]);
    Ok((named, default_parts))
}

#[cfg(test)]
mod tests {
    use super::render_component;
    use std::collections::BTreeMap;

    #[test]
    fn component_rendering_escapes_attrs_and_fills_default_and_named_slots() {
        let tmpl = "<div data-t=\"{{title}}\"><slot name=\"before\"></slot>|<slot name=\"after\"></slot>|<slot></slot></div>";
        let attrs = BTreeMap::from([("title".into(), "A < B & \"C\"".into())]);
        let inner = "<div slot=\"before\">old</div><div slot=\"after\">new</div><p>body</p>";
        let rendered = render_component(tmpl, &attrs, inner).unwrap();
        assert_eq!(
            rendered,
            "<div data-t=\"A &lt; B &amp; &quot;C&quot;\">old|new|<p>body</p></div>"
        );
    }
}
