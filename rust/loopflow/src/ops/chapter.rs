//! Explicit Project rotation. Shared bindings select Projects; receipts retain recovery evidence.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::engine::git::{is_clean, rev_parse};
use crate::id::WaveId;
use crate::ops::{OpsError, OpsResult};
use crate::pm::{PmItem, PmProject, ProjectContent, ProjectStatus};
use crate::store::sqlite::project_selection::read_project_binding;
use crate::store::Store;
use crate::work::wave::{Wave, WaveLocator};

use super::pm::{lock_wave_planning, pm_store, project_is_foreign, PmContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskDisposition {
    Move,
    Historical,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChapterTask {
    pub task: PmItem,
    pub disposition: TaskDisposition,
    pub reason: String,
    pub observed_at: i64,
}

/// Retained command input. Names describe the plan; IDs select its destinations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChapterPlan {
    pub name: String,
    pub waves: Vec<WaveChapterPlan>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveChapterPlan {
    pub wave_id: WaveId,
    pub successor_id: String,
    pub create: bool,
    pub project_name: String,
    pub content: ProjectContent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChapterRotation {
    pub name: String,
    pub waves: Vec<WaveRotation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaveRotation {
    pub wave: String,
    pub successor_id: String,
    pub predecessor: Option<PmProject>,
    pub successor: Option<PmProject>,
    pub tasks: Vec<ChapterTask>,
}

fn error(value: impl std::fmt::Display) -> OpsError {
    OpsError::Message(value.to_string())
}

pub fn new_chapter(
    repo: &Path,
    name: &str,
    plan: &Path,
    wave: Option<&str>,
    dry_run: bool,
) -> OpsResult<ChapterRotation> {
    let plan: ChapterPlan =
        serde_json::from_slice(&std::fs::read(plan).map_err(error)?).map_err(error)?;
    if plan.name != name {
        return Err(error("chapter name must match the retained plan"));
    }
    let rotation = tokio::runtime::Runtime::new()
        .map_err(error)?
        .block_on(rotate(repo, &plan, wave, dry_run))?;
    if !dry_run
        && crate::engine::config::load_config_or_default(Some(repo))
            .pm
            .and_then(|pm| pm.linear_team)
            .is_some()
    {
        eprintln!("Saved locally; pending Linear sync.");
    }
    Ok(rotation)
}

pub(crate) async fn rotate(
    repo: &Path,
    plan: &ChapterPlan,
    only_wave: Option<&str>,
    dry_run: bool,
) -> OpsResult<ChapterRotation> {
    if plan.name.trim().is_empty()
        || plan.name.trim() != plan.name
        || plan.name.len() > 160
        || plan.name.contains(['\n', '\r'])
    {
        return Err(error("chapter name must be 1–160 characters on one line"));
    }
    let store = pm_store().await?;
    let selected_wave = match only_wave {
        Some(name) => Some(
            store
                .get_wave_at(&WaveLocator::discover(repo, name).map_err(error)?)
                .await
                .map_err(error)?
                .ok_or_else(|| error("Wave is not registered"))?
                .id()
                .clone(),
        ),
        None => None,
    };
    let mut ids = BTreeSet::new();
    let mut successors = BTreeSet::new();
    let mut inputs = Vec::new();
    for input in &plan.waves {
        if selected_wave
            .as_ref()
            .is_some_and(|id| id != &input.wave_id)
        {
            continue;
        }
        if !ids.insert(input.wave_id.as_str().to_owned())
            || !successors.insert(input.successor_id.clone())
        {
            return Err(error("chapter plan repeats a Wave or destination"));
        }
        if input.create {
            let successor = uuid::Uuid::parse_str(
                input
                    .successor_id
                    .strip_prefix("proj_")
                    .unwrap_or(&input.successor_id),
            )
            .map_err(error)?;
            if successor.get_version() != Some(uuid::Version::Random) {
                return Err(error(
                    "new Project destinations require a UUID v4 allocated with the plan",
                ));
            }
        }
        if input.content.krs.is_empty()
            || input.content.krs.iter().any(|kr| kr.text.trim().is_empty())
        {
            return Err(error(
                "each chapter destination requires nonempty authored KRs",
            ));
        }
        input.content.validate().map_err(error)?;
        if input.project_name.trim().is_empty() {
            return Err(error("Project name is empty"));
        }
        inputs.push(input);
    }
    if inputs.is_empty() {
        return Err(error("chapter plan has no selected Waves"));
    }
    inputs.sort_by(|a, b| a.wave_id.as_str().cmp(b.wave_id.as_str()));
    let mut contexts = Vec::new();
    for input in inputs {
        let wave = store
            .get_wave(&input.wave_id)
            .await
            .map_err(error)?
            .ok_or_else(|| error("planned Wave is unavailable"))?;
        let located = store
            .get_wave_at(&WaveLocator::discover(repo, wave.slug()).map_err(error)?)
            .await
            .map_err(error)?;
        if located.as_ref().map(Wave::id) != Some(wave.id()) {
            return Err(error("planned Wave belongs to another repository"));
        }
        super::pm::require_planning_home(&store, &wave).await?;
        super::metrics::validate_chapter_targets(&wave, &input.content.metric_targets)
            .map_err(error)?;
        let acquisition = lock_wave_planning(&wave).await?;
        contexts.push((input, wave, acquisition));
    }
    let mut roots = Vec::new();
    for (_, wave, _) in &contexts {
        roots.extend(
            store
                .list_tasks(Some(wave.id()))
                .await
                .map_err(error)?
                .into_iter()
                .filter_map(|task| task.worktree)
                .filter(|path| !path.as_os_str().is_empty()),
        );
    }
    let checkouts = store.lock_checkout_roots(roots).await.map_err(error)?;
    let mut prepared = Vec::new();
    let mut guards = Vec::new();
    for (input, wave, acquisition) in contexts {
        let acquisition = Arc::new(acquisition.with_checkouts(&checkouts));
        if !dry_run {
            super::project::import_binding(&store, &wave, &acquisition)?;
        }
        prepared.push(prepare_rotation(&store, input, &wave, &plan.name).await?);
        guards.push(acquisition);
    }
    if dry_run {
        return Ok(ChapterRotation {
            name: plan.name.clone(),
            waves: prepared.into_iter().map(|(_, result)| result).collect(),
        });
    }
    store
        .sqlite
        .rotate_projects(&plan.name, &prepared)
        .map_err(error)?;
    for (input, result) in &mut prepared {
        let id = crate::durable::ProjectId::parse(&input.successor_id).map_err(error)?;
        let mut successor = store.sqlite.planning_project(&id).map_err(error)?;
        successor.id = id.to_string();
        result.successor = Some(successor);
        if let Some(predecessor) = &mut result.predecessor {
            let id = crate::durable::ProjectId::parse(&predecessor.id).map_err(error)?;
            *predecessor = store.sqlite.planning_project(&id).map_err(error)?;
            predecessor.id = id.to_string();
        }
        for task in &mut result.tasks {
            let id = crate::durable::TaskId::parse(&task.task.id).map_err(error)?;
            task.task = super::task::task_planning_item(
                &store,
                &store
                    .sqlite
                    .task(&id)
                    .map_err(error)?
                    .ok_or_else(|| error("rotation Task disappeared"))?,
            )?;
        }
    }
    drop(guards);
    Ok(ChapterRotation {
        name: plan.name.clone(),
        waves: prepared.into_iter().map(|(_, result)| result).collect(),
    })
}

async fn prepare_rotation(
    store: &Store,
    input: &WaveChapterPlan,
    wave: &Wave,
    name: &str,
) -> OpsResult<(WaveChapterPlan, WaveRotation)> {
    let mut input = input.clone();
    if input.create {
        let uuid = uuid::Uuid::parse_str(
            input
                .successor_id
                .strip_prefix("proj_")
                .unwrap_or(&input.successor_id),
        )
        .map_err(error)?;
        input.successor_id = format!("proj_{}", uuid.simple());
    } else {
        let project = store
            .get_project_by_project(&input.successor_id)
            .await
            .map_err(error)?
            .ok_or_else(|| error("destination is unavailable"))?;
        if project.wave_id != *wave.id() {
            return Err(error("destination belongs to another Wave"));
        }
        input.successor_id = project.id.to_string();
    }
    let binding = match read_project_binding(&store.sqlite, wave.id()).map_err(error)? {
        Some(id) => Some(
            store
                .get_project_by_project(&id)
                .await
                .map_err(error)?
                .ok_or_else(|| error("selected predecessor is unavailable"))?
                .id
                .to_string(),
        ),
        None => None,
    };
    let receipt = store
        .project_transition(wave.id(), &input.successor_id)
        .await
        .map_err(error)?;
    if let Some(receipt) = &receipt {
        if receipt.reset_name.as_deref() != Some(name)
            || receipt.create_successor != Some(input.create)
            || receipt.settled_at.is_none()
            || binding.as_deref() != Some(&input.successor_id)
        {
            return Err(error(
                "retained rotation conflicts with this request or the selected Project",
            ));
        }
    }
    let predecessor_id = receipt
        .as_ref()
        .map(|r| r.predecessor_id.clone())
        .unwrap_or(binding);
    let projects = store.list_projects(Some(wave.id())).await.map_err(error)?;
    let project = |id: &str| -> OpsResult<Option<PmProject>> {
        let Some(saved) = projects.iter().find(|p| p.id.as_str() == id) else {
            return Ok(None);
        };
        let mut projection = store.sqlite.planning_project(&saved.id).map_err(error)?;
        projection.id = saved.id.to_string();
        Ok(Some(projection))
    };
    let predecessor = predecessor_id
        .as_deref()
        .map(project)
        .transpose()?
        .flatten();
    if predecessor_id.is_some() && predecessor.is_none() {
        return Err(error("selected predecessor is unavailable"));
    }
    let successor = project(&input.successor_id)?;
    if receipt.is_none()
        && (input.create == successor.is_some()
            || predecessor_id.as_deref() == Some(&input.successor_id))
    {
        return Err(error(
            "destination must be new for creation or an existing different Project for selection",
        ));
    }
    if successor.as_ref().is_some_and(|p| {
        !matches!(
            p.status,
            ProjectStatus::Backlog | ProjectStatus::Planned | ProjectStatus::Started
        )
    }) {
        return Err(error("terminal Project cannot be a rotation destination"));
    }
    let retained = store
        .project_transition_items(wave.id(), &input.successor_id)
        .await
        .map_err(error)?;
    let mut tasks = Vec::new();
    for task in store.list_tasks(Some(wave.id())).await.map_err(error)? {
        if task.project_id.as_str() != predecessor_id.as_deref().unwrap_or("")
            && !retained.iter().any(|id| id == task.id.as_str())
        {
            continue;
        }
        let mut item = super::task::task_planning_item(store, &task)?;
        item.id = task.id.to_string();
        let mut decision = disposition(store, item).await?;
        if task.worktree.is_none() && decision.disposition == TaskDisposition::Unresolved {
            decision.disposition = TaskDisposition::Historical;
            decision.reason = "unplaced backlog stays with its Project".into();
        }
        if retained.iter().any(|id| id == task.id.as_str()) {
            decision.disposition = TaskDisposition::Move;
            decision.reason = "retained rotation membership".into();
        }
        if decision.disposition == TaskDisposition::Unresolved {
            return Err(error(format!(
                "{}: {}",
                task.plan.identifier, decision.reason
            )));
        }
        tasks.push(decision);
    }
    let result = WaveRotation {
        wave: wave.slug().into(),
        successor_id: input.successor_id.clone(),
        predecessor,
        successor,
        tasks,
    };
    Ok((input, result))
}

// The migration marker scopes old-content decoding to the exact pre-upgrade
// records. New Planned Projects never enter this transition. Reads project the
// conversion; explicit sync/rotation confirms it remotely before clearing it.
pub(crate) async fn adopt_legacy_projects(
    repo: &Path,
    store: &Store,
    wave_name: &str,
    ctx: &PmContext,
    apply: bool,
) -> OpsResult<Vec<PmProject>> {
    let locator = WaveLocator::discover(repo, wave_name).map_err(error)?;
    let Some(wave) = store.get_wave_at(&locator).await.map_err(error)? else {
        return Ok(Vec::new());
    };
    let acquisition = if apply {
        Some(lock_wave_planning(&wave).await?)
    } else {
        None
    };
    let mut pending = store
        .projects_pending_adoption(wave.id())
        .await
        .map_err(error)?;
    if pending.is_empty() {
        return Ok(Vec::new());
    }
    let mut projects = ctx
        .client
        .list_projects(&ctx.initiative)
        .await
        .map_err(error)?;
    for (id, _) in &pending {
        if !projects.iter().any(|project| &project.id == id) {
            projects.push(ctx.client.project_ownership(id).await.map_err(error)?);
        }
    }
    // Retained migration records can belong to another repository's Team.
    // Leave those Projects and their receipts untouched, just as ordinary sync does.
    projects.retain(|project| !project_is_foreign(project, &ctx.team_id));
    pending.retain(|(id, _)| projects.iter().any(|project| &project.id == id));
    let has_current = projects
        .iter()
        .any(|project| project.status == ProjectStatus::Started);
    let candidates: Vec<_> = pending
        .iter()
        .filter(|(id, evidence)| {
            *evidence != 0
                && projects.iter().any(|project| {
                    &project.id == id
                        && matches!(
                            project.status,
                            ProjectStatus::Backlog | ProjectStatus::Planned
                        )
                })
        })
        .collect();
    let recorded: Vec<_> = candidates
        .iter()
        .filter(|(_, evidence)| *evidence == 1)
        .collect();
    let promote = if has_current {
        None
    } else if let [candidate] = recorded.as_slice() {
        Some(candidate.0.as_str())
    } else if recorded.is_empty()
        && candidates.len() == 1
        && projects
            .iter()
            .filter(|project| {
                matches!(
                    project.status,
                    ProjectStatus::Backlog | ProjectStatus::Planned
                )
            })
            .count()
            == 1
    {
        Some(candidates[0].0.as_str())
    } else if candidates.is_empty() {
        None
    } else {
        return Err(error(format!("Wave {wave_name}: legacy current Project is ambiguous; set the intended Project In Progress in Linear")));
    };
    // Validate the entire Wave before performing the first conversion.
    let mut converted = Vec::new();
    for (id, _) in &pending {
        converted.push(
            ctx.client
                .adopt_project(
                    id,
                    &ctx.initiative,
                    &ctx.team_id,
                    promote == Some(id.as_str()),
                    false,
                )
                .await
                .map_err(error)?,
        );
    }
    if apply {
        for project in &mut converted {
            let confirmed = ctx
                .client
                .adopt_project(
                    &project.id,
                    &ctx.initiative,
                    &ctx.team_id,
                    promote == Some(project.id.as_str()),
                    true,
                )
                .await
                .map_err(error)?;
            project.revision.clone_from(&confirmed.revision);
            if confirmed != *project {
                return Err(error(format!(
                    "Project {} changed during adoption; refresh and retry",
                    project.id
                )));
            }
            store
                .finish_project_adoption(
                    wave.id(),
                    &project.id,
                    acquisition
                        .as_ref()
                        .expect("apply owns the Wave lock")
                        .clone(),
                )
                .await
                .map_err(error)?;
        }
    }
    Ok(converted)
}

async fn disposition(store: &Store, item: PmItem) -> OpsResult<ChapterTask> {
    let mut evidence = TaskStartEvidence {
        begun: false,
        // This Machine's missing Task row says nothing about work on another Machine.
        authored: None,
        published: false,
        abandoned: false,
        completed: false,
    };
    if let Some(task) = store.get_task_by_issue(&item.id).await.map_err(error)? {
        evidence = store.chapter_task_evidence(&task.id).await.map_err(error)?;
        let prs = store.task_prs(&task.id).await.map_err(error)?;
        evidence.published = prs
            .iter()
            .any(|pr| pr.publication.is_some() || pr.merge_commit.is_some());
        if let (Some(pr), Some(worktree)) = (prs.last(), task.worktree.as_ref()) {
            evidence.authored = is_clean(worktree).ok().and_then(|clean| {
                rev_parse(worktree, "HEAD")
                    .ok()
                    .map(|head| !clean || head != pr.base_commit)
            });
        }
    }
    let (disposition, reason) = classify_task(&item, &evidence);
    Ok(ChapterTask {
        task: item,
        disposition,
        reason,
        observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
    })
}

/// Missing local evidence is distinct from evidence that no work began.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStartEvidence {
    pub begun: bool,
    pub authored: Option<bool>,
    pub published: bool,
    pub abandoned: bool,
    pub completed: bool,
}

pub fn classify_task(task: &PmItem, evidence: &TaskStartEvidence) -> (TaskDisposition, String) {
    let state = task.state.as_deref();
    let terminal = matches!(state, Some("completed" | "canceled" | "duplicate")) || task.completed;
    if evidence.abandoned
        && !terminal
        && (state == Some("started")
            || evidence.begun
            || evidence.published
            || evidence.authored != Some(false))
    {
        let reason = if evidence.authored.is_none() {
            "local retirement has unavailable checkout evidence"
        } else {
            "local retirement conflicts with refreshed start evidence"
        };
        return (TaskDisposition::Unresolved, reason.into());
    }
    if evidence.completed && !terminal {
        return (
            TaskDisposition::Unresolved,
            "local completion awaits provider settlement".into(),
        );
    }
    if evidence.abandoned && !terminal && !evidence.completed {
        return (
            TaskDisposition::Unresolved,
            "local abandonment awaits explicit provider settlement".into(),
        );
    }
    if terminal || evidence.completed {
        return (
            TaskDisposition::Historical,
            "work is already terminal".into(),
        );
    }
    if evidence.begun
        || evidence.published
        || evidence.authored == Some(true)
        || state == Some("started")
    {
        return (
            TaskDisposition::Move,
            "work has started; retain its identity and execution".into(),
        );
    }
    if evidence.authored == Some(false) && matches!(state, Some("backlog" | "unstarted" | "triage"))
    {
        return (
            TaskDisposition::Historical,
            "unreviewed backlog remains in its existing Project".into(),
        );
    }
    (
        TaskDisposition::Unresolved,
        "start evidence is unavailable; refresh before rotating this Task".into(),
    )
}

#[cfg(test)]
#[path = "chapter_tests.rs"]
mod tests;
