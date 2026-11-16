pub fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    if !content.starts_with("---") {
        return None;
    }
    let mut parts = content.splitn(3, "---");
    let _ = parts.next();
    let frontmatter = parts.next()?;
    let rest = parts.next()?;
    let body = rest.strip_prefix('\n').unwrap_or(rest);
    Some((frontmatter, body))
}

pub fn skill_description(content: &str) -> Option<String> {
    let Some((frontmatter, body)) = split_frontmatter(content) else {
        return first_prose_line(content);
    };
    serde_yaml_ng::from_str::<serde_yaml_ng::Value>(frontmatter)
        .ok()
        .and_then(|value| {
            value
                .get("description")?
                .as_str()?
                .lines()
                .next()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
        })
        .or_else(|| first_prose_line(body))
}

pub fn first_prose_line(body: &str) -> Option<String> {
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("```") {
            continue;
        }
        return Some(line.to_string());
    }
    None
}
