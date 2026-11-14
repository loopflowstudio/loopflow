//! The `checkout` namespace: the Git checkout of a repository on this
//! Machine. `checkout.commit` commits it, `checkout.create` makes a sibling
//! worktree, and `checkout.sync` brings its branch up to date with the one
//! it follows. A sync that may launch an agent to resolve conflicts runs
//! detached ([`super::launch`]); its plan, and a `continue` or `abort` of a
//! sync in progress, answer in place.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{launch, Error, ErrorCode, Result};
use crate::git::worktrees::PlacementStrategy;
use crate::id::LfProcessId;
use crate::ops::{Progress, SyncPlan};

/// What `checkout.commit` left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Committed {
    pub branch: Option<String>,
    pub head: String,
    /// A commit was made; false when nothing was staged.
    pub committed: bool,
    pub pushed: bool,
}

/// What `checkout.create` made, or would make with `preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Checkout {
    pub path: PathBuf,
    pub branch: String,
    /// The ref the branch was started from; null when the branch existed.
    pub base: Option<String>,
    /// How the placement was resolved; null for a persistent workspace.
    pub strategy: Option<PlacementStrategy>,
    /// The worktree exists now; false for a preview.
    pub created: bool,
}

/// What `checkout.sync` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Sync {
    /// `preview`: the strategy a sync would take; nothing moved.
    Planned { plan: SyncPlan },
    /// The sync runs detached as `process`, with its output in `log`.
    Running {
        #[serde(rename = "lf_process_id")]
        process: LfProcessId,
        run: String,
        log: PathBuf,
    },
    /// The sync, continue or abort finished in place.
    Finished { message: String },
}

impl Sync {
    /// What `lf sync` prints.
    pub fn render(&self) -> String {
        match self {
            Self::Planned { plan } => {
                let mut lines = vec![
                    format!("branch: {}", plan.branch),
                    format!("base: {}", plan.base_ref),
                ];
                if let Some(fork_base) = &plan.fork_base {
                    lines.push(format!("fork_base: {fork_base}"));
                }
                lines.push(format!(
                    "class: {}",
                    serde_json::to_value(&plan.class)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned))
                        .unwrap_or_default()
                ));
                lines.push(format!(
                    "strategy: {}",
                    serde_json::to_value(&plan.strategy)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned))
                        .unwrap_or_default()
                ));
                lines.push(format!("unique_commits: {}", plan.unique_commits));
                lines.push(format!("changed_files: {}", plan.changed_files.len()));
                lines.push(format!(
                    "protected: {}",
                    matches!(plan.class, crate::ops::SyncClass::Protected)
                ));
                lines.join("\n")
            }
            Self::Running { process, run, log } => format!(
                "{run} running as process {process}\n  follow: lf process show {process}\n  log:    {}",
                log.display()
            ),
            Self::Finished { message } => message.clone(),
        }
    }
}

/// `lf sync`'s arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncInput<'a> {
    /// The branch to sync onto; the default branch or the stack parent otherwise.
    pub onto: Option<&'a str>,
    /// `--plan`: the strategy only.
    pub preview: bool,
    /// `--manual`: keep the sync and edits local; conflicts are the caller's.
    pub manual: bool,
    /// `--continue`: stage resolved paths and finish the sync in progress.
    pub continue_sync: bool,
    /// `--abort`: abort the sync in progress.
    pub abort: bool,
    /// `--adopt`: claim a raw sync no Loopflow operation owns.
    pub adopt: bool,
}

impl SyncInput<'_> {
    /// The sync neither plans nor finishes one in progress, so it may merge
    /// and launch an agent.
    pub fn moves(&self) -> bool {
        !(self.preview || self.continue_sync || self.abort)
    }
}

/// `checkout.commit`: commit the checkout containing `directory`, as `lf
/// commit [-m <message>] [-p] [--no-add] [<path>…]` does. With `paths`, only
/// those, through an index of their own; `message` is required then. Without
/// a message the configured agent writes one.
pub fn commit(
    directory: &Path,
    message: Option<&str>,
    push: bool,
    no_add: bool,
    paths: &[String],
    agent: Option<&str>,
    progress: &impl Progress,
) -> Result<Committed> {
    let repo = repo_root(directory)?;
    let committed = if paths.is_empty() {
        crate::ops::commit_workflow(
            &repo,
            &crate::ops::CommitOptions {
                add: !no_add,
                push,
                message: message.map(str::to_owned),
                agent: agent.map(str::to_owned),
                ..crate::ops::CommitOptions::for_task("commit")
            },
            progress,
            &|_| {},
        )
        .map_err(outcome_unknown)?
    } else {
        crate::ops::commit_selected(&repo, paths, message).map_err(outcome_unknown)?;
        true
    };
    Ok(Committed {
        branch: crate::git::current_branch(&repo).map_err(outcome_unknown)?,
        head: crate::git::rev_parse(&repo, "HEAD").map_err(outcome_unknown)?,
        committed,
        pushed: push && paths.is_empty(),
    })
}

/// `checkout.create`: a worktree for `name` beside the repository containing
/// `directory`, following its layout, as `lf wt create <name>` does. With
/// `preview`, the placement only. With `persistent`, a workspace kept after
/// delivery.
pub fn create(
    directory: &Path,
    name: &str,
    preview: bool,
    persistent: bool,
    progress: &impl Progress,
) -> Result<Checkout> {
    use crate::git::worktrees::{
        create_from_placement_plan, ensure_agent_worktree, main_repo_root, plan_placement,
        WorktreeSegment,
    };
    let repo = repo_root(directory)?;
    let main_repo = main_repo_root(&repo).map_err(outcome_unknown)?;
    let segment = WorktreeSegment::parse(name)
        .map_err(|error| Error::new(ErrorCode::Invalid, error.to_string()))?;
    if persistent && !preview {
        let workspace = ensure_agent_worktree(&main_repo, segment).map_err(outcome_unknown)?;
        return Ok(Checkout {
            path: workspace.path,
            branch: workspace.branch,
            base: None,
            strategy: None,
            created: true,
        });
    }
    let mut placement = plan_placement(&main_repo, segment.clone()).map_err(outcome_unknown)?;
    if preview {
        return Ok(Checkout {
            path: placement.worktree_path,
            branch: placement.branch,
            base: Some(placement.base_ref),
            strategy: Some(placement.strategy),
            created: false,
        });
    }
    if placement.strategy != PlacementStrategy::UseExistingWorktree {
        crate::ops::checkout::refresh_main(&main_repo, progress).map_err(outcome_unknown)?;
        placement = plan_placement(&main_repo, segment).map_err(outcome_unknown)?;
    }
    let result = create_from_placement_plan(&main_repo, &placement).map_err(outcome_unknown)?;
    crate::ops::telemetry::record_ops_metric(
        &repo,
        serde_json::json!({
            "op": "wt.create",
            "branch": placement.branch,
            "base_ref": placement.base_ref,
            "strategy": placement.strategy,
            "exit_status": "ok",
        }),
    );
    Ok(Checkout {
        path: result.path,
        branch: result.branch,
        base: result.base_branch,
        strategy: Some(placement.strategy),
        created: true,
    })
}

/// `checkout.sync`: the plan, continue or abort answers in place; a sync that
/// moves the branch starts `lf sync [<onto>] [--manual] --json` detached and
/// returns the Process that carries it, since resolving a conflict may launch
/// an agent.
pub fn sync(directory: &Path, input: &SyncInput<'_>) -> Result<Sync> {
    if !input.moves() {
        return carry_sync(directory, input, &super::Diagnostics);
    }
    launch::start(directory, &spell_sync(input))
}

pub(super) fn spell_sync(input: &SyncInput<'_>) -> Vec<String> {
    let mut args = vec!["sync".to_owned()];
    args.extend(input.onto.map(str::to_owned));
    if input.manual {
        args.push("--manual".to_owned());
    }
    args.push("--json".to_owned());
    args
}

/// `lf sync` in this process. Started by [`sync`], a sync that moves the
/// branch reports its Process first and carries on in its own log.
pub fn carry_sync(
    directory: &Path,
    input: &SyncInput<'_>,
    progress: &impl Progress,
) -> Result<Sync> {
    let repo = repo_root(directory)?;
    crate::lf::commands::ops::run_sync_in(&repo, input, progress)
        .map_err(|error| Error::refused(&error, ErrorCode::OutcomeUnknown))
}

fn repo_root(directory: &Path) -> Result<PathBuf> {
    crate::repo::require_repo_root(directory, "this command")
        .map_err(|error| Error::refused(&error, ErrorCode::Invalid))
}

fn outcome_unknown(error: impl Into<anyhow::Error>) -> Error {
    Error::outcome_unknown(&error.into())
}

#[cfg(test)]
mod tests {
    use super::{Checkout, Committed, Sync};

    fn result<T: serde::de::DeserializeOwned + serde::Serialize>(raw: &str) -> T {
        let message: serde_json::Value = serde_json::from_str(raw).unwrap();
        let typed: T = serde_json::from_value(message["result"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&typed).unwrap(), message["result"]);
        typed
    }

    #[test]
    fn receipts_match_the_shared_fixtures_without_defaults() {
        let committed: Committed = result(include_str!(
            "../../tests/fixtures/dto/server/checkout_committed.json"
        ));
        assert!(committed.committed);
        let created: Checkout = result(include_str!(
            "../../tests/fixtures/dto/server/checkout_created.json"
        ));
        assert!(created.created);
        let planned: Sync = result(include_str!(
            "../../tests/fixtures/dto/server/checkout_sync_planned.json"
        ));
        assert!(planned.render().starts_with("branch: "));
        let running: Sync = result(include_str!(
            "../../tests/fixtures/dto/server/checkout_syncing.json"
        ));
        assert!(running.render().contains("running as process"));
        assert!(serde_json::from_str::<Sync>(r#"{"state":"finished"}"#).is_err());
        assert!(serde_json::from_str::<Committed>(r#"{"head":"abc"}"#).is_err());
    }
}
