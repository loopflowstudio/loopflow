//! Repository chapter rotation converges on fresh Linear Project status.
//! No local record owns the chapter or remembers a partially applied operation.

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::durable::WorkRef;
use crate::engine::git::{is_clean, rev_parse};
use crate::ops::{OpsError, OpsResult};
use crate::pm::{PmItem, PmProject, ProjectContent, ProjectStatus};
use crate::store::Store;
use crate::work::project::{Project, ProjectId};
use crate::work::wave::{Wave, WaveLocator};

use super::pm::{
    checked_projects, linear_project_name, pm_store, refresh_pm_snapshot, resolve_context,
    PmContext,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskDisposition {
    Move,
    Abandon,
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

/// A preview/result, never a persisted Chapter or recovery receipt.
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

pub fn new_chapter(repo: &Path, name: &str, dry_run: bool) -> OpsResult<ChapterRotation> {
    tokio::runtime::Runtime::new()
        .map_err(error)?
        .block_on(rotate(repo, name, dry_run))
}

pub fn update_plan(repo: &Path, wave: Option<&str>, content: &ProjectContent) -> OpsResult<()> {
    content.validate().map_err(error)?;
    if content.flow.trim().is_empty() {
        return Err(error("Project content requires a nonempty flow: line"));
    }
    let wave =
        crate::work::wave::context::resolve_managed_wave_sync(Some(repo), wave).map_err(error)?;
    tokio::runtime::Runtime::new()
        .map_err(error)?
        .block_on(async {
            let _lock = rotation_lock(&wave).await?;
            super::metrics::validate_chapter_targets(&wave, &content.metric_targets)
                .map_err(error)?;
            let ctx = resolve_context(repo, wave.name()).await?;
            let projects = checked_projects(repo, &ctx, wave.name()).await?;
            let project = select_current(wave.name(), &projects)?;
            let provider = ctx
                .client
                .project_ownership(&project.id)
                .await
                .map_err(error)?;
            ctx.client
                .update_project(&provider.id, &provider.name, content)
                .await
                .map_err(error)?;
            refresh_pm_snapshot(repo, wave.name(), &ctx).await?;
            Ok(())
        })
}

pub(crate) fn select_current(wave: &str, projects: &[PmProject]) -> OpsResult<PmProject> {
    let current: Vec<_> = projects
        .iter()
        .filter(|project| project.status == ProjectStatus::Started)
        .collect();
    match current.as_slice() {
        [project] => Ok((*project).clone()),
        [] => Err(error(format!("Wave {wave} has no In Progress Project"))),
        _ => Err(error(format!(
            "Wave {wave} has competing In Progress Projects: {}",
            current
                .iter()
                .map(|project| format!("{} ({})", project.name, project.id))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

pub(crate) async fn current_project(store: &Store, wave: &Wave) -> OpsResult<PmProject> {
    let snapshot = store
        .pm_snapshot(wave.id())
        .await
        .map_err(error)?
        .ok_or_else(|| error("Project planning is unavailable; run `lf repo refresh <wave>`"))?;
    let snapshot = snapshot.snapshot;
    select_current(wave.name(), &snapshot.projects)
}

// Stable across Homes, including a lost create response before initiative attachment.
fn successor_id(initiative: &str, name: &str) -> String {
    let digest = Sha256::digest(format!("loopflow-project\0{initiative}\0{name}"));
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

fn plan_rotation(
    name: &str,
    inventories: &[(String, String, Vec<PmProject>)],
) -> OpsResult<ChapterRotation> {
    if name.trim().is_empty()
        || name.trim() != name
        || name.len() > 160
        || name.contains(['\n', '\r'])
    {
        return Err(error("chapter name must be 1–160 characters on one line"));
    }
    let predecessor_names: BTreeSet<_> = inventories
        .iter()
        .flat_map(|(_, _, projects)| projects)
        .filter(|project| project.status == ProjectStatus::Started && project.name != name)
        .map(|project| project.name.as_str())
        .collect();
    if predecessor_names.len() > 1 {
        return Err(error(format!(
            "competing current chapter names: {}; resolve the Projects in Linear",
            predecessor_names.into_iter().collect::<Vec<_>>().join(", ")
        )));
    }
    let predecessor_name = predecessor_names.first().copied();
    let mut waves = Vec::new();
    for (wave, initiative, projects) in inventories {
        let targets: Vec<_> = projects
            .iter()
            .filter(|project| project.name == name)
            .collect();
        let successor = match targets.as_slice() {
            [] => None,
            [project]
                if matches!(
                    project.status,
                    ProjectStatus::Planned | ProjectStatus::Started
                ) =>
            {
                Some((*project).clone())
            }
            [_] => {
                return Err(error(format!(
                    "Wave {wave}: target {name} exists but is neither Planned nor In Progress"
                )))
            }
            _ => {
                return Err(error(format!(
                    "Wave {wave}: several Projects are named {name}; choose one in Linear"
                )))
            }
        };
        let current: Vec<_> = projects
            .iter()
            .filter(|project| project.status == ProjectStatus::Started && project.name != name)
            .collect();
        let predecessor = match current.as_slice() {
            [project] => Some((*project).clone()),
            [] if successor
                .as_ref()
                .is_some_and(|project| project.status == ProjectStatus::Started) =>
            {
                None
            }
            [] if projects.is_empty() || (projects.len() == 1 && successor.is_some()) => None,
            [] => {
                // Recover the inverse status-write order only with shared predecessor evidence.
                let previous: Vec<_> = projects
                    .iter()
                    .filter(|project| {
                        Some(project.name.as_str()) == predecessor_name
                            && project.status == ProjectStatus::Completed
                    })
                    .collect();
                match previous.as_slice() {
                    [project] if successor.is_some() => Some((*project).clone()),
                    _ => return Err(error(format!("Wave {wave} has no unambiguous current predecessor; set its Project status in Linear"))),
                }
            }
            _ => {
                return Err(error(format!(
                    "Wave {wave} has competing In Progress predecessors"
                )))
            }
        };
        let id = successor
            .as_ref()
            .map(|project| project.id.clone())
            .unwrap_or_else(|| successor_id(initiative, name));
        waves.push(WaveRotation {
            wave: wave.clone(),
            successor_id: id,
            predecessor,
            successor,
            tasks: Vec::new(),
        });
    }
    Ok(ChapterRotation {
        name: name.into(),
        waves,
    })
}

pub(crate) async fn rotate(repo: &Path, name: &str, dry_run: bool) -> OpsResult<ChapterRotation> {
    let store = pm_store().await?;
    let names = super::pm::list_local_waves(repo)?;
    if names.is_empty() {
        return Err(error("repository has no Waves"));
    }
    let mut contexts = Vec::new();
    let mut inventories = Vec::new();
    let mut locks = Vec::new();
    for wave_name in names {
        let wave = if dry_run {
            let locator = WaveLocator::discover(repo, &wave_name).map_err(error)?;
            store
                .get_wave_at(&locator)
                .await
                .map_err(error)?
                .unwrap_or_else(|| {
                    Wave::new(
                        crate::id::WaveId::new(),
                        wave_name.clone(),
                        repo.display().to_string(),
                    )
                })
        } else {
            crate::work::wave::ensure_wave_row(&store, repo, &wave_name)
                .await
                .map_err(error)?
        };
        if !dry_run {
            locks.push(rotation_lock(&wave).await?);
        }
        let ctx = resolve_context(repo, &wave_name).await?;
        let projects = checked_projects(repo, &ctx, &wave_name).await?;
        inventories.push((wave_name, ctx.initiative.clone(), projects));
        contexts.push((wave, ctx));
    }
    let mut plan = plan_rotation(name, &inventories)?;
    // Evaluate every Wave before the first provider write, even if an earlier Wave is ready.
    for (entry, (wave, ctx)) in plan.waves.iter_mut().zip(&contexts) {
        entry.tasks = rotation_tasks(&store, wave, ctx, entry).await?;
    }
    if dry_run {
        return Ok(plan);
    }
    for entry in &plan.waves {
        let flow = entry
            .successor
            .as_ref()
            .or(entry.predecessor.as_ref())
            .map(|project| project.flow.as_str())
            .unwrap_or("feature");
        if flow.trim().is_empty() {
            return Err(error(format!(
                "Wave {}: set the Project's flow: line before rotating; no Project status changed",
                entry.wave
            )));
        }
        if let Some(task) = entry
            .tasks
            .iter()
            .find(|task| task.disposition == TaskDisposition::Unresolved)
        {
            return Err(error(format!(
                "{}: {}; no Project status changed",
                task.task.identifier, task.reason
            )));
        }
    }
    for (entry, (wave, ctx)) in plan.waves.iter_mut().zip(&contexts) {
        adopt_legacy_projects(repo, &store, wave.name(), ctx, true).await?;
        apply_rotation(repo, &store, wave, ctx, name, entry)
            .await
            .map_err(|cause| error(format!("{cause}; retry `lf repo new-chapter {name}`")))?;
        refresh_pm_snapshot(repo, wave.name(), ctx).await?;
    }
    for (wave, ctx) in &contexts {
        let projects = checked_projects(repo, ctx, wave.name()).await?;
        let current = select_current(wave.name(), &projects)?;
        if current.name != name {
            return Err(error(format!(
                "Wave {} now selects {}; reconcile competing chapter changes in Linear",
                wave.name(),
                current.name
            )));
        }
    }
    Ok(plan)
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
    let pending = store
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
                .finish_project_adoption(wave.id(), &project.id)
                .await
                .map_err(error)?;
        }
    }
    Ok(converted)
}

async fn rotation_tasks(
    store: &Store,
    wave: &Wave,
    ctx: &PmContext,
    entry: &WaveRotation,
) -> OpsResult<Vec<ChapterTask>> {
    let Some(predecessor) = &entry.predecessor else {
        return Ok(Vec::new());
    };
    let mut items = ctx
        .client
        .list_items(&predecessor.id)
        .await
        .map_err(error)?;
    let projects = store.list_projects(Some(wave.id())).await.map_err(error)?;
    for task in store.list_tasks(Some(wave.id())).await.map_err(error)? {
        if !projects.iter().any(|project| {
            project.id == task.project_id && project.plan.id.as_str() == predecessor.id
        }) || items.iter().any(|item| item.id == task.plan.id.as_str())
        {
            continue;
        }
        let (item, _) = ctx
            .client
            .issue_ownership(task.plan.id.as_str())
            .await
            .map_err(error)?
            .ok_or_else(|| error("Task planning is unavailable during chapter rotation"))?;
        if item.project_id.as_deref() != Some(predecessor.id.as_str())
            && item.project_id.as_deref() != Some(entry.successor_id.as_str())
        {
            return Err(error(format!(
                "{} moved outside this chapter transition",
                item.identifier
            )));
        }
        items.push(item);
    }
    let mut tasks = Vec::new();
    for item in items {
        tasks.push(disposition(store, item).await?);
    }
    Ok(tasks)
}

/// Adopt provider Project facts and same-Wave issue moves without creating Task execution.
pub(crate) async fn sync_projects(
    store: &Store,
    wave: &Wave,
    snapshot: &crate::pm::PmSnapshot,
) -> OpsResult<()> {
    for plan in &snapshot.projects {
        let project = record_project(store, wave, plan).await?;
        for item in snapshot
            .items
            .iter()
            .filter(|item| item.project_id.as_deref() == Some(plan.id.as_str()))
        {
            if let Some(task) = store.get_task_by_issue(&item.id).await.map_err(error)? {
                if task.project_id != project.id {
                    store
                        .move_chapter_task(&task.id, &project.id)
                        .await
                        .map_err(error)?;
                }
            }
        }
    }
    Ok(())
}

async fn record_project(store: &Store, wave: &Wave, plan: &PmProject) -> OpsResult<Project> {
    if let Some(mut project) = store
        .get_project_by_project(&plan.id)
        .await
        .map_err(error)?
    {
        project.plan =
            super::project::project_plan(plan, time::OffsetDateTime::now_utc().unix_timestamp())?;
        store.update_project(&project).await.map_err(error)?;
        return Ok(project);
    }
    let now = time::OffsetDateTime::now_utc();
    let project = Project {
        id: ProjectId::new(),
        plan: super::project::project_plan(plan, now.unix_timestamp())?,
        wave_id: wave.id().clone(),
        iteration: 0,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    store.create_project(&project).await.map_err(error)?;
    Ok(project)
}

async fn apply_rotation(
    repo: &Path,
    store: &Store,
    wave: &Wave,
    ctx: &PmContext,
    name: &str,
    entry: &mut WaveRotation,
) -> OpsResult<()> {
    let linear_name = linear_project_name(repo, wave.name(), name).await?;
    let mut successor = match ctx
        .client
        .find_project(&entry.successor_id)
        .await
        .map_err(error)?
    {
        Some(project) => project,
        None if entry.successor.is_some() => {
            return Err(error(
                "the selected Planned Project disappeared; refresh its ownership",
            ))
        }
        None => {
            let flow = entry
                .predecessor
                .as_ref()
                .map(|project| project.flow.as_str())
                .unwrap_or("feature");
            if flow.trim().is_empty() {
                return Err(error(
                    "predecessor has no flow: line; set its default Flow before rotating",
                ));
            }
            let content = ProjectContent {
                flow: flow.to_string(),
                metric_targets: Vec::new(),
                krs: Vec::new(),
            };
            ctx.client
                .create_project(
                    &ctx.initiative,
                    &linear_name,
                    &content,
                    Some(&entry.successor_id),
                )
                .await
                .map_err(error)?;
            ctx.client
                .project_ownership(&entry.successor_id)
                .await
                .map_err(error)?
        }
    };
    if (successor.name != linear_name && successor.name != name)
        || successor.team_ids != vec![ctx.team_id.clone()]
        || successor
            .initiative_ids
            .iter()
            .any(|id| id != &ctx.initiative)
    {
        return Err(error(
            "successor identity or ownership changed; reconcile it in Linear",
        ));
    }
    if !successor.initiative_ids.contains(&ctx.initiative) {
        ctx.client
            .attach_project(&ctx.initiative, &successor.id)
            .await
            .map_err(error)?;
        successor = ctx
            .client
            .project_ownership(&successor.id)
            .await
            .map_err(error)?;
        if successor.initiative_ids != vec![ctx.initiative.clone()] {
            return Err(error("successor attachment is not confirmed"));
        }
    }
    if !matches!(
        successor.status,
        ProjectStatus::Planned | ProjectStatus::Started
    ) {
        return Err(error("successor is no longer Planned or In Progress"));
    }
    if successor.status != ProjectStatus::Started {
        ctx.client
            .set_project_status(&successor.id, ProjectStatus::Started)
            .await
            .map_err(error)?;
        successor = ctx
            .client
            .project_ownership(&successor.id)
            .await
            .map_err(error)?;
        if successor.status != ProjectStatus::Started {
            return Err(error("successor activation is not confirmed"));
        }
    }
    successor.name = name.to_string();
    successor.slug = crate::pm::project_slug(name);
    let local = record_project(store, wave, &successor).await?;
    // Reconcile a transfer that another Home or an interrupted caller already performed.
    for item in ctx.client.list_items(&successor.id).await.map_err(error)? {
        if let Some(task) = store.get_task_by_issue(&item.id).await.map_err(error)? {
            store
                .move_chapter_task(&task.id, &local.id)
                .await
                .map_err(error)?;
        }
    }
    entry.successor = Some(successor);
    let Some(predecessor) = entry.predecessor.as_ref() else {
        return Ok(());
    };
    let fresh = ctx
        .client
        .project_ownership(&predecessor.id)
        .await
        .map_err(error)?;
    if !matches!(
        fresh.status,
        ProjectStatus::Started | ProjectStatus::Completed
    ) || fresh.team_ids != vec![ctx.team_id.clone()]
        || fresh.initiative_ids != vec![ctx.initiative.clone()]
    {
        return Err(error(
            "predecessor status or ownership changed; reconcile it in Linear",
        ));
    }
    entry.tasks = rotation_tasks(store, wave, ctx, entry).await?;
    for decision in &entry.tasks {
        let (item, _) = ctx
            .client
            .issue_ownership(&decision.task.id)
            .await
            .map_err(error)?
            .ok_or_else(|| error("Task planning is unavailable during chapter rotation"))?;
        let task = store.get_task_by_issue(&item.id).await.map_err(error)?;
        if item.project_id.as_deref() == Some(entry.successor_id.as_str()) {
            if let Some(task) = task {
                store
                    .move_chapter_task(&task.id, &local.id)
                    .await
                    .map_err(error)?;
            }
            continue;
        }
        if item.project_id.as_deref() != Some(predecessor.id.as_str()) {
            return Err(error(format!(
                "{} moved outside this transition",
                item.identifier
            )));
        }
        let mut decision = disposition(store, item).await?;
        if decision.disposition == TaskDisposition::Abandon {
            if let Some(task) = &task {
                if !store
                    .retire_chapter_backlog(&task.id)
                    .await
                    .map_err(error)?
                {
                    decision = disposition(store, decision.task).await?;
                }
            }
        }
        match decision.disposition {
            TaskDisposition::Move => {
                ctx.client
                    .move_item_to_project(&decision.task.id, &entry.successor_id)
                    .await
                    .map_err(error)?;
                let (confirmed, _) = ctx
                    .client
                    .issue_ownership(&decision.task.id)
                    .await
                    .map_err(error)?
                    .ok_or_else(|| error("Task planning is unavailable during chapter rotation"))?;
                if confirmed.project_id.as_deref() != Some(entry.successor_id.as_str()) {
                    return Err(error("Task transfer is not confirmed"));
                }
                if let Some(task) = task {
                    store
                        .move_chapter_task(&task.id, &local.id)
                        .await
                        .map_err(error)?;
                }
            }
            TaskDisposition::Abandon => {
                ctx.client
                    .cancel_item(&decision.task.id)
                    .await
                    .map_err(error)?;
                let (confirmed, _) = ctx
                    .client
                    .issue_ownership(&decision.task.id)
                    .await
                    .map_err(error)?
                    .ok_or_else(|| error("Task planning is unavailable during chapter rotation"))?;
                if confirmed.project_id.as_deref() != Some(predecessor.id.as_str())
                    || confirmed.state.as_deref() != Some("canceled")
                {
                    return Err(error("Task cancellation is not confirmed"));
                }
            }
            TaskDisposition::Historical => {}
            TaskDisposition::Unresolved => {
                return Err(error(format!(
                    "{}: {}",
                    decision.task.identifier, decision.reason
                )))
            }
        }
    }
    let remaining = rotation_tasks(store, wave, ctx, entry).await?;
    if remaining.iter().any(|task| {
        task.task.project_id.as_deref() == Some(predecessor.id.as_str())
            && task.disposition != TaskDisposition::Historical
    }) {
        return Err(error(
            "predecessor still has unfinished Tasks; refresh and retry",
        ));
    }
    let current = checked_projects(repo, ctx, wave.name()).await?;
    if current.iter().any(|project| {
        project.status == ProjectStatus::Started
            && project.id != predecessor.id
            && project.id != entry.successor_id
    }) {
        return Err(error(
            "another In Progress Project appeared during rotation; reconcile it in Linear",
        ));
    }
    let current_successor = current
        .iter()
        .find(|project| project.id == entry.successor_id)
        .ok_or_else(|| error("successor is missing from the final Project inventory"))?;
    if current_successor.status != ProjectStatus::Started || current_successor.name != name {
        return Err(error(
            "successor is no longer the intended In Progress Project; reconcile it in Linear",
        ));
    }
    let current_predecessor = current
        .iter()
        .find(|project| project.id == predecessor.id)
        .ok_or_else(|| error("predecessor is missing from the final Project inventory"))?;
    match current_predecessor.status {
        ProjectStatus::Started => {
            ctx.client
                .set_project_status(&predecessor.id, ProjectStatus::Completed)
                .await
                .map_err(error)?;
        }
        ProjectStatus::Completed => {}
        _ => {
            return Err(error(
                "predecessor status changed during rotation; reconcile it in Linear",
            ));
        }
    }
    let mut completed = ctx
        .client
        .project_ownership(&predecessor.id)
        .await
        .map_err(error)?;
    if completed.status != ProjectStatus::Completed {
        return Err(error("predecessor completion is not confirmed"));
    }
    completed.name = predecessor.name.clone();
    completed.slug = predecessor.slug.clone();
    record_project(store, wave, &completed).await?;
    Ok(())
}

pub(crate) async fn require_chapter_home(store: &Store, wave: &Wave) -> OpsResult<()> {
    let placement = store
        .placement(&WorkRef::Wave(wave.id().clone()))
        .await
        .map_err(error)?;
    let local = store.local_home().await.map_err(error)?;
    if placement.home_id != local.id {
        return Err(error(format!(
            "Wave {} is placed on {}; run this command with `lf ssh {}`",
            wave.name(),
            placement.home_id,
            placement.home_id
        )));
    }
    Ok(())
}

pub(crate) async fn rotation_lock(wave: &Wave) -> OpsResult<File> {
    let path = crate::store::current_home_lf_home_dir().join("chapter-locks");
    #[cfg(test)]
    let path = super::pm::PM_TEST_CONTEXT
        .try_with(|context| context.path.with_extension("chapter-locks"))
        .unwrap_or(path);
    std::fs::create_dir_all(&path).map_err(error)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.join(format!("{}.lock", wave.id())))
        .map_err(error)?;
    // OS ownership releases on crash; provider state makes the next holder a resumer.
    for _ in 0..300 {
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => return Ok(file),
            Err(cause) if cause.kind() == std::io::ErrorKind::WouldBlock => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(cause) => return Err(error(cause)),
        }
    }
    Err(error(
        "another chapter rotation is active; retry the same chapter id",
    ))
}

async fn disposition(store: &Store, item: PmItem) -> OpsResult<ChapterTask> {
    let mut evidence = TaskStartEvidence {
        begun: false,
        worker_claimed: false,
        // This Home's missing Task row says nothing about work on another Home.
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
        if let Some(pr) = prs.last() {
            evidence.authored = is_clean(&task.worktree).ok().and_then(|clean| {
                rev_parse(&task.worktree, "HEAD")
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
    pub worker_claimed: bool,
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
            || evidence.worker_claimed
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
            TaskDisposition::Abandon,
            "finish confirmed local backlog retirement".into(),
        );
    }
    if terminal || evidence.completed {
        if evidence.worker_claimed {
            return (
                TaskDisposition::Unresolved,
                "terminal planning state conflicts with an active worker claim".into(),
            );
        }
        return (
            TaskDisposition::Historical,
            "work is already terminal".into(),
        );
    }
    if evidence.worker_claimed
        || evidence.begun
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
            TaskDisposition::Abandon,
            "unstarted backlog expires at the chapter boundary".into(),
        );
    }
    (
        TaskDisposition::Unresolved,
        "start evidence is unavailable; refresh before retiring this Task".into(),
    )
}

#[cfg(test)]
#[path = "chapter_tests.rs"]
mod tests;
