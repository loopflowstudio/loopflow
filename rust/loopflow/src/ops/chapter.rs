//! Explicit Project rotation. Shared bindings select Projects; receipts retain recovery evidence.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::engine::git::{is_clean, rev_parse};
use crate::id::WaveId;
use crate::ops::{OpsError, OpsResult};
use crate::pm::{PmItem, PmProject, ProjectContent, ProjectStatus};
use crate::store::project_transitions::ProjectTransition;
use crate::store::sqlite::project_selection::{read_project_binding, write_project_binding};
use crate::store::{PlanningLocks, Store};
use crate::work::wave::{Wave, WaveLocator};

use super::pm::{lock_wave_planning, pm_store, project_is_foreign, resolve_context, PmContext};

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

struct PreparedRotation {
    input: WaveChapterPlan,
    wave: Wave,
    ctx: PmContext,
    acquisition: Arc<PlanningLocks>,
    transition: ProjectTransition,
    reserved: bool,
    switched: bool,
    conversions: Vec<String>,
    result: WaveRotation,
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
    tokio::runtime::Runtime::new()
        .map_err(error)?
        .block_on(rotate(repo, &plan, wave, dry_run))
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
        let successor = uuid::Uuid::parse_str(
            input
                .successor_id
                .strip_prefix("proj_")
                .unwrap_or(&input.successor_id),
        )
        .map_err(error)?;
        if input.create && successor.get_version() != Some(uuid::Version::Random) {
            return Err(error(
                "new Project destinations require a UUID v4 allocated with the plan",
            ));
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
        let ctx = if store
            .sqlite
            .personal_wave_definition(wave.id())
            .map_err(error)?
            .is_some()
        {
            None
        } else {
            Some(resolve_context(repo, wave.slug()).await?)
        };
        contexts.push((input, wave, ctx, acquisition));
    }
    let mut roots = Vec::new();
    for (_, wave, _, _) in &contexts {
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
    let mut personal = Vec::new();
    let mut personal_guards = Vec::new();
    for (input, wave, ctx, acquisition) in contexts {
        let acquisition = Arc::new(acquisition.with_checkouts(&checkouts));
        let Some(ctx) = ctx else {
            personal.push(prepare_local_rotation(&store, input, &wave, &plan.name).await?);
            personal_guards.push(acquisition);
            continue;
        };
        if !dry_run {
            super::project::import_binding(&store, &wave, &ctx, &acquisition).await?;
        }
        let binding = read_project_binding(&store.sqlite, wave.id()).map_err(error)?;
        let existing = store
            .project_transition(wave.id(), &input.successor_id)
            .await
            .map_err(error)?;
        let pending = store
            .pending_project_transition(wave.id())
            .await
            .map_err(error)?;
        if pending
            .as_ref()
            .is_some_and(|t| t.successor_id != input.successor_id)
        {
            return Err(error(format!(
                "Wave {} has another unfinished transition",
                wave.slug()
            )));
        }
        let reserved = existing.is_some();
        let transition = existing.unwrap_or_else(|| ProjectTransition {
            wave_id: wave.id().clone(),
            successor_id: input.successor_id.clone(),
            predecessor_id: binding.clone(),
            reset_name: Some(plan.name.clone()),
            create_successor: Some(input.create),
            created_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            settled_at: None,
        });
        if transition.create_successor != Some(input.create)
            || transition.reset_name.as_deref() != Some(&plan.name)
            || transition.predecessor_id.as_deref() == Some(&input.successor_id)
        {
            return Err(error("retained transition does not match this plan"));
        }
        let switched = binding.as_deref() == Some(&input.successor_id);
        if !switched && binding != transition.predecessor_id
            || transition.settled_at.is_some() && !switched
        {
            return Err(error(
                "Project selection changed since this plan; preserve the intervening decision",
            ));
        }
        let project_observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
        let pending_conversion = store
            .projects_pending_adoption(wave.id())
            .await
            .map_err(error)?;
        let mut conversions = Vec::new();
        let mut read = async |id: &str| -> OpsResult<Option<PmProject>> {
            if pending_conversion.iter().any(|(pending, _)| pending == id) {
                conversions.push(id.to_owned());
                Ok(Some(
                    ctx.client
                        .adopt_project(id, &ctx.initiative, &ctx.team_id, false, false)
                        .await
                        .map_err(error)?,
                ))
            } else {
                ctx.client.find_project(id).await.map_err(error)
            }
        };
        let predecessor = match &transition.predecessor_id {
            Some(id) => {
                let project = read(id)
                    .await?
                    .ok_or_else(|| error("predecessor is unavailable"))?;
                require_owned(&ctx, &project, false)?;
                if !matches!(
                    project.status,
                    ProjectStatus::Backlog
                        | ProjectStatus::Planned
                        | ProjectStatus::Started
                        | ProjectStatus::Completed
                ) {
                    return Err(error("predecessor status changed"));
                }
                if !reserved && project.status == ProjectStatus::Completed {
                    return Err(error("configured predecessor is terminal"));
                }
                Some(project)
            }
            None => None,
        };
        let successor = read(&input.successor_id).await?;
        if let Some(project) = &successor {
            require_owned(&ctx, project, input.create && reserved)?;
            if !matches!(
                project.status,
                ProjectStatus::Backlog | ProjectStatus::Planned | ProjectStatus::Started
            ) {
                return Err(error("successor is no longer available for activation"));
            }
            if switched
                && (project.status != ProjectStatus::Started
                    || !matches_plan(project, &input.content))
            {
                return Err(error(
                    "selected destination changed after the Project switch",
                ));
            }
            if input.create && !reserved {
                return Err(error(
                    "creation destination already exists without this reservation",
                ));
            }
        } else if !input.create || switched || transition.settled_at.is_some() {
            return Err(error(
                "selected destination is unavailable; it cannot be recreated",
            ));
        } else {
            ctx.client
                .require_initiative(&ctx.initiative)
                .await
                .map_err(error)?;
        }
        let mut entry = PreparedRotation {
            input: input.clone(),
            wave,
            ctx,
            acquisition,
            transition,
            reserved,
            switched,
            conversions,
            result: WaveRotation {
                wave: String::new(),
                successor_id: input.successor_id.clone(),
                predecessor,
                successor,
                tasks: Vec::new(),
            },
        };
        if let Some(project) = entry.result.predecessor.take() {
            let project = accept_project(&store, &entry, project, project_observed_at).await?;
            if !matches!(
                project.status,
                ProjectStatus::Backlog
                    | ProjectStatus::Planned
                    | ProjectStatus::Started
                    | ProjectStatus::Completed
            ) || (!entry.reserved && project.status == ProjectStatus::Completed)
            {
                return Err(error("accepted predecessor status prevents rotation"));
            }
            entry.result.predecessor = Some(project);
        }
        if let Some(project) = entry.result.successor.take() {
            let project = if project.initiative_ids.is_empty() {
                project
            } else {
                accept_project(&store, &entry, project, project_observed_at).await?
            };
            if !matches!(
                project.status,
                ProjectStatus::Backlog | ProjectStatus::Planned | ProjectStatus::Started
            ) || (entry.switched
                && (project.status != ProjectStatus::Started
                    || !matches_plan(&project, &entry.input.content)))
            {
                return Err(error("accepted successor facts prevent rotation"));
            }
            entry.result.successor = Some(project);
        }
        entry.result.wave = entry.wave.slug().to_owned();
        entry.result.tasks = rotation_tasks(&store, &entry).await?;
        prepared.push(entry);
    }
    if dry_run {
        return Ok(ChapterRotation {
            name: plan.name.clone(),
            waves: personal
                .into_iter()
                .map(|(_, result)| result)
                .chain(prepared.into_iter().map(|entry| entry.result))
                .collect(),
        });
    }
    for entry in &prepared {
        if let Some(task) = entry
            .result
            .tasks
            .iter()
            .find(|task| task.disposition == TaskDisposition::Unresolved)
        {
            return Err(error(format!(
                "{}: {}; no provider writes performed",
                task.task.identifier, task.reason
            )));
        }
    }
    store
        .sqlite
        .rotate_local_projects(&plan.name, &personal)
        .map_err(error)?;
    for (input, result) in &mut personal {
        let id = crate::durable::ProjectId::parse(&input.successor_id).map_err(error)?;
        result.successor = Some(super::task::project_planning_item(
            &store,
            store
                .sqlite
                .project(&id)
                .map_err(error)?
                .ok_or_else(|| error("rotation destination disappeared"))?,
        )?);
        if let Some(predecessor) = &mut result.predecessor {
            let id = crate::durable::ProjectId::parse(&predecessor.id).map_err(error)?;
            *predecessor = super::task::project_planning_item(
                &store,
                store
                    .sqlite
                    .project(&id)
                    .map_err(error)?
                    .ok_or_else(|| error("rotation predecessor disappeared"))?,
            )?;
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
    drop(personal_guards);
    // Every pair is reserved before the first provider mutation.
    for entry in &prepared {
        if !entry.reserved {
            store
                .reserve_project_transition(entry.transition.clone(), entry.acquisition.clone())
                .await
                .map_err(error)?;
        }
    }
    for entry in &mut prepared {
        apply_rotation(&store, entry).await.map_err(|cause| {
            error(format!(
                "Wave {}: {cause}; retry with the same chapter plan",
                entry.wave.slug()
            ))
        })?;
    }
    Ok(ChapterRotation {
        name: plan.name.clone(),
        waves: personal
            .into_iter()
            .map(|(_, result)| result)
            .chain(prepared.into_iter().map(|entry| entry.result))
            .collect(),
    })
}

async fn prepare_local_rotation(
    store: &Store,
    input: &WaveChapterPlan,
    wave: &Wave,
    name: &str,
) -> OpsResult<(WaveChapterPlan, WaveRotation)> {
    let uuid = uuid::Uuid::parse_str(
        input
            .successor_id
            .strip_prefix("proj_")
            .unwrap_or(&input.successor_id),
    )
    .map_err(error)?;
    let mut input = input.clone();
    input.successor_id = format!("proj_{}", uuid.simple());
    let binding = read_project_binding(&store.sqlite, wave.id()).map_err(error)?;
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
                "retained local rotation conflicts with this request or the selected Project",
            ));
        }
    }
    let predecessor_id = receipt
        .as_ref()
        .map(|r| r.predecessor_id.clone())
        .unwrap_or(binding);
    let projects = store
        .list_projects(Some(wave.id()))
        .await
        .map_err(error)?
        .into_iter()
        .map(|project| super::task::project_planning_item(store, project))
        .collect::<OpsResult<Vec<_>>>()?;
    let predecessor = predecessor_id
        .as_ref()
        .and_then(|id| projects.iter().find(|p| &p.id == id))
        .cloned();
    let successor = projects
        .iter()
        .find(|p| p.id == input.successor_id)
        .cloned();
    if receipt.is_none()
        && (input.create == successor.is_some()
            || predecessor_id.as_deref() == Some(&input.successor_id))
    {
        return Err(error("local destination must be new for creation or an existing different Project for selection"));
    }
    if successor
        .as_ref()
        .is_some_and(|p| matches!(p.status, ProjectStatus::Completed | ProjectStatus::Canceled))
    {
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
        let item = super::task::task_planning_item(store, &task)?;
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

fn require_owned(ctx: &PmContext, project: &PmProject, unattached: bool) -> OpsResult<()> {
    if project.team_ids != [ctx.team_id.clone()]
        || !(project.initiative_ids == [ctx.initiative.clone()]
            || unattached && project.initiative_ids.is_empty())
    {
        return Err(error("Project ownership changed; reconcile it in Linear"));
    }
    Ok(())
}

async fn rotation_tasks(store: &Store, entry: &PreparedRotation) -> OpsResult<Vec<ChapterTask>> {
    let selected = store
        .project_transition_items(entry.wave.id(), &entry.input.successor_id)
        .await
        .map_err(error)?;
    let mut items = Vec::new();
    if !entry.switched {
        if let Some(predecessor) = &entry.result.predecessor {
            items = entry
                .ctx
                .client
                .list_items(&predecessor.id)
                .await
                .map_err(error)?;
            let projects = store
                .list_projects(Some(entry.wave.id()))
                .await
                .map_err(error)?;
            for task in store
                .list_tasks(Some(entry.wave.id()))
                .await
                .map_err(error)?
            {
                if projects.iter().any(|p| {
                    p.id == task.project_id
                        && p.plan
                            .linear_id
                            .as_ref()
                            .is_some_and(|id| id.as_str() == predecessor.id)
                }) && !items.iter().any(|item| {
                    task.plan
                        .linear_id
                        .as_ref()
                        .is_some_and(|id| id.as_str() == item.id)
                }) {
                    let (item, _) = entry
                        .ctx
                        .client
                        .issue_ownership(task.plan.linear_id()?.as_str())
                        .await
                        .map_err(error)?
                        .ok_or_else(|| error("Task planning is unavailable"))?;
                    items.push(item);
                }
            }
        }
    }
    for id in &selected {
        if !items.iter().any(|item| &item.id == id) {
            items.push(
                entry
                    .ctx
                    .client
                    .issue_ownership(id)
                    .await
                    .map_err(error)?
                    .ok_or_else(|| error("selected Task planning is unavailable"))?
                    .0,
            );
        }
    }
    let mut decisions = Vec::new();
    for item in items {
        if item.team_id.as_deref() != Some(entry.ctx.team_id.as_str())
            || (item.project_id.as_deref() != entry.transition.predecessor_id.as_deref()
                && item.project_id.as_deref() != Some(&entry.input.successor_id))
        {
            return Err(error(format!(
                "{} moved outside this transition",
                item.identifier
            )));
        }
        if entry.switched && item.project_id.as_deref() != Some(&entry.input.successor_id) {
            return Err(error(format!(
                "{} moved after the Project switch; preserve the external decision",
                item.identifier
            )));
        }
        let retained = selected.contains(&item.id);
        let mut decision = disposition(store, item).await?;
        if retained && decision.disposition != TaskDisposition::Unresolved {
            decision.disposition = TaskDisposition::Move;
            decision.reason = "selected by this transition; confirm its destination".into();
        }
        decisions.push(decision);
    }
    Ok(decisions)
}

async fn accept_project(
    store: &Store,
    entry: &PreparedRotation,
    project: PmProject,
    observed_at: i64,
) -> OpsResult<PmProject> {
    super::project::accept_project(
        store,
        &entry.wave,
        &entry.ctx,
        project,
        observed_at,
        &entry.acquisition,
    )
    .await
}

async fn read_project(store: &Store, entry: &PreparedRotation, id: &str) -> OpsResult<PmProject> {
    let observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
    let project = entry
        .ctx
        .client
        .project_ownership(id)
        .await
        .map_err(error)?;
    accept_project(store, entry, project, observed_at).await
}

fn matches_plan(project: &PmProject, content: &ProjectContent) -> bool {
    project.krs == content.krs
        && project.workflow == content.workflow
        && project.metric_targets == content.metric_targets
}

async fn apply_rotation(store: &Store, entry: &mut PreparedRotation) -> OpsResult<()> {
    for id in &entry.conversions {
        entry
            .ctx
            .client
            .adopt_project(id, &entry.ctx.initiative, &entry.ctx.team_id, false, true)
            .await
            .map_err(error)?;
        store
            .finish_project_adoption(entry.wave.id(), id, entry.acquisition.clone())
            .await
            .map_err(error)?;
    }
    let id = entry.input.successor_id.clone();
    let mut observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut successor = match entry.ctx.client.find_project(&id).await.map_err(error)? {
        Some(project) => project,
        None if entry.input.create && !entry.switched => {
            let created = entry
                .ctx
                .client
                .create_project(
                    &entry.ctx.initiative,
                    &entry.input.project_name,
                    &entry.input.content,
                    Some(&id),
                )
                .await
                .map_err(error)?;
            if created != id {
                return Err(error("provider returned a different destination identity"));
            }
            observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
            entry
                .ctx
                .client
                .project_ownership(&id)
                .await
                .map_err(error)?
        }
        None => return Err(error("selected destination disappeared")),
    };
    require_owned(&entry.ctx, &successor, entry.input.create)?;
    if successor.initiative_ids.is_empty() {
        entry
            .ctx
            .client
            .attach_project(&entry.ctx.initiative, &id)
            .await
            .map_err(error)?;
        observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
        successor = entry
            .ctx
            .client
            .project_ownership(&id)
            .await
            .map_err(error)?;
    }
    successor = accept_project(store, entry, successor, observed_at).await?;
    if !matches!(
        successor.status,
        ProjectStatus::Backlog | ProjectStatus::Planned | ProjectStatus::Started
    ) {
        return Err(error("accepted successor status prevents plan changes"));
    }
    if !entry.switched && !matches_plan(&successor, &entry.input.content) {
        entry
            .ctx
            .client
            .apply_project_plan(&id, &entry.input.content)
            .await
            .map_err(error)?;
        successor = read_project(store, entry, &id).await?;
    }
    if !matches_plan(&successor, &entry.input.content) {
        return Err(error("destination content changed during rotation"));
    }
    if !matches!(
        successor.status,
        ProjectStatus::Backlog | ProjectStatus::Planned | ProjectStatus::Started
    ) {
        return Err(error("successor status changed during rotation"));
    }
    if successor.status != ProjectStatus::Started {
        entry
            .ctx
            .client
            .set_project_status(&id, ProjectStatus::Started)
            .await
            .map_err(error)?;
        successor = read_project(store, entry, &id).await?;
        if successor.status != ProjectStatus::Started {
            return Err(error("successor activation is not confirmed"));
        }
    }
    entry.result.successor = Some(successor);
    entry.result.tasks = rotation_tasks(store, entry).await?;
    for decision in &entry.result.tasks {
        if decision.disposition == TaskDisposition::Unresolved {
            return Err(error(&decision.reason));
        }
        if decision.disposition != TaskDisposition::Move {
            continue;
        }
        if !entry.switched {
            store
                .select_project_transition_item(
                    entry.wave.id(),
                    &id,
                    &decision.task.id,
                    entry.acquisition.clone(),
                )
                .await
                .map_err(error)?;
        }
        let mut observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
        let (mut item, mut project) = entry
            .ctx
            .client
            .issue_ownership(&decision.task.id)
            .await
            .map_err(error)?
            .ok_or_else(|| error("Task planning is unavailable"))?;
        if item.team_id.as_deref() != Some(entry.ctx.team_id.as_str()) {
            return Err(error("Task Team changed during rotation"));
        }
        if item.project_id.as_deref() != Some(&id) {
            if entry.switched
                || item.project_id.as_deref() != entry.transition.predecessor_id.as_deref()
            {
                return Err(error("selected Task moved outside this transition"));
            }
            entry
                .ctx
                .client
                .move_item_to_project(&item.id, &id)
                .await
                .map_err(error)?;
            observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
            (item, project) = entry
                .ctx
                .client
                .issue_ownership(&item.id)
                .await
                .map_err(error)?
                .ok_or_else(|| error("Task transfer is unavailable"))?;
            if item.project_id.as_deref() != Some(&id)
                || item.team_id.as_deref() != Some(entry.ctx.team_id.as_str())
            {
                return Err(error("Task transfer is not confirmed"));
            }
        }
        store
            .put_pm_task(
                entry.wave.repo(),
                "linear",
                crate::store::PmTaskRecord {
                    item,
                    project,
                    observed_at,
                },
                Some((entry.wave.id().clone(), entry.ctx.initiative.clone())),
                Some(entry.acquisition.clone()),
            )
            .await
            .map_err(error)?;
    }
    // Recheck provider facts before selection changes. Never sweep historical starts after it.
    if !entry.switched
        && rotation_tasks(store, entry).await?.iter().any(|task| {
            task.task.project_id.as_deref() == entry.transition.predecessor_id.as_deref()
                && task.disposition != TaskDisposition::Historical
        })
    {
        return Err(error("predecessor still has unfinished selected work"));
    }
    let current = read_project(store, entry, &id).await?;
    if current.status != ProjectStatus::Started || !matches_plan(&current, &entry.input.content) {
        return Err(error("successor changed before the Project switch"));
    }
    if let Some(previous) = &entry.transition.predecessor_id {
        let current = read_project(store, entry, previous).await?;
        if !matches!(
            current.status,
            ProjectStatus::Backlog
                | ProjectStatus::Planned
                | ProjectStatus::Started
                | ProjectStatus::Completed
        ) {
            return Err(error(
                "predecessor status changed before the Project switch",
            ));
        }
    }
    let expected = if entry.switched {
        Some(id.as_str())
    } else {
        entry.transition.predecessor_id.as_deref()
    };
    write_project_binding(
        &store.sqlite,
        entry.wave.id(),
        expected,
        &id,
        &entry.acquisition,
    )
    .map_err(error)?;
    if let Some(previous) = &entry.transition.predecessor_id {
        let mut project = read_project(store, entry, previous).await?;
        if !matches!(
            project.status,
            ProjectStatus::Backlog
                | ProjectStatus::Planned
                | ProjectStatus::Started
                | ProjectStatus::Completed
        ) {
            return Err(error("predecessor status changed during rotation"));
        }
        if project.status != ProjectStatus::Completed {
            entry
                .ctx
                .client
                .set_project_status(previous, ProjectStatus::Completed)
                .await
                .map_err(error)?;
            project = read_project(store, entry, previous).await?;
            if project.status != ProjectStatus::Completed {
                return Err(error("predecessor completion is not confirmed"));
            }
        }
    }
    store
        .settle_project_transition(entry.wave.id(), &id, entry.acquisition.clone())
        .await
        .map_err(error)?;
    Ok(())
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
