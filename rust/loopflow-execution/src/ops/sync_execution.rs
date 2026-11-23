use crate::agent::{run_agent, ProcessConfig};
use crate::config::{load_config_or_default, Config};
use crate::git::{current_branch, get_default_branch};
use crate::graph::load_skill;
use crate::ops::telemetry::record_ops_metric;
use crate::ops::{
    abort_sync_for_resolution, continue_sync_for_resolution, plan_sync, recover_sync,
    sync_class_name, sync_strategy_name, sync_with_plan, OpsError, Progress, SyncOptions,
};
use crate::prompt::process::{prepare_process_prompt, ContextSourceOverrides, ProcessPromptInput};
use crate::prompt::Surface;
use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

pub(crate) fn run_sync_in(
    repo_root: &Path,
    input: &crate::engine::checkout::SyncInput<'_>,
    progress: &impl Progress,
) -> Result<crate::engine::checkout::Sync> {
    use crate::engine::checkout::Sync;
    let crate::engine::checkout::SyncInput {
        mut onto,
        preview: plan_only,
        manual,
        continue_sync,
        abort,
        adopt,
    } = *input;
    if onto.is_some() && (continue_sync || abort) {
        return Err(anyhow!(
            "a sync target cannot be combined with --continue or --abort"
        ));
    }
    if adopt && !(continue_sync || abort) {
        return Err(anyhow!(
            "--adopt is only valid with `lf sync --continue` or `lf sync --abort`"
        ));
    }
    if continue_sync {
        continue_sync_for_resolution(repo_root, adopt)?;
        return Ok(Sync::Finished {
            message: "Sync complete; branch remains local.".into(),
        });
    }
    if abort {
        abort_sync_for_resolution(repo_root, adopt)?;
        return Ok(Sync::Finished {
            message: "Sync aborted.".into(),
        });
    }
    if !plan_only {
        crate::engine::launch::carry(
            "checkout.sync",
            format!("sync onto {}", onto.unwrap_or("the branch it follows")),
        )?;
    }
    let started = Instant::now();
    let default = get_default_branch(repo_root)?;
    let upstream = crate::git::integration_base(repo_root)?;
    let on_main = current_branch(repo_root)?.as_deref() == Some(&default);
    if !plan_only && (onto.is_none() || (on_main && onto == Some(upstream.as_str()))) {
        crate::ops::checkout::refresh_main(repo_root, progress)?;
        if on_main && onto.is_none_or(|target| target == upstream) {
            return Ok(Sync::Finished {
                message: "Main is current; unpublished commits and edits remain local.".into(),
            });
        }
    }
    // Another open Task as the target stacks this Task on it; the stacked sync
    // below then merges the parent the record now names.
    if let Some(target) =
        onto.filter(|target| !plan_only && !on_main && *target != default && *target != upstream)
    {
        if let Some(parent) = crate::ops::task::open_task_named(repo_root, target)? {
            progress.status(&crate::ops::task::stack_task(repo_root, &parent)?);
            onto = None;
        }
    }
    // An explicit default branch detaches the child using its recorded fork;
    // ordinary sync continues to follow the live parent.
    let base = if on_main {
        None
    } else {
        Some(crate::ops::checkout::checkout_base(repo_root, &default)?)
    };
    let stacked = base.as_ref().and_then(|base| base.stack());
    let detach =
        stacked.is_some() && onto.is_some_and(|target| target == default || target == upstream);
    if let (Some(_), Some(onto), false) = (stacked, onto, detach) {
        return Err(anyhow!(
            "{onto} names no other open Task; name one to restack this Task on it, the default branch to detach it, or omit the target to sync its parent"
        ));
    }
    let fork_base = stacked.map(|stacked| stacked.fork_base.clone());
    let target = match (&base, onto) {
        _ if detach => upstream.clone(),
        (_, Some(onto)) => onto.to_string(),
        (Some(base), None) => base.sync_ref(&default)?,
        (None, None) => upstream.clone(),
    };
    let target = target.as_str();
    if !plan_only {
        crate::git::fetch_tracking_ref(repo_root, target)?;
    }
    // Classify against the tree holding the caller's edits. The plan that
    // runs is the one the metric records.
    let mut plan = plan_sync(repo_root, Some(target), fork_base.clone())?;
    if plan_only {
        return Ok(Sync::Planned { plan });
    }
    // Checkpoint ordinary edits so their conflicts reach the merge resolver,
    // rather than surfacing during stash restoration after the resolver is gone.
    // The checkpoint moves HEAD, so plan again against it.
    if !manual
        && !on_main
        && matches!(plan.strategy, crate::ops::SyncStrategy::MergeTarget)
        && crate::git::intervention_state(repo_root)?.is_none()
        && checkpoint_uncommitted_work(
            repo_root,
            &format!("Checkpoint edits before syncing onto {}", plan.base_ref),
        )?
    {
        progress.status("Committed uncommitted edits before merging.");
        plan = plan_sync(repo_root, Some(target), fork_base.clone())?;
    }
    let options = SyncOptions {
        onto: plan.base_ref.clone(),
        push: !manual && !on_main,
        fork_base,
    };
    let (verification, agent_launched) = if manual {
        (sync_with_plan(repo_root, &options, &plan, progress)?, false)
    } else {
        let recovery_config = load_config_or_default(Some(repo_root));
        crate::ops::checkout::with_preserved_edits(repo_root, || {
            match sync_with_plan(repo_root, &options, &plan, progress) {
                Ok(verification) => Ok((verification, false)),
                Err(OpsError::SyncConflict {
                    onto,
                    detail,
                    recovery,
                }) => Ok((
                    resolve_sync_conflict(
                        repo_root,
                        &onto,
                        &detail,
                        recovery,
                        progress,
                        &recovery_config,
                    )
                    .map_err(|error| OpsError::Message(error.to_string()))?,
                    true,
                )),
                Err(err) => Err(err),
            }
        })?
    };
    if manual {
        return Ok(Sync::Finished {
            message: format!(
                "Synced onto {}; the sync and edits remain local.",
                plan.base_ref
            ),
        });
    }
    record_ops_metric(
        repo_root,
        serde_json::json!({
            "op": "sync",
            "branch": plan.branch,
            "base_ref": plan.base_ref,
            "class": sync_class_name(&plan.class),
            "strategy": sync_strategy_name(&plan.strategy),
            "unique_commits": verification.unique_commits,
            "changed_files": plan.changed_files.len(),
            "protected": matches!(plan.class, crate::ops::SyncClass::Protected),
            "scratch_stashed": plan.scratch_stashed,
            "agent_launched": agent_launched,
            "duration_ms": started.elapsed().as_millis(),
            "exit_status": "ok",
        }),
    );
    Ok(Sync::Finished {
        message: format!("Synced onto {}.", plan.base_ref),
    })
}

/// Checkpoint ordinary edits while retaining telemetry's staged and working contents.
/// Returns whether there was anything to commit.
pub fn checkpoint_uncommitted_work(repo: &Path, message: &str) -> Result<bool> {
    // A literal `.lf/tmp` prefix makes Git reject the pathspec when ignored.
    // Use the same rooted glob when staging and refreshing the real index.
    const EXCLUDE_TELEMETRY: &str = ":(exclude,top,glob)[.]lf/tmp/**";
    crate::git::git_stdout(repo, &["add", "-A", "--", ":/", EXCLUDE_TELEMETRY])?;
    // Start from the staged tree so explicitly staged ignored user files survive.
    // Excluding telemetry from `add` alone cannot exclude already-staged entries.
    let staged_tree = crate::git::git_stdout(repo, &["write-tree"])?;
    let directory = tempfile::tempdir()?;
    let index = directory.path().join("index");
    let inherit_index = |command: &mut Command| {
        command.env("GIT_INDEX_FILE", &index);
    };
    let run = |args: &[&str]| crate::git::git_stdout_inheriting(repo, args, &inherit_index);
    run(&["read-tree", staged_tree.trim()])?;
    run(&["reset", "-q", "HEAD", "--", ":(top,glob)[.]lf/tmp/**"])?;
    // Filenames can consist entirely of whitespace.
    if run(&["diff", "--cached", "--name-only"])?.is_empty() {
        return Ok(false);
    }
    crate::git::commit(repo, message, &inherit_index)?;
    // Refresh only committed paths; the merge's existing preservation wrapper
    // carries the remaining telemetry through the merge with its index intact.
    crate::git::git_stdout(
        repo,
        &["reset", "-q", "HEAD", "--", ":/", EXCLUDE_TELEMETRY],
    )
    .map_err(|error| anyhow!("checkpoint saved; index refresh failed: {error}"))?;
    Ok(true)
}

/// Run one conflict resolution under the owning operation's scoped IDs.
/// The agent continues the existing sequencer. The caller keeps ownership
/// and performs verification and the single push.
fn resolve_sync_conflict(
    repo_root: &Path,
    onto: &str,
    detail: &str,
    recovery: Option<Box<crate::ops::SyncRecovery>>,
    progress: &impl Progress,
    config: &Config,
) -> Result<crate::ops::SyncVerification> {
    let recovery =
        recovery.ok_or_else(|| anyhow!("sync conflict has no owned recovery operation"))?;
    let context = format!(
        "<lf:sync-conflict>\nSync onto: {onto}\n{detail}\nContinue the existing owned sequencer; do not start another sync or push.\n</lf:sync-conflict>"
    );
    progress.status("Launching sync agent to resolve conflicts...");
    Ok(recover_sync(*recovery, || {
        process_skill_agent(repo_root, "sync-conflicts", Some(&context), config)
            .map_err(|error| OpsError::Message(error.to_string()))
    })?)
}

/// Run a PR-mutating op; on a sync conflict, launch the sync agent to
/// resolve it and retry once. A second conflict is a real error.
pub(crate) fn with_sync_retry<T>(
    repo_root: &Path,
    label: &str,
    progress: &impl Progress,
    op: impl Fn(&Path, bool) -> Result<T, OpsError>,
) -> Result<T> {
    match op(repo_root, false) {
        Ok(value) => Ok(value),
        Err(OpsError::SyncConflict {
            onto,
            detail,
            recovery,
        }) => {
            resolve_sync_conflict(
                repo_root,
                &onto,
                &detail,
                recovery,
                progress,
                &load_config_or_default(Some(repo_root)),
            )?;
            progress.status(&format!("Retrying {label} after sync..."));
            op(repo_root, true).map_err(Into::into)
        }
        Err(err) => Err(err.into()),
    }
}

/// Launch an agent with a named skill when an ops command needs judgment.
///
/// Used when mechanical operations hit a situation that requires agent
/// reasoning — e.g., sync conflicts that need conflict resolution.
fn process_skill_agent(
    repo_root: &Path,
    skill_name: &str,
    context: Option<&str>,
    config: &Config,
) -> Result<()> {
    let skill = load_skill(skill_name, repo_root)?;

    let message = context.map(|value| value.to_string());
    let prepared = prepare_process_prompt(
        config,
        ProcessPromptInput {
            repo_root: repo_root.to_path_buf(),
            skill: Some(skill_name.to_string()),
            resolved_skill: Some(skill),
            surface: Surface::Headless,
            message,
            source_overrides: ContextSourceOverrides {
                // Sync conflicts already name the affected paths in `context`.
                // Embedding every authored file here makes the task prompt grow
                // with the branch and can exceed the OS argument limit before
                // the resolver starts. The agent has the repository as its cwd
                // and can inspect the conflict in place.
                diff_files: Some(false),
                diff: Some(false),
                ..Default::default()
            },
            ..ProcessPromptInput::default()
        },
    )?;

    let agent = &prepared.config.agent;
    let provider = agent.harness.as_str().to_owned();
    let model = agent.model.clone();
    let cwd = prepared
        .config
        .cwd
        .clone()
        .unwrap_or_else(|| repo_root.to_path_buf());
    let context = crate::trace::PreparedTurnContext::from_prompts(
        &crate::agent::system_prompt_with_structured_replies(&prepared.config),
        &prepared.config.task_prompt,
    );
    let capture = crate::session_record::SessionRecorder::begin_with_context(
        crate::session_record::SessionCaptureSpec {
            harness: provider,
            model,
            surface: "headless".to_string(),
            cwd,
            repo: Some(repo_root.to_path_buf()),
            worktree: Some(repo_root.to_path_buf()),
            skill: Some(skill_name.to_string()),
            subjects: Vec::new(),
            graph: crate::session_record::SessionGraphMembership::Independent,
            work: None,
        },
        &context,
        Some(crate::session_record::AgentProcessRequest::from_prepared(
            &prepared.config,
        )?),
    )
    .map(crate::process::LfProcessRuntime::new)?;

    let launch = prepared.config;
    let process = ProcessConfig {
        stream: true,
        runtime: Some(capture.clone()),
        ..Default::default()
    };

    let result = run_agent(&launch, &process, crate::harness::LaunchKind::Batch);
    let outcome = match &result {
        Ok(result) if result.exit_code == 0 => "completed",
        Ok(_) | Err(_) => "failed",
    };
    capture.finish(outcome)?;
    let result = result?;
    if result.exit_code != 0 {
        let reason = result.stderr.trim();
        let reason = if reason.is_empty() {
            String::new()
        } else {
            format!("\n{reason}")
        };
        return Err(anyhow!(
            "agent exited with code {} while resolving {skill_name}{reason}",
            result.exit_code
        ));
    }
    Ok(())
}
