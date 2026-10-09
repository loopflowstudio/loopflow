//! Render the compiler's captured plan for execution in the current conversation.
use crate::engine::flow::return_target;
use crate::engine::flow_output::FlowOutput;
use crate::engine::{ConcreteStep, LoadError, Skill};

pub fn render_flow_instructions(name: &str, steps: &[ConcreteStep]) -> Result<String, LoadError> {
    let mut output = format!("# Flow: {}\n\n", name);
    output.push_str("Carry out this frozen plan in the current conversation using the current request and accumulated evidence. Apply the resolved skill bodies below here; naming another slash command is not execution. Do not launch `lf run`, new Sessions, or workers to carry out these steps.\n\n\
Track the current occurrence, loop pass, decision and pending input in the transcript. Follow every command, loop, branch and review boundary. Stop on command failure. Never automatically replay a successful side effect after interruption: inspect evidence first; if the position is uncertain, ask here.\n\n\
A review requires the participant's response in this conversation; never fabricate approval or substitute an agent. If no participant is available, explain the missing input and stop. A blocked decision stops here pending new direction. Correct invalid decisions at that decision, without rerunning preceding work. There is no arbitrary iteration limit.\n\n\
The current model performs the work. Agent preferences and native declarations below are disclosed, not permission to switch providers or bypass controls. Preserve resource bases. If a step requires unavailable native execution, explain the specific limitation and stop. Source edits take effect only on a fresh invocation. This instruction-driven plan has no autonomous driver's enforcement or crash recovery; it creates no FlowProcess or step Process and does not advance a Task's Workflow. Explicit LF commands keep their normal effects and authorization boundaries. Completing this chat Flow is not Task completion.\n\n## Plan\n\n");
    let mut bodies = Vec::new();
    render_steps(steps, "", &mut bodies, &mut output)?;
    output.push_str("\n## Resolved skill bodies\n");
    for (index, skill) in bodies.iter().enumerate() {
        output.push_str(&format!("\n### Body {}: {}\n\n", index + 1, skill.name));
        if let Some(source) = &skill.source {
            output.push_str(&format!(
                "Source: {} ({:?})\nBase directory for this skill: {}\n\n",
                source.path.display(),
                source.dialect,
                source.path.parent().expect("source has a parent").display()
            ));
        }
        if skill.agent.is_some() || skill.default_agent.is_some() || skill.action_style.is_some() {
            output.push_str(&format!("Authored preferences: agent={:?}, default_agent={:?}, action_style={:?}. These do not switch the current model.\n\n", skill.agent, skill.default_agent, skill.action_style));
        }
        output.push_str(&skill.source_text());
        output.push('\n');
    }
    Ok(output)
}

fn body_number<'a>(skill: &'a Skill, bodies: &mut Vec<&'a Skill>) -> usize {
    if let Some(index) = bodies.iter().position(|body| *body == skill) {
        index + 1
    } else {
        bodies.push(skill);
        bodies.len()
    }
}

fn render_steps<'a>(
    steps: &'a [ConcreteStep],
    prefix: &str,
    bodies: &mut Vec<&'a Skill>,
    output: &mut String,
) -> Result<(), LoadError> {
    for (index, step) in steps.iter().enumerate() {
        let position = format!("{prefix}{}", index + 1);
        match step {
            ConcreteStep::Skill(occurrence) => {
                let body = body_number(&occurrence.skill, bodies);
                output.push_str(&format!(
                    "{position}. Apply **{}** (body {body})",
                    occurrence.skill.name
                ));
                if let Some(id) = &occurrence.id {
                    output.push_str(&format!("; occurrence ID `{id}`"));
                }
                output.push_str(".\n");
                if occurrence.human {
                    output.push_str("   Review with the participant here and pause for their response before continuing.\n");
                }
                if occurrence.returns.is_some() {
                    let target = return_target(steps, index).ok_or_else(|| {
                        LoadError::InvalidFlow(format!(
                            "invalid compiled return target at {position}"
                        ))
                    })?;
                    output.push_str(&format!("   Loop decision: iterate returns to {prefix}{} and repeats the entire range {prefix}{} through {position}, inclusive (including intervening commands and branches). Advance continues after {position}. Blocked explains missing input and stops here.\n   Decision contract: {}\n", target + 1, target + 1, FlowOutput::Decision.schema()));
                }
            }
            ConcreteStep::Command(command) => {
                let argv = command.item.argv();
                let quoted = argv
                    .iter()
                    .map(|arg| crate::engine::process::shell_escape(arg))
                    .collect::<Vec<_>>()
                    .join(" ");
                output.push_str(&format!("{position}. Execute this exact LF argv in order; stop on failure:\n\n```sh\n{quoted}\n```\n\n"));
            }
            ConcreteStep::Xor(branch) => {
                let body = body_number(&branch.router, bodies);
                let mut paths: Vec<_> = branch.paths.keys().cloned().collect();
                paths.sort();
                output.push_str(&format!("{position}. Route with **{}** (body {body}). Choose exactly one declared path, complete it, then rejoin after {position}. An invalid route never defaults to a branch.\n   Route contract: {}\n", branch.router.name, FlowOutput::Route(paths.clone()).schema()));
                for name in paths {
                    let path = &branch.paths[&name];
                    output.push_str(&format!("\nPath `{name}`: {}\n\n", path.description));
                    render_steps(&path.steps, &format!("{position}[{name}]."), bodies, output)?;
                    output.push_str(&format!("End path `{name}`: rejoin after {position}.\n\n"));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::render_flow_instructions;
    use crate::engine::flow::{compile_flow, load_authored_flow};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn portable_plan_renders_compiled_loops_branches_reviews_and_frozen_bodies() {
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
        fs::create_dir_all(repo.path().join(".claude/skills/native-check")).unwrap();
        fs::write(
            repo.path().join(".lf/skills/work.md"),
            "Original work body.",
        )
        .unwrap();
        fs::write(
            repo.path().join(".lf/skills/loop-or-next.md"),
            "Decide using evidence.",
        )
        .unwrap();
        fs::write(
            repo.path().join(".lf/skills/xor-route.md"),
            "Route using findings.",
        )
        .unwrap();
        fs::write(
            repo.path().join(".claude/skills/native-check/SKILL.md"),
            "---\nname: NativeCheck\nallowed-tools: Read\n---\nOriginal native body.",
        )
        .unwrap();
        fs::write(
            repo.path().join(".lf/flows/repair.yaml"),
            "- work\n- loop: work\n",
        )
        .unwrap();
        fs::write(
            repo.path().join(".lf/flows/chat.yaml"),
            r#"
- step: {name: work, id: first}
- cmd: commit -m don't-expand-$HOME;echo-unsafe
- step: {name: work, id: second}
- loop: first
- xor:
    paths:
      repair:
        flow: repair
        description: Repair the findings
      skip:
        description: Nothing to repair
- step: {name: native-check, id: review, human: true}
"#,
        )
        .unwrap();
        let steps = compile_flow(
            &load_authored_flow("chat", repo.path()).unwrap(),
            repo.path(),
        )
        .unwrap();
        fs::write(
            repo.path().join(".lf/skills/work.md"),
            "Changed after capture",
        )
        .unwrap();
        let plan = render_flow_instructions("chat", &steps).unwrap();
        assert!(plan.contains("entire range 1 through 4"));
        assert!(plan.contains("entire range 5[repair].1 through 5[repair].2"));
        assert!(plan.contains("rejoin after 5"));
        assert!(plan.contains("occurrence ID `first`"));
        assert!(plan.contains("occurrence ID `second`"));
        assert!(plan.contains("pause for their response"));
        assert!(plan.contains("Repair the findings"));
        assert!(plan.contains("\"advance\",\"iterate\",\"blocked\""));
        assert_eq!(plan.matches("Original work body.").count(), 1);
        assert!(!plan.contains("Changed after capture"));
        assert!(plan.contains("allowed-tools: Read"));
        assert!(plan.contains(
            &repo
                .path()
                .join(".claude/skills/native-check")
                .display()
                .to_string()
        ));
        let command = plan
            .split("```sh\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap();
        assert_eq!(
            shlex::split(command).unwrap(),
            vec!["lf", "commit", "-m", "don't-expand-$HOME;echo-unsafe"]
        );
        assert!(plan.contains("no arbitrary iteration limit"));
        assert!(plan.contains("does not advance a Task's Workflow"));
    }
}
