//! Built-in step definitions and system docs embedded in the binary.
//!
//! Registration is automatic: drop a file into the right builtins/
//! subdirectory and build.rs generates the HashMap entries.

/// Bundled OPERATE.md - opt-in loopflow operating guidance for agents.
pub const OPERATE_DOC: &str = include_str!("builtins/OPERATE.md");

/// Surface instruction prompts, one per surface variant.
pub const SURFACE_HEADLESS: &str = include_str!("builtins/surfaces/headless.md");
pub const SURFACE_CLI: &str = include_str!("builtins/surfaces/cli.md");
pub const SURFACE_CONCERTO_MAC: &str = include_str!("builtins/surfaces/concerto_mac.md");
pub const SURFACE_CONCERTO_IPHONE: &str = include_str!("builtins/surfaces/concerto_iphone.md");

/// Returns the content of a built-in step, if it exists.
pub fn get_builtin_step(name: &str) -> Option<&'static str> {
    BUILTIN_STEPS.get(name).copied()
}

/// One-line description for a built-in step. Prefers the `description:` frontmatter
/// field, falling back to the first prose line after the closing `---`.
/// Returns an empty string if neither is present.
pub fn builtin_step_description(name: &str) -> String {
    let Some(content) = get_builtin_step(name) else {
        return String::new();
    };
    step_description_from_content(content)
}

/// One-line description for a built-in flow, drawn from the first non-blank,
/// non-YAML-sequence comment line in the YAML. Returns an empty string if
/// nothing descriptive is found.
pub fn builtin_flow_description(name: &str) -> String {
    let Some(content) = get_builtin_flow(name) else {
        return String::new();
    };
    flow_description_from_content(content)
}

fn step_description_from_content(content: &str) -> String {
    // 1. Try `description:` in the frontmatter block.
    if let Some(desc) = frontmatter_description(content) {
        return first_line(&desc);
    }

    // 2. Fall back to first prose line after frontmatter.
    let body = strip_frontmatter(content);
    first_prose_line(body)
}

fn frontmatter_description(content: &str) -> Option<String> {
    let stripped = content.strip_prefix("---")?;
    let end = stripped.find("\n---")?;
    let frontmatter = &stripped[..end];
    let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(frontmatter).ok()?;
    let desc = value
        .as_mapping()?
        .get(serde_yaml_ng::Value::String("description".to_string()))?;
    Some(desc.as_str()?.trim().to_string())
}

fn flow_description_from_content(content: &str) -> String {
    for raw in content.lines() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("# ") {
            return trimmed.trim_start_matches('#').trim().to_string();
        }
        if !trimmed.is_empty() {
            break;
        }
    }
    String::new()
}

fn strip_frontmatter(content: &str) -> &str {
    let Some(stripped) = content.strip_prefix("---") else {
        return content;
    };
    let Some(end) = stripped.find("\n---") else {
        return content;
    };
    stripped[end + 4..].trim_start_matches('\n')
}

fn first_prose_line(body: &str) -> String {
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        // Skip section headings and code fences.
        if line.starts_with('#') || line.starts_with("```") {
            continue;
        }
        return line.to_string();
    }
    String::new()
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().to_string()
}

/// Returns the content of a built-in flow, if it exists.
pub fn get_builtin_flow(name: &str) -> Option<&'static str> {
    BUILTIN_FLOWS.get(name).copied()
}

/// Returns the content of a built-in goal, if it exists.
pub fn get_builtin_goal(name: &str) -> Option<&'static str> {
    BUILTIN_GOALS.get(name).copied()
}

/// Resolve a bare name to its builtin step key. Returns the exact match if one
/// exists; otherwise, if exactly one namespaced key ends with `/{name}`, returns
/// that key. Returns `None` for no match or ambiguous matches.
pub fn resolve_builtin_step(name: &str) -> Option<&'static str> {
    if let Some((key, _)) = BUILTIN_STEPS.get_key_value(name) {
        return Some(key);
    }
    resolve_bare_in_map(name, &BUILTIN_STEPS)
}

/// Resolve a bare name to its builtin flow key. Returns the exact match if one
/// exists; otherwise, if exactly one namespaced key ends with `/{name}`, returns
/// that key. Returns `None` for no match or ambiguous matches.
pub fn resolve_builtin_flow(name: &str) -> Option<&'static str> {
    if let Some((key, _)) = BUILTIN_FLOWS.get_key_value(name) {
        return Some(key);
    }
    resolve_bare_in_map(name, &BUILTIN_FLOWS)
}

/// Resolve a bare name to its builtin goal key.
pub fn resolve_builtin_goal(name: &str) -> Option<&'static str> {
    if let Some((key, _)) = BUILTIN_GOALS.get_key_value(name) {
        return Some(key);
    }
    resolve_bare_in_map(name, &BUILTIN_GOALS)
}

fn resolve_bare_in_map(
    bare: &str,
    map: &std::collections::HashMap<&'static str, &'static str>,
) -> Option<&'static str> {
    if bare.contains('/') {
        return None;
    }
    let suffix = format!("/{bare}");
    let mut matches = map.keys().filter(|key| key.ends_with(&suffix)).copied();
    let first = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some(first)
    }
}

/// Returns the content of a built-in direction, if it exists.
pub fn get_builtin_direction(name: &str) -> Option<&'static str> {
    BUILTIN_DIRECTIONS.get(name).copied()
}

/// Returns the content of a built-in ops prompt, if it exists.
pub fn get_builtin_ops_prompt(name: &str) -> Option<&'static str> {
    BUILTIN_OPS_PROMPTS.get(name).copied()
}

/// List of all built-in step names.
pub fn builtin_step_names() -> Vec<&'static str> {
    BUILTIN_STEPS.keys().copied().collect()
}

/// List of all built-in flow names.
pub fn builtin_flow_names() -> Vec<&'static str> {
    BUILTIN_FLOWS.keys().copied().collect()
}

/// Iterate over all built-in flows as (name, yaml_content) pairs.
pub fn builtin_flow_entries() -> impl Iterator<Item = (&'static str, &'static str)> {
    BUILTIN_FLOWS.iter().map(|(k, v)| (*k, *v))
}

/// List of all built-in direction names.
pub fn builtin_direction_names() -> Vec<&'static str> {
    BUILTIN_DIRECTIONS.keys().copied().collect()
}

/// Returns the member direction names for a builtin group, if it exists.
pub fn builtin_direction_group(name: &str) -> Option<&'static Vec<&'static str>> {
    BUILTIN_DIRECTION_GROUPS.get(name)
}

/// List of all builtin direction group names.
pub fn builtin_direction_group_names() -> Vec<&'static str> {
    BUILTIN_DIRECTION_GROUPS.keys().copied().collect()
}

// Generated by build.rs — scans builtins/ subdirectories automatically.
include!(concat!(env!("OUT_DIR"), "/builtin_steps.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_flows.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_goals.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_flow_categories.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_step_categories.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_directions.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_direction_groups.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_ops_prompts.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    const WAVE_AUTHORING_DOC: &str = include_str!("../../../../docs/wave-authoring.md");

    #[test]
    fn priority_guidance_is_embedded_in_prompts_and_docs() {
        let update_wave = get_builtin_step("update-wave").expect("update-wave prompt");
        let ingest = get_builtin_step("ingest").expect("ingest prompt");
        let design = get_builtin_step("design").expect("design prompt");
        let scan_waves = get_builtin_step("scan").expect("scan prompt");
        let split_wave = get_builtin_step("split-wave").expect("split-wave prompt");

        assert!(update_wave.contains("1-fix-broken-build.md"));
        assert!(update_wave.contains("4-*"));
        assert!(!update_wave.contains("numbered item files"));
        assert!(ingest.contains("highest-priority non-empty level"));
        assert!(design.contains("1-*.md"));
        assert!(scan_waves.contains("1-*` through `4-*"));
        assert!(!scan_waves.contains("All numbered item files"));
        assert!(split_wave.contains("Bucketed roadmap files"));
        assert!(WAVE_AUTHORING_DOC.contains("1-fix-crash-loop.md"));
        assert!(WAVE_AUTHORING_DOC.contains("2-daemon-integrity.md"));
        assert!(!WAVE_AUTHORING_DOC.contains("all `01-*` items complete"));
    }
}
