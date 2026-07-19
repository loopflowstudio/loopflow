//! Built-in skill definitions and system docs embedded in the binary.
//!
//! Registration is automatic: drop a file into the right builtins/
//! subdirectory and build.rs generates the HashMap entries.

/// Bundled LOOPFLOW.md - the one loopflow operating document every launched
/// agent receives, including the speech vocabulary (`lf chat`, `lf memory`).
pub const LOOPFLOW_DOC: &str = include_str!("builtins/LOOPFLOW.md");

/// Headless preamble — the only surface that needs one (no user is present).
pub const SURFACE_HEADLESS: &str = include_str!("builtins/surfaces/headless.md");

/// Returns the content of a built-in skill, if it exists.
pub fn get_builtin_skill(name: &str) -> Option<&'static str> {
    BUILTIN_STEPS.get(name).copied()
}

/// One-line description for a built-in skill. Prefers the `description:` frontmatter
/// field, falling back to the first prose line after the closing `---`.
/// Returns an empty string if neither is present.
pub fn builtin_skill_description(name: &str) -> String {
    let Some(content) = get_builtin_skill(name) else {
        return String::new();
    };
    skill_description_from_content(content)
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

fn skill_description_from_content(content: &str) -> String {
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

/// Resolve a bare name to its builtin skill key. Returns the exact match if one
/// exists; otherwise, if exactly one namespaced key ends with `/{name}`, returns
/// that key. Returns `None` for no match or ambiguous matches.
pub fn resolve_builtin_skill(name: &str) -> Option<&'static str> {
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

/// List of all built-in skill names.
pub fn builtin_skill_names() -> Vec<&'static str> {
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
include!(concat!(env!("OUT_DIR"), "/builtin_skills.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_flows.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_goals.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_flow_categories.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_skill_categories.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_directions.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_direction_groups.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_ops_prompts.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    const WAVES_DOC: &str = include_str!("../../../../docs/waves.md");

    #[test]
    fn wave_model_is_embedded_in_prompts_and_docs() {
        let update_wave = get_builtin_skill("update-wave").expect("update-wave prompt");
        let design = get_builtin_skill("design").expect("design prompt");
        let scan_waves = get_builtin_skill("scan").expect("scan prompt");

        // The roadmap lives in Linear, reached via `lf pm` — no local N-*.md files.
        assert!(update_wave.contains("lf pm"));
        assert!(update_wave.contains("MEMORY.md"));
        assert!(!update_wave.contains("1-fix-broken-build.md"));
        assert!(design.contains("lf pm"));
        assert!(design.contains("GOAL.md"));
        assert!(!design.contains("1-*.md"));
        assert!(scan_waves.contains("lf pm show"));
        assert!(WAVES_DOC.contains("GOAL.md"));
        assert!(WAVES_DOC.contains("Linear"));
        assert!(!WAVES_DOC.contains("1-fix-crash-loop.md"));

        // The ingest skill is gone; workers are handed their task at dispatch.
        assert!(get_builtin_skill("ingest").is_none());
    }

    #[test]
    fn interactive_skills_support_human_and_parent_feedback() {
        for name in [
            "design",
            "explore",
            "refine",
            "review-design",
            "review-open-work",
            "review",
            "init",
        ] {
            let skill = get_builtin_skill(name).expect("interactive builtin skill");
            assert!(
                skill.contains("## Reviewer mode"),
                "{name} does not define reviewer modes"
            );
            assert!(
                skill.contains("**Human reviewer:**"),
                "{name} does not define attended behavior"
            );
            assert!(
                skill.contains("**Parent reviewer:**"),
                "{name} does not define headless parent behavior"
            );
            assert!(
                skill.contains("review protocol"),
                "{name} does not route parent dialogue through the review protocol"
            );
        }

        let review_slice = get_builtin_skill("review-slice").expect("review-slice skill");
        for evidence in [
            "interactive: false",
            "production-like path",
            "Done when",
            "Implemented behavior",
            "complete diff",
            "lf pr publish",
        ] {
            assert!(
                review_slice.contains(evidence),
                "review-slice omits {evidence:?} evidence"
            );
        }
        assert!(get_builtin_skill("demo").is_none());
        assert!(get_builtin_skill("code-review").is_none());
    }

    #[test]
    fn execution_context_grants_delegation_by_tier() {
        assert!(LOOPFLOW_DOC.contains("Execute Here First"));
        assert!(!LOOPFLOW_DOC.contains("lf pm show"));
        assert!(!LOOPFLOW_DOC.contains("--detach"));

        let wave = get_builtin_skill("wave_pursue").expect("wave pursue");
        assert!(wave.contains("lf task run <issue-id>"));
        assert!(wave.contains("lf task status"));
        assert!(wave.contains("stable worktree"));

        let project = get_builtin_skill("project_pursue").expect("project pursue");
        assert!(project.contains("lf task run <issue-id>"));
        assert!(!project.contains("lf loop"));

        let task = get_builtin_skill("task_pursue").expect("task pursue");
        assert!(task.contains("second Task"));
        assert!(task.contains("lf pr land"));
        assert!(task.contains("pinned final flow"));
        assert!(!task.contains("lf pm task done"));
        assert!(task.contains("lf pm task create"));

        for (flow, steps) in [
            ("wave", ["wave_clarify", "wave_pursue", "wave_mutate"]),
            (
                "project",
                ["project_clarify", "project_pursue", "project_mutate"],
            ),
        ] {
            let flow = get_builtin_flow(flow).expect("tier flow");
            for step in steps {
                assert!(flow.contains(&format!("- {step}")));
            }
            assert!(!flow.contains("loop:"));
        }

        assert!(get_builtin_flow("task").is_none());
        assert!(get_builtin_flow("task-design")
            .expect("Task first flow")
            .contains("- kickoff"));
        assert!(get_builtin_flow("slice")
            .expect("Task loop flow")
            .contains("- review-slice"));
        assert!(get_builtin_flow("ship")
            .expect("Task final flow")
            .contains("- op: pr land -c"));
    }

    #[test]
    fn builtin_skills_do_not_name_retired_child_controls() {
        let retired = [
            "lf handoff",
            "lf reviews",
            "lf task follow-up",
            "lf task receipt",
            "lf task acknowledge",
            "lf task decide",
            "lf task request-decision",
            "lf task review",
            "lf project follow-up",
            "lf project receipt",
            "lf project acknowledge",
            "lf project decide",
            "lf project request-decision",
        ];

        for (name, skill) in BUILTIN_STEPS.iter() {
            for command in retired {
                assert!(
                    !skill.contains(command),
                    "{name} still instructs agents to run retired `{command}`"
                );
            }
        }
    }

    #[test]
    fn task_design_and_multi_task_outputs_share_the_machine_contract() {
        let clarify = get_builtin_skill("task_clarify").expect("task clarify");
        for requirement in [
            "User-visible outcome",
            "End-to-end proof",
            "Source of truth",
            "Affected surfaces and consumers",
            "Absent and error states",
            "Operational boundary",
            "Exclusions",
            "implementation receipts",
        ] {
            assert!(clarify.contains(requirement));
        }

        for name in [
            "wave_pursue",
            "project_pursue",
            "scan",
            "assess",
            "wave-report",
        ] {
            let skill = get_builtin_skill(name).expect("multi-Task output skill");
            assert!(skill.contains("lf roadmap"));
            assert!(skill.contains("lf status"));
            assert!(skill.contains(
                "[identifier](provider URL) — readable active PR/workspace slug — status/next owner"
            ));
            assert!(skill.contains("task.identifier"));
            assert!(skill.contains("reference.issue_url"));
            assert!(skill.contains("active_pr.slug"));
            assert!(skill.contains("reference.workspace.slug"));
            assert!(skill.contains("runtime.status"));
            assert!(skill.contains("next_move.owner"));
            assert!(skill.contains("reconstruct a provider URL"));
            assert!(skill.contains("is explicitly absent"));
        }
    }

    #[test]
    fn build_is_one_bounded_pass_without_delivery() {
        let flow = get_builtin_flow("build").expect("build flow");
        for step in ["kickoff", "implement", "compress", "lint", "gate"] {
            assert!(flow.contains(&format!("- {step}")));
        }
        assert!(flow.contains("name: review-design"));
        assert!(flow.contains("feedback: true"));
        assert!(!flow.contains("loop:"));
        assert!(!flow.contains("deploy"));
        assert!(!flow.contains("pr land"));
    }

    #[test]
    fn generic_execution_skills_never_infer_a_wave_or_require_pm() {
        for name in ["implement", "gate", "qa", "research", "rebase"] {
            let skill = get_builtin_skill(name).expect("generic skill");
            assert!(skill.contains("seed names the exact wave"), "{name}");
            assert!(!skill.contains("matches this work"), "{name}");
            assert!(!skill.contains("lf pm show"), "{name}");
        }
    }

    #[test]
    fn vsm_system_goals_are_registered() {
        let key = resolve_builtin_goal("s3").expect("s3 goal");
        let goal = get_builtin_goal(key).expect("registered goal");

        assert_eq!(key, "s3");
        assert!(goal.contains("True north: the whole is worth more than the sum of its parts."));
    }

    #[test]
    fn retired_export_memory_skill_is_not_registered() {
        assert!(get_builtin_skill("export-memory").is_none());
    }

    #[test]
    fn project_promotion_is_an_authored_flow() {
        let flow = get_builtin_flow("project-promote").expect("promotion flow");
        let skill = get_builtin_skill("project-promote").expect("promotion skill");

        assert!(flow.contains("- project-promote"));
        assert!(skill.contains("lf pm show --wave <parent> --project <slug> --json"));
        assert!(skill.contains("parent_wave_id"));
        assert!(skill.contains("lf pm show"));
        assert!(skill.contains("lf radio pub --parent"));
    }
}
