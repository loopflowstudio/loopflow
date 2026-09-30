//! Built-in skill definitions and system docs embedded in the binary.
//!
//! Registration is automatic: drop a file into the right builtins/
//! subdirectory and build.rs generates the HashMap entries.

/// Bundled LOOPFLOW.md - the one loopflow operating document every launched
/// agent receives, including the Work and Session vocabulary.
pub const LOOPFLOW_DOC: &str = include_str!("builtins/LOOPFLOW.md");

/// Headless preamble — the only surface that needs one (no user is present).
pub const SURFACE_HEADLESS: &str = include_str!("builtins/surfaces/headless.md");

/// Shared contract for every surface with someone participating in the current conversation.
pub const SURFACE_HUMAN_PRESENT: &str = include_str!("builtins/surfaces/human-present.md");

/// Live chat channel (e.g. Discord): a person is reading, so the reply is a
/// plain, deliberate, addressed message — never a working transcript.
pub const SURFACE_CHAT: &str = include_str!("builtins/surfaces/chat.md");

/// Returns the content of a built-in skill, if it exists.
pub fn get_builtin_skill(name: &str) -> Option<&'static str> {
    BUILTIN_SKILLS.get(name).copied()
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
    if let Some((key, _)) = BUILTIN_SKILLS.get_key_value(name) {
        return Some(key);
    }
    resolve_bare_in_map(name, &BUILTIN_SKILLS)
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

/// List of all built-in skill names.
pub fn builtin_skill_names() -> Vec<&'static str> {
    BUILTIN_SKILLS.keys().copied().collect()
}

/// List of all built-in flow names.
pub fn builtin_flow_names() -> Vec<&'static str> {
    BUILTIN_FLOWS.keys().copied().collect()
}

/// Iterate over all built-in flows as (name, yaml_content) pairs.
pub fn builtin_flow_entries() -> impl Iterator<Item = (&'static str, &'static str)> {
    BUILTIN_FLOWS.iter().map(|(k, v)| (*k, *v))
}

// Generated by build.rs — scans builtins/ subdirectories automatically.
include!(concat!(env!("OUT_DIR"), "/builtin_skills.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_flows.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_goals.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_flow_categories.rs"));
include!(concat!(env!("OUT_DIR"), "/builtin_skill_categories.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    const WAVES_DOC: &str = include_str!("../../../../docs/waves.md");

    #[test]
    fn wave_model_is_embedded_in_prompts_and_docs() {
        let design = get_builtin_skill("design").expect("design prompt");
        let wave = get_builtin_skill("wave/operate").expect("Wave operation");

        // The roadmap lives in Linear, reached via `lf wave sync` — no local N-*.md files.
        assert!(design.contains("lf wave status"));
        assert!(design.contains("GOAL.md"));
        assert!(!design.contains("1-*.md"));
        assert!(wave.contains("lf wave status"));
        assert!(WAVES_DOC.contains("GOAL.md"));
        assert!(WAVES_DOC.contains("Linear"));
        assert!(!WAVES_DOC.contains("1-fix-crash-loop.md"));

        // The ingest skill is gone; workers are handed their task at dispatch.
        assert!(get_builtin_skill("ingest").is_none());
    }

    #[test]
    fn wave_goal_authoring_keeps_live_metrics_wave_owned() {
        let prompt = get_builtin_skill("prompt").expect("prompt skill");

        assert!(prompt.contains("Wave-owned metrics"));
        assert!(prompt.contains("never copy them"));
        assert!(prompt.contains("into the Wave body"));
        assert!(!prompt.contains("Readable measures"));
    }

    #[test]
    fn project_guidance_sponsors_metrics_and_task_workers_propose_them() {
        let normalize = |prompt: &str| prompt.split_whitespace().collect::<Vec<_>>().join(" ");

        let project = normalize(get_builtin_skill("wave/operate").expect("project operate"));
        assert!(project.contains("For each sponsored metric that moved"));
        assert!(project.contains("A Met frontier may keep a worker"));
        assert!(project.contains("a Met guardrail stays quiet until its alarm"));
        assert!(project.contains("intent graph, not a control plane"));

        let task = normalize(get_builtin_skill("implement").expect("implementation"));
        assert!(task.contains("While building feature work, notice signals"));
        assert!(task.contains("otherwise leave the proposal for Wave sponsorship"));
        assert!(task.contains("Metric proposals are discoveries, not a completion quota"));
    }

    #[test]
    fn formerly_attended_skills_have_bounded_headless_contracts() {
        let design = get_builtin_skill("design").expect("design skill");
        for contract in [
            "## Surface",
            "**Interactive:**",
            "**Headless:**",
            "without claiming User confirmation",
        ] {
            assert!(design.contains(contract), "design is missing {contract}");
        }

        let launch_plan = get_builtin_skill("launch-plan").expect("launch-plan skill");
        for contract in [
            "Do not create a manifest, receipt, marker, or new planning state",
            "Keep that core in this Task",
            "lf task create --run",
            "--flow <chosen-flow>",
            "lf task checkout <issue> --json",
            "lf task run <issue> --flow <chosen-flow>",
            "Use the Flow the user selected",
        ] {
            assert!(
                launch_plan.contains(contract),
                "launch-plan is missing {contract}"
            );
        }

        for name in ["refine", "review-open-work", "init"] {
            let skill = get_builtin_skill(name).expect("builtin skill");
            assert!(
                skill.contains("## Reviewer mode"),
                "{name} does not define reviewer modes"
            );
            assert!(
                skill.contains("**Interactive reviewer:**"),
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

        let demo = get_builtin_skill("demo").expect("demo skill");
        for contract in [
            "human feedback, revised artifact references, and remaining",
            "headless surface",
            "run `lf ask \"<exact request>\"`",
            "closing, detaching, provider exit, or lack of response",
        ] {
            assert!(demo.contains(contract), "demo omits {contract:?}");
        }
        for (name, skill) in BUILTIN_SKILLS.iter() {
            assert!(
                !skill.lines().any(|line| line.starts_with("interactive:")),
                "{name} still carries retired interactive scheduling metadata"
            );
        }
        assert!(get_builtin_skill("code-review").is_none());
    }

    #[test]
    fn init_connects_the_distributed_system_before_customizing_skills() {
        let init = get_builtin_skill("init").expect("init skill");

        for command in [
            "lf auth status",
            "lf auth route show",
            "lf home id --json",
            "lf wave list --json",
            "lf wave status <wave> --json",
            "lf roadmap --wave <wave> --json",
            "lf task run <ISSUE-ID>",
            "lf home observe <home-id>",
            "lf ssh <home-id> --wave <wave> wave/operate",
        ] {
            assert!(init.contains(command), "init omits {command:?}");
        }

        assert!(init.contains("Loopflow is not primarily a prompt launcher"));
        assert!(init.contains("Installed harnesses are a capability of this Home"));
        assert!(init.contains("team-wide repo configuration"));
        assert!(init.contains("wave/<name>/MEMORY.md"));
        assert!(!init.contains("lf memory"));
        assert!(!init.contains("git remote -v"));
        assert!(!init.contains("lf doctor --json"));
    }

    #[test]
    fn execution_context_grants_delegation_by_tier() {
        assert!(LOOPFLOW_DOC.contains("Execute Here First"));
        assert!(LOOPFLOW_DOC.contains("lf screenshot SOURCE -o OUTPUT"));
        assert!(LOOPFLOW_DOC.contains("never launch a GUI browser executable"));
        assert!(!LOOPFLOW_DOC.contains("lf pm show"));
        assert!(!LOOPFLOW_DOC.contains("--detach"));

        let wave = get_builtin_skill("wave/operate").expect("wave operate");
        assert!(wave.contains("lf task run <issue-id>"));
        assert!(wave.contains("lf task status"));
        assert!(wave.contains("Tasks progress independently"));
        assert!(wave.contains("S5 · Identity"));
        assert!(!wave.contains("lf loop"));

        let task = get_builtin_skill("implement").expect("implementation");
        assert!(task.contains("second Task"));
        assert!(task.contains("Publication, landing and navigation belong to the caller"));
        assert!(!task.contains("lf pm task done"));

        assert!(get_builtin_flow("task").is_none());
        let first = get_builtin_flow("task-design").expect("Task first flow");
        assert!(first.contains("name: kickoff"));
        assert_eq!(first.matches("human: true").count(), 1);
        assert!(first.contains("id: review_kickoff"));
        assert!(!get_builtin_flow("incident")
            .expect("incident first flow")
            .contains("human:"));
        assert!(get_builtin_flow("pursue")
            .expect("Task loop flow")
            .contains("flow: refresh"));
        assert!(get_builtin_flow("ship")
            .expect("Task final flow")
            .contains("- cmd: pr land -c"));

        for wrapper in ["design", "launch-plan", "ship-5whys", "wave"] {
            assert!(get_builtin_flow(wrapper).is_none());
        }
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

        for (name, skill) in BUILTIN_SKILLS.iter() {
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
        let kickoff = get_builtin_skill("kickoff").expect("implementation plan");
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
            assert!(kickoff.contains(requirement));
        }

        for name in ["wave/operate", "wave-report"] {
            let skill = get_builtin_skill(name).expect("multi-Task output skill");
            assert!(skill.contains("lf roadmap"));
            assert!(skill.contains("lf wave status"));
            assert!(skill.contains("task.identifier"));
            assert!(skill.contains("reference.issue_url"));
            assert!(skill.contains("active_pr.slug"));
            assert!(skill.contains("reference.workspace.slug"));
            assert!(skill.contains("runtime.status"));
            assert!(skill.contains("next_move.owner"));
        }
    }

    #[test]
    fn consolidated_catalog_removes_variants_without_aliases() {
        for name in [
            "restore",
            "expand",
            "integrate-upstream",
            "ship-5whys",
            "task/clarify",
            "task/pursue",
            "task/mutate",
            "iterate",
            "explore",
            "scan",
            "assess",
            "mutate",
            "review",
            "vsm/operate",
            "s2-scan",
            "s2-assess",
            "s3-scan",
            "s3-assess",
            "s4-scan",
            "s4-assess",
            "s5-scan",
            "s5-assess",
        ] {
            assert!(get_builtin_skill(name).is_none(), "{name}");
        }
        for name in [
            "build",
            "slice",
            "pair",
            "design-and-ship",
            "sync",
            "launch-plan",
            "garden",
            "garden-act",
            "build-or-silent",
            "s1-build",
            "govern-operations",
            "govern-control",
            "govern-coordination",
            "govern-intelligence",
            "govern-identity",
        ] {
            assert!(get_builtin_flow(name).is_none(), "{name}");
        }
        for name in [
            "design",
            "kickoff",
            "debug",
            "s1",
            "s2",
            "s3",
            "s4",
            "s5",
            "reduce",
            "polish",
            "concept-review",
            "review-design",
            "research",
            "qa",
            "triage",
            "gate",
            "demo",
        ] {
            assert!(get_builtin_skill(name).is_some(), "{name}");
        }
    }

    #[test]
    fn release_run_names_its_durable_restart_contract() {
        let skill = get_builtin_skill("release-run").expect("release-run skill");
        let skill = skill.split_whitespace().collect::<Vec<_>>().join(" ");

        for contract in [
            "Process death does not make generated release state temporary",
            "release-candidate/<target>/<tag>/<commit>",
            "prepare-<target>-<tag>-<commit>",
            "publish-<target>-<tag>",
            ".lf/releases/<tag>/<commit>-<workflow-run-id>",
            "observe → classify → converge or refuse",
            "Mismatched, dirty, differently registered, or live-owned state fails closed",
            "must not manually remove a stale generated worktree, ref, or artifact directory",
        ] {
            assert!(skill.contains(contract), "release-run omits {contract:?}");
        }
    }

    #[test]
    fn generic_execution_skills_never_infer_a_wave_or_require_pm() {
        for name in ["implement", "gate", "qa", "research", "rebase-conflicts"] {
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
    fn chapter_skills_are_registered() {
        for name in [
            "start-chapter",
            "review-chapter",
            "wave/start-chapter",
            "wave/review-chapter",
        ] {
            assert!(get_builtin_skill(name).is_some(), "{name}");
            assert!(
                !builtin_skill_description(name).is_empty(),
                "{name} needs a catalog description"
            );
        }
        assert_eq!(
            resolve_builtin_skill("start-chapter"),
            Some("start-chapter")
        );
        assert_eq!(
            resolve_builtin_skill("review-chapter"),
            Some("review-chapter")
        );
    }

    #[test]
    fn retired_memory_skills_are_not_registered() {
        for name in ["export-memory", "update-wave", "record-learnings"] {
            assert!(get_builtin_skill(name).is_none(), "{name}");
        }
    }
}
