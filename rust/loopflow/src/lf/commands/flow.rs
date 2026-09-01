use crate::engine::flow::load_xor_path_items;
use crate::engine::{
    expand_flow, human_occurrence_ids, xor_verdict_path, ConcreteStep, ConcreteXor,
    ExecutionContext, ExecutionSkill, Flow, FlowEngine, FlowOutcome, FlowProgress, SkillExecutor,
    SkillOutcome, TEMP_XOR_ROUTE_STEP_NAME,
};
use crate::journal::{self, LfEventFields, LfEventType, LfNode};
use crate::lf::output::Colors;
use crate::lf::Cli;
use crate::ops::{commit_workflow, CommitOptions, NullProgress};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

/// Run a flow: print pipeline header, then execute each skill sequentially.
pub fn run(flow: &Flow, message: Option<&str>, cli: &Cli, repo: &Path) -> Result<()> {
    let items = expand_flow(flow, repo)?;
    print_pipeline_header(&flow.name, &items, repo)?;
    execute(&flow.name, &items, None, message, cli, repo)
}

pub fn show(name: &str, repo: &Path) -> Result<()> {
    let flow = crate::engine::load_flow(name, repo)?;
    let items = expand_flow(&flow, repo)?;
    for line in render_pipeline_lines(&items, repo)? {
        println!("{line}");
    }
    Ok(())
}

pub fn validate(name: &str, repo: &Path) -> Result<()> {
    let flow = crate::engine::load_flow(name, repo)?;
    let mut human = human_occurrence_ids(&flow, repo)?;
    human.sort();
    if human.is_empty() {
        println!("{}: valid (no human nodes)", flow.name);
    } else {
        println!("{}: valid (human nodes: {})", flow.name, human.join(", "));
    }
    Ok(())
}

/// Run exactly one expanded top-level step. The resident owns the cursor and
/// invokes this hidden primitive once per body, so a body boundary maps to
/// a product step instead of an entire flow.
pub fn run_step(flow: &str, index: usize, message: &str, cli: &Cli, repo: &Path) -> Result<()> {
    let definition = crate::engine::load_flow(flow, repo)?;
    let items = expand_flow(&definition, repo)?;
    let item = items
        .get(index)
        .cloned()
        .ok_or_else(|| anyhow!("flow '{flow}' has no step at index {index}"))?;
    execute(
        &definition.name,
        std::slice::from_ref(&item),
        Some(index as u32),
        Some(message),
        cli,
        repo,
    )
}

/// Execute expanded steps on a fresh runtime, bracketed by flow journal events.
/// `index` names the single step when the caller is running one body's worth.
fn execute(
    flow_name: &str,
    items: &[ConcreteStep],
    index: Option<u32>,
    message: Option<&str>,
    cli: &Cli,
    repo: &Path,
) -> Result<()> {
    let fields = |extra: LfEventFields| LfEventFields {
        flow: Some(flow_name.to_string()),
        index,
        ..extra
    };
    journal::emit(
        repo,
        LfNode::Flow,
        LfEventType::Started,
        fields(LfEventFields::default()),
    );
    let _flow_env = EnvVarGuard::set("LOOPFLOW_FLOW_NAME", flow_name);
    let executor = CliFlowExecutor {
        cli,
        message,
        repo: repo.to_path_buf(),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build flow runtime")?;
    let result = runtime
        .block_on(FlowEngine::new(executor).run(items, 0))
        .map(|outcome| match outcome {
            FlowOutcome::Completed | FlowOutcome::Waiting => (),
        });
    match &result {
        Ok(()) => journal::emit(
            repo,
            LfNode::Flow,
            LfEventType::Completed,
            fields(LfEventFields::default()),
        ),
        Err(err) => journal::emit(
            repo,
            LfNode::Flow,
            LfEventType::Errored,
            fields(LfEventFields {
                error: Some(err.to_string()),
                ..LfEventFields::default()
            }),
        ),
    }
    result
}

fn print_pipeline_header(flow_name: &str, items: &[ConcreteStep], repo: &Path) -> Result<()> {
    let colors = Colors::new();
    let lines = render_pipeline_lines(items, repo)?;
    let pipeline = lines
        .into_iter()
        .map(|line| {
            format!(
                "  {dim}{line}{reset}",
                dim = colors.dim,
                reset = colors.reset
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    eprintln!(
        "\n{dim}\u{2500}\u{2500} flow {reset}{bold}{name}{reset}\n{pipeline}\n",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        name = flow_name,
        pipeline = pipeline,
    );
    Ok(())
}

fn render_pipeline_lines(items: &[ConcreteStep], repo: &Path) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for item in items {
        lines.extend(render_pipeline_item(item, repo)?);
    }
    Ok(lines)
}

fn render_pipeline_item(item: &ConcreteStep, repo: &Path) -> Result<Vec<String>> {
    match item {
        ConcreteStep::Skill(skill) if skill.policy.human => Ok(vec![format!(
            "{} [human:{}]",
            skill.skill.name,
            skill
                .policy
                .id
                .as_deref()
                .expect("validated human node has an id"),
        )]),
        ConcreteStep::Skill(skill) => Ok(vec![skill.skill.name.clone()]),
        ConcreteStep::Op(ops) => Ok(vec![format!("op: {}", ops.item.display_name())]),
        ConcreteStep::Xor(branch) => {
            render_branch_item("xor", branch, TEMP_XOR_ROUTE_STEP_NAME, repo)
        }
    }
}

fn render_branch_item(
    kind: &str,
    branch: &ConcreteXor,
    default_router: &str,
    repo: &Path,
) -> Result<Vec<String>> {
    render_branch_pipeline(
        kind,
        branch.router.as_deref().unwrap_or(default_router),
        &branch.paths,
        repo,
    )
}

fn render_branch_pipeline(
    kind: &str,
    router: &str,
    paths: &std::collections::HashMap<String, crate::engine::XorPath>,
    repo: &Path,
) -> Result<Vec<String>> {
    let mut lines = vec![format!("[{kind} via {router}]")];
    let mut keys: Vec<&String> = paths.keys().collect();
    keys.sort();

    for (index, key) in keys.into_iter().enumerate() {
        let path = paths
            .get(key)
            .expect("branch path key collected from map should exist");
        let nested_items = load_xor_path_items(path, repo)?;
        let nested = render_pipeline_lines(&nested_items, repo)?;
        let branch_prefix = tree_prefix(index, paths.len());
        if nested.is_empty() {
            lines.push(format!("{branch_prefix} {key}"));
            continue;
        }

        let nested_chain = nested.join(" → ");
        lines.push(format!("{branch_prefix} {key} → {nested_chain}"));
    }

    Ok(lines)
}

fn tree_prefix(index: usize, total: usize) -> &'static str {
    if index + 1 == total {
        "└─"
    } else {
        "├─"
    }
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            std::env::set_var(self.key, previous);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

struct CliFlowExecutor<'a> {
    cli: &'a Cli,
    message: Option<&'a str>,
    repo: PathBuf,
}

#[async_trait]
impl SkillExecutor for CliFlowExecutor<'_> {
    fn repo_root(&self) -> &Path {
        &self.repo
    }

    async fn run_skill(
        &self,
        skill: &ExecutionSkill,
        ctx: ExecutionContext,
    ) -> Result<SkillOutcome> {
        if skill.skill.policy.human
            && !crate::lf::commands::run::is_interactive_run(
                self.cli,
                Some(&skill.invoke_as),
                self.message,
            )
        {
            let node_id = skill
                .skill
                .policy
                .id
                .as_deref()
                .expect("validated human flow node has an id");
            anyhow::bail!(
                "human flow node {node_id} requires an attached User surface; run it through durable Task Work to park it as a human session"
            );
        }
        if let Some(progress) = ctx.progress {
            print_skill_progress(progress, &skill.display_name);
        } else {
            print_nested_skill_progress(&skill.display_name);
        }

        run_skill_with_journal(
            &self.repo,
            &skill.display_name,
            ctx.progress.map(|progress| progress.index),
            || {
                if let Some(prompt) = skill.temporary_content.as_deref() {
                    let _guard = write_temp_skill(&self.repo, &skill.invoke_as, prompt)?;
                    crate::lf::commands::run::run(
                        Some(skill.invoke_as.as_str()),
                        self.message,
                        self.cli,
                    )?;
                } else {
                    crate::lf::commands::run::run(
                        Some(skill.invoke_as.as_str()),
                        self.message,
                        self.cli,
                    )?;
                }
                commit_skill_work(&self.repo, &skill.display_name)?;
                if skill.skill.policy.human {
                    confirm_present_human_review(
                        skill
                            .skill
                            .policy
                            .id
                            .as_deref()
                            .expect("validated human node has an id"),
                        &mut std::io::stdin().lock(),
                        &mut std::io::stderr().lock(),
                    )?;
                }
                Ok(())
            },
        )?;
        Ok(SkillOutcome::Completed)
    }

    async fn run_op(&self, ops: &crate::engine::ConcreteOp, ctx: ExecutionContext) -> Result<()> {
        if let Some(progress) = ctx.progress {
            let colors = Colors::new();
            eprintln!(
                "{dim}[{current}/{total}]{reset} {bold}op:{reset} {cmd}",
                dim = colors.dim,
                reset = colors.reset,
                bold = colors.bold,
                current = progress.index + 1,
                total = progress.total,
                cmd = ops.item.display_name(),
            );
        } else {
            eprintln!("op: {}", ops.item.display_name());
        }
        crate::ops::execute_flow_ops(&self.repo, &ops.item, &NullProgress)?;
        Ok(())
    }

    async fn read_xor_verdict(&self, branch: &crate::engine::ConcreteXor) -> Result<String> {
        crate::engine::flow::read_xor_verdict(&xor_verdict_path(&self.repo), branch)
            .map_err(anyhow::Error::msg)
    }
}

fn confirm_present_human_review(
    node_id: &str,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<()> {
    write!(
        output,
        "Accept human flow node {node_id} against its exact current content? [y/N] "
    )?;
    output.flush()?;
    let mut response = String::new();
    if input.read_line(&mut response)? == 0 {
        anyhow::bail!("human flow node {node_id} exited without explicit User acceptance");
    }
    if matches!(
        response.trim().to_ascii_lowercase().as_str(),
        "y" | "yes" | "accept"
    ) {
        return Ok(());
    }
    anyhow::bail!("human flow node {node_id} was not accepted by the User")
}

fn print_skill_progress(progress: FlowProgress, skill_name: &str) {
    let colors = Colors::new();
    eprintln!(
        "{dim}[{current}/{total}]{reset} {bold}{name}{reset}",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        current = progress.index + 1,
        total = progress.total,
        name = skill_name,
    );
}

fn print_nested_skill_progress(skill_name: &str) {
    let colors = Colors::new();
    eprintln!(
        "{dim}[*]{reset} {bold}{name}{reset}",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        name = skill_name,
    );
}

fn run_skill_with_journal(
    repo: &Path,
    skill_name: &str,
    index: Option<usize>,
    run: impl FnOnce() -> Result<()>,
) -> Result<()> {
    journal::emit(
        repo,
        LfNode::Skill,
        LfEventType::Started,
        LfEventFields {
            skill: Some(skill_name.to_string()),
            index: index.map(|value| value as u32),
            ..LfEventFields::default()
        },
    );
    let result = run();
    match &result {
        Ok(_) => journal::emit(
            repo,
            LfNode::Skill,
            LfEventType::Completed,
            LfEventFields {
                skill: Some(skill_name.to_string()),
                index: index.map(|value| value as u32),
                ..LfEventFields::default()
            },
        ),
        Err(err) => journal::emit(
            repo,
            LfNode::Skill,
            LfEventType::Errored,
            LfEventFields {
                skill: Some(skill_name.to_string()),
                index: index.map(|value| value as u32),
                error: Some(err.to_string()),
                ..LfEventFields::default()
            },
        ),
    }
    result
}

/// Commit any uncommitted changes left by the previous skill.
pub(crate) fn commit_skill_work(repo: &Path, skill_name: &str) -> Result<bool> {
    let options = CommitOptions {
        add: true,
        message: Some(format!("lf commit: {skill_name}")),
        ..CommitOptions::for_task(skill_name)
    };
    commit_workflow(repo, &options, &NullProgress).map_err(Into::into)
}
fn write_temp_skill(repo: &Path, name: &str, prompt: &str) -> Result<TempSkillGuard> {
    let tmp_skill_dir = repo.join(".lf/skills");
    std::fs::create_dir_all(&tmp_skill_dir)?;
    let path = tmp_skill_dir.join(format!("{name}.md"));
    let original_content = std::fs::read_to_string(&path).ok();
    std::fs::write(&path, prompt)?;
    Ok(TempSkillGuard {
        path,
        original_content,
    })
}

struct TempSkillGuard {
    path: PathBuf,
    original_content: Option<String>,
}

impl Drop for TempSkillGuard {
    fn drop(&mut self) {
        let result = match &self.original_content {
            Some(content) => std::fs::write(&self.path, content),
            None => match std::fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(err),
            },
        };

        if let Err(err) = result {
            eprintln!(
                "failed to restore temporary skill {}: {}",
                self.path.display(),
                err
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        confirm_present_human_review, render_pipeline_lines, write_temp_skill, CliFlowExecutor,
    };
    use crate::engine::{
        ConcreteSkill, ConcreteStep, ExecutionContext, ExecutionSkill, Flow, Skill, SkillExecutor,
    };
    use crate::lf::Cli;
    use std::fs;
    use tempfile::tempdir;
    use tempfile::TempDir;

    #[test]
    fn write_xor_route_skill_removes_temp_file_when_none_existed() {
        let temp = TempDir::new().unwrap();
        let skill_path = temp.path().join(".lf/skills/xor-route.md");

        {
            let _guard =
                write_temp_skill(temp.path(), "xor-route", "temporary route prompt").unwrap();
            assert_eq!(
                fs::read_to_string(&skill_path).unwrap(),
                "temporary route prompt"
            );
        }

        assert!(
            !skill_path.exists(),
            "temporary xor-route skill should be removed after use"
        );
    }

    #[test]
    fn write_xor_route_skill_restores_existing_file() {
        let temp = TempDir::new().unwrap();
        let skills_dir = temp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        let skill_path = skills_dir.join("xor-route.md");
        fs::write(&skill_path, "existing route prompt").unwrap();

        {
            let _guard =
                write_temp_skill(temp.path(), "xor-route", "temporary route prompt").unwrap();
            assert_eq!(
                fs::read_to_string(&skill_path).unwrap(),
                "temporary route prompt"
            );
        }

        assert_eq!(
            fs::read_to_string(&skill_path).unwrap(),
            "existing route prompt"
        );
    }

    #[test]
    fn render_pipeline_lines_expands_xor_paths_on_separate_lines() {
        let temp = tempdir().unwrap();
        let flows_dir = temp.path().join(".lf/flows/tend");
        fs::create_dir_all(&flows_dir).unwrap();
        fs::write(
            flows_dir.join("tune.yaml"),
            "- tend/play-chord\n- tend/review-chord\n",
        )
        .unwrap();

        let flow = Flow {
            name: "tend".to_string(),
            items: vec![
                crate::engine::flow::Step::Skill(crate::engine::flow::SkillStep {
                    skill: crate::engine::flow::Skill::named("tend/scan-waves"),
                    policy: crate::engine::OccurrencePolicy::default(),
                }),
                crate::engine::flow::Step::Xor(crate::engine::flow::XorDef {
                    router: Some("tend/assess".to_string()),
                    paths: [
                        (
                            "tune".to_string(),
                            crate::engine::flow::XorPath {
                                flow: Some("tend/tune".to_string()),
                                skill: None,
                                steps: Vec::new(),
                                description: "Adjust the chord".to_string(),
                                direction: Vec::new(),
                            },
                        ),
                        (
                            "silence".to_string(),
                            crate::engine::flow::XorPath {
                                flow: None,
                                skill: None,
                                steps: Vec::new(),
                                description: "No-op".to_string(),
                                direction: Vec::new(),
                            },
                        ),
                    ]
                    .into_iter()
                    .collect(),
                }),
            ],
        };

        let items = crate::engine::expand_flow(&flow, temp.path()).unwrap();
        let lines = render_pipeline_lines(&items, temp.path()).unwrap();

        assert_eq!(
            lines,
            vec![
                "tend/scan-waves".to_string(),
                "[xor via tend/assess]".to_string(),
                "├─ silence".to_string(),
                "└─ tune → tend/play-chord → tend/review-chord".to_string(),
            ]
        );

        assert!(matches!(items[1], ConcreteStep::Xor(_)));
    }

    #[test]
    fn rendered_pipeline_lists_human_node_identity() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let flow = crate::engine::load_flow("task-design", &repo).unwrap();
        let items = crate::engine::expand_flow(&flow, &repo).unwrap();

        let lines = render_pipeline_lines(&items, &repo).unwrap();
        assert_eq!(
            lines,
            vec![
                "kickoff".to_string(),
                "review-design [human:review_kickoff]".to_string(),
            ]
        );
    }

    #[test]
    fn validation_lists_human_nodes_hidden_in_xor_paths() {
        let repo = tempdir().unwrap();
        let flows = repo.path().join(".lf/flows");
        fs::create_dir_all(&flows).unwrap();
        fs::write(
            flows.join("choice.yaml"),
            "- xor:\n    paths:\n      review:\n        description: Review it\n        steps:\n          - step:\n              id: revise_choice\n              name: implement\n          - step:\n              id: review_choice\n              name: review-design\n              human: true\n",
        )
        .unwrap();
        let flow = crate::engine::load_flow("choice", repo.path()).unwrap();
        let human = crate::engine::human_occurrence_ids(&flow, repo.path()).unwrap();
        assert_eq!(human, vec!["review_choice"]);
        let items = crate::engine::expand_flow(&flow, repo.path()).unwrap();
        assert!(render_pipeline_lines(&items, repo.path())
            .unwrap()
            .iter()
            .any(|line| line.contains("review-design [human:review_choice]")));
    }

    #[test]
    fn present_human_flow_requires_a_typed_acceptance_after_the_session() {
        let mut output = Vec::new();
        confirm_present_human_review(
            "review_design",
            &mut std::io::Cursor::new("accept\n"),
            &mut output,
        )
        .expect("typed acceptance settles the present-human node");
        assert!(String::from_utf8(output)
            .unwrap()
            .contains("exact current content"));

        let declined = confirm_present_human_review(
            "review_design",
            &mut std::io::Cursor::new("no\n"),
            &mut Vec::new(),
        )
        .expect_err("an explicit refusal cannot advance the node");
        assert!(declined.to_string().contains("was not accepted"));

        let closed = confirm_present_human_review(
            "review_design",
            &mut std::io::Cursor::new(Vec::<u8>::new()),
            &mut Vec::new(),
        )
        .expect_err("raw session exit cannot advance the node");
        assert!(closed
            .to_string()
            .contains("without explicit User acceptance"));
    }

    #[tokio::test]
    async fn detached_human_flow_refuses_to_invent_user_acceptance() {
        let repo = tempdir().unwrap();
        let cli = Cli {
            batch: true,
            ..Cli::default()
        };
        let executor = CliFlowExecutor {
            cli: &cli,
            message: None,
            repo: repo.path().to_path_buf(),
        };
        let skill = ExecutionSkill::regular(ConcreteSkill {
            skill: Skill::named("design"),
            policy: crate::engine::OccurrencePolicy {
                id: Some("review_design".to_string()),
                human: true,
            },
            flow_parents: vec!["design".to_string()],
        });

        let error = executor
            .run_skill(&skill, ExecutionContext { progress: None })
            .await
            .expect_err("a detached direct flow has no User acceptance context");

        assert!(error.to_string().contains("durable Task Work"));
        assert!(!repo.path().join("scratch").exists());
    }
}
