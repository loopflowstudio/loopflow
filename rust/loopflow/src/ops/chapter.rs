//! Deterministic chapter rotation. Skills author plans; this operation alone
//! replaces their membership, preserving Task execution and resumable receipts.

use std::fs::{File, OpenOptions};
use std::path::Path;
use std::time::Duration;

use crate::durable::WorkRef;
use crate::engine::git::{is_clean, rev_parse};
use crate::ops::{OpsError, OpsResult};
use crate::pm::{PmItem, PmProject, ProjectContent, ProjectFlowPlan};
use crate::store::Store;
use crate::work::chapter::{
    classify_task, Chapter, ChapterHistoryEntry, ChapterId, ChapterMetricEvidence, ChapterPhase,
    ChapterSnapshot, ChapterTask, TaskDisposition, TaskStartEvidence,
};
use crate::work::project::{Project, ProjectId};
use crate::work::wave::{Wave, WaveLocator};

use super::pm::{
    fetch_pm_snapshot, linear_project_name, pm_store, refresh_pm_snapshot, resolve_context,
    resolve_wave, PmContext,
};

#[derive(Debug, Clone)]
pub struct NewChapterRequest {
    pub wave: Option<String>,
    pub chapter: ChapterId,
    pub content: ProjectContent,
}

fn error(value: impl std::fmt::Display) -> OpsError {
    OpsError::Message(value.to_string())
}

pub fn empty_plan() -> ProjectContent {
    ProjectContent {
        metric_targets: Vec::new(),
        flows: ProjectFlowPlan::empty(),
        krs: Vec::new(),
    }
}

pub fn new_chapter(repo: &Path, request: &NewChapterRequest, dry_run: bool) -> OpsResult<Chapter> {
    tokio::runtime::Runtime::new()
        .map_err(error)?
        .block_on(rotate(repo, request, dry_run))
}

pub fn chapter_snapshot(
    repo: &Path,
    wave: Option<&str>,
    chapter: Option<&ChapterId>,
) -> OpsResult<ChapterSnapshot> {
    let wave =
        crate::work::wave::context::resolve_managed_wave_sync(Some(repo), wave).map_err(error)?;
    tokio::runtime::Runtime::new()
        .map_err(error)?
        .block_on(async {
            let store = pm_store().await?;
            read_chapter(&store, &wave, chapter).await
        })
}

pub(crate) async fn read_chapter(
    store: &Store,
    wave: &Wave,
    id: Option<&ChapterId>,
) -> OpsResult<ChapterSnapshot> {
    let chapters = store.chapters(wave.id()).await.map_err(error)?;
    if let Some(id) = id.filter(|id| !chapters.iter().any(|chapter| &chapter.id == *id)) {
        for next in &chapters {
            for predecessor in &next.predecessors {
                if id.as_str() == format!("legacy-{}", predecessor.id) {
                    return frozen_chapter(wave, id.clone(), predecessor, next, next.clone());
                }
            }
        }
    }
    let chapter = store
        .chapter(wave.id(), id)
        .await
        .map_err(error)?
        .ok_or_else(|| error("no recorded chapter for this Wave"))?;
    let successor = store
        .chapters(wave.id())
        .await
        .map_err(error)?
        .into_iter()
        .find(|next| {
            next.activated_at.is_some()
                && next
                    .predecessors
                    .iter()
                    .any(|project| project.id == chapter.project_id)
        });
    if let Some(successor) = successor {
        let plan = successor
            .predecessors
            .iter()
            .find(|project| project.id == chapter.project_id)
            .expect("successor contains predecessor");
        return frozen_chapter(wave, chapter.id.clone(), plan, &successor, chapter);
    }
    let row = store
        .pm_snapshot(wave.id())
        .await
        .map_err(error)?
        .ok_or_else(|| error("chapter evidence unavailable; run `lf pm sync --wave <wave>`"))?;
    let snapshot: crate::pm::PmSnapshot = serde_json::from_str(&row.payload).map_err(error)?;
    let plan = snapshot
        .projects
        .into_iter()
        .find(|project| project.id == chapter.project_id)
        .ok_or_else(|| {
            error("current chapter content unavailable; resume its transition or refresh PM")
        })?;
    let mut tasks = Vec::new();
    for item in snapshot
        .items
        .into_iter()
        .filter(|item| item.project_id == chapter.project_id)
    {
        tasks.push(disposition(store, item).await?);
    }
    let evaluated_at = time::OffsetDateTime::now_utc();
    let metrics =
        super::metrics::chapter_metric_portfolio(store, wave, &plan.metric_targets, evaluated_at)
            .await
            .map_err(error)?;
    Ok(ChapterSnapshot {
        metrics,
        metrics_evaluated_at: evaluated_at.unix_timestamp(),
        id: chapter.id.clone(),
        wave: wave.name().to_string(),
        source_project_id: chapter.project_id.clone(),
        source_project_slug: plan.slug.clone(),
        content: ProjectContent {
            metric_targets: plan.metric_targets.clone(),
            flows: plan.flows.unwrap_or_else(ProjectFlowPlan::empty),
            krs: plan.krs,
        },
        tasks,
        observed_at: row.synced_at,
        closed_at: None,
        transition: chapter,
    })
}

fn frozen_chapter(
    wave: &Wave,
    id: ChapterId,
    plan: &PmProject,
    successor: &Chapter,
    transition: Chapter,
) -> OpsResult<ChapterSnapshot> {
    let observed_at = successor
        .activated_at
        .ok_or_else(|| error("chapter boundary is not yet recorded"))?;
    let evidence = successor
        .predecessor_metrics
        .iter()
        .find(|evidence| evidence.project_id == plan.id)
        .ok_or_else(|| error("chapter metric boundary evidence unavailable"))?;
    Ok(ChapterSnapshot {
        metrics: evidence.portfolio.clone(),
        metrics_evaluated_at: evidence.evaluated_at,
        id,
        wave: wave.name().to_string(),
        source_project_id: plan.id.clone(),
        source_project_slug: plan.slug.clone(),
        content: ProjectContent {
            metric_targets: plan.metric_targets.clone(),
            flows: plan.flows.clone().unwrap_or_else(ProjectFlowPlan::empty),
            krs: plan.krs.clone(),
        },
        tasks: successor
            .tasks
            .iter()
            .filter(|task| task.at_boundary && task.task.project_id == plan.id)
            .cloned()
            .collect(),
        observed_at,
        closed_at: Some(observed_at),
        transition,
    })
}

pub fn chapter_history(repo: &Path, wave: Option<&str>) -> OpsResult<Vec<ChapterHistoryEntry>> {
    let wave =
        crate::work::wave::context::resolve_managed_wave_sync(Some(repo), wave).map_err(error)?;
    tokio::runtime::Runtime::new()
        .map_err(error)?
        .block_on(async {
            let store = pm_store().await?;
            let chapters = store.chapters(wave.id()).await.map_err(error)?;
            let mut history = Vec::new();
            for chapter in &chapters {
                let successor = chapters.iter().find(|next| {
                    next.activated_at.is_some()
                        && next
                            .predecessors
                            .iter()
                            .any(|plan| plan.id == chapter.project_id)
                });
                let source = successor.and_then(|next| {
                    next.predecessors
                        .iter()
                        .find(|plan| plan.id == chapter.project_id)
                });
                let local = store
                    .get_project_by_project(&chapter.project_id)
                    .await
                    .map_err(error)?;
                history.push(ChapterHistoryEntry {
                    id: chapter.id.clone(),
                    source_project_id: chapter.project_id.clone(),
                    source_project_slug: source
                        .map(|plan| plan.slug.clone())
                        .or_else(|| local.as_ref().map(|project| project.plan.slug.clone()))
                        .unwrap_or_default(),
                    source_work_id: local.map(|project| project.id.to_string()),
                    closed_at: successor.and_then(|next| next.activated_at),
                    phase: chapter.phase,
                });
                for source in &chapter.predecessors {
                    if chapter.activated_at.is_some()
                        && !chapters.iter().any(|old| old.project_id == source.id)
                    {
                        history.push(ChapterHistoryEntry {
                            id: ChapterId::parse(&format!("legacy-{}", source.id))
                                .map_err(error)?,
                            source_project_id: source.id.clone(),
                            source_project_slug: source.slug.clone(),
                            source_work_id: store
                                .get_project_by_project(&source.id)
                                .await
                                .map_err(error)?
                                .map(|project| project.id.to_string()),
                            closed_at: chapter.activated_at,
                            phase: ChapterPhase::Complete,
                        });
                    }
                }
            }
            Ok(history)
        })
}

pub fn update_plan(repo: &Path, wave: Option<&str>, content: &ProjectContent) -> OpsResult<()> {
    content.validate().map_err(error)?;
    let wave =
        crate::work::wave::context::resolve_managed_wave_sync(Some(repo), wave).map_err(error)?;
    tokio::runtime::Runtime::new()
        .map_err(error)?
        .block_on(async {
            let store = pm_store().await?;
            require_chapter_home(&store, &wave).await?;
            let _lock = rotation_lock(&wave).await?;
            let chapter = store
                .chapter(wave.id(), None)
                .await
                .map_err(error)?
                .ok_or_else(|| error("Wave has no chapter"))?;
            super::metrics::validate_chapter_targets(&wave, &content.metric_targets)
                .map_err(error)?;
            let ctx = resolve_context(repo, wave.name()).await?;
            let project = ctx
                .client
                .project_ownership(&chapter.project_id)
                .await
                .map_err(error)?;
            ctx.client
                .update_project(&project.id, &project.name, content)
                .await
                .map_err(error)?;
            refresh_pm_snapshot(repo, wave.name(), &ctx).await?;
            Ok(())
        })
}

pub(crate) async fn current_project(store: &Store, wave: &Wave) -> OpsResult<PmProject> {
    let chapter = store
        .chapter(wave.id(), None)
        .await
        .map_err(error)?
        .ok_or_else(|| {
            error(format!(
                "Wave {} has no chapter; run `lf wave new-chapter --wave {} --chapter <id>`",
                wave.name(),
                wave.name()
            ))
        })?;
    let snapshot = store
        .pm_snapshot(wave.id())
        .await
        .map_err(error)?
        .ok_or_else(|| error("chapter planning is unavailable; refresh the Wave's PM snapshot"))?;
    let snapshot: crate::pm::PmSnapshot = serde_json::from_str(&snapshot.payload).map_err(error)?;
    snapshot
        .projects
        .into_iter()
        .find(|project| project.id == chapter.project_id)
        .ok_or_else(|| {
            error("current chapter is missing from the PM snapshot; refresh its evidence")
        })
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
    // OS ownership releases on crash; durable receipts make the next holder a resumer.
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

pub(crate) async fn rotate(
    repo: &Path,
    request: &NewChapterRequest,
    dry_run: bool,
) -> OpsResult<Chapter> {
    request.content.validate().map_err(error)?;
    let store = pm_store().await?;
    let name = match request.wave.as_deref() {
        Some(name) => resolve_wave(Some(name))?,
        None => crate::work::wave::context::resolve_managed_wave(
            Some(&store),
            Some(repo),
            None,
            std::env::var(crate::work::wave::context::WAVE_ID_ENV)
                .ok()
                .as_deref(),
        )
        .await
        .map_err(error)?
        .name()
        .to_string(),
    };
    let locator = WaveLocator::discover(repo, &name).map_err(error)?;
    let wave = if dry_run {
        store
            .get_wave_at(&locator)
            .await
            .map_err(error)?
            .ok_or_else(|| error("initialize the Wave before previewing its first chapter"))?
    } else {
        crate::controller::wave::registry::ensure_wave_row(&store, repo, &name)
            .await
            .map_err(error)?
    };
    if !dry_run {
        require_chapter_home(&store, &wave).await?;
    }
    let _lock = if dry_run {
        None
    } else {
        Some(rotation_lock(&wave).await?)
    };
    let existing = store
        .chapter(wave.id(), Some(&request.chapter))
        .await
        .map_err(error)?;
    if let Some(chapter) = &existing {
        if chapter.content != request.content {
            return Err(error("this chapter id already has different content; update its plan explicitly or use a new id"));
        }
        if chapter.phase == ChapterPhase::Complete {
            return Ok(chapter.clone());
        }
    }
    // Once activated, resume the recorded boundary even if instruments changed.
    // Task transfers must not depend on resampling already-frozen metric evidence.
    if existing
        .as_ref()
        .is_none_or(|chapter| chapter.activated_at.is_none())
    {
        super::metrics::validate_chapter_targets(&wave, &request.content.metric_targets)
            .map_err(error)?;
    }
    let ctx = resolve_context(repo, &name).await?;
    let mut chapter = match existing {
        Some(mut chapter) => {
            if dry_run && chapter.activated_at.is_none() {
                refresh_boundary(repo, &store, &wave, &ctx, &mut chapter).await?;
            }
            chapter
        }
        None => {
            if store
                .chapters(wave.id())
                .await
                .map_err(error)?
                .iter()
                .any(|chapter| chapter.phase != ChapterPhase::Complete)
            {
                return Err(error(
                    "resume the unfinished chapter before starting another",
                ));
            }
            let snapshot = fetch_pm_snapshot(repo, &name, &ctx).await?;
            let tasks =
                rotation_tasks(&store, &wave, &ctx, &snapshot.projects, snapshot.items).await?;
            Chapter {
                predecessor_metrics: Vec::new(),
                id: request.chapter.clone(),
                wave_id: wave.id().clone(),
                wave: name.clone(),
                project_id: uuid::Uuid::new_v4().to_string(),
                content: request.content.clone(),
                predecessors: snapshot.projects,
                tasks,
                phase: if dry_run {
                    ChapterPhase::Preview
                } else {
                    ChapterPhase::Preparing
                },
                created_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                activated_at: None,
                completed_at: None,
                error: None,
            }
        }
    };
    if dry_run {
        chapter.phase = ChapterPhase::Preview;
        for index in 0..chapter.tasks.len() {
            if chapter.activated_at.is_some() && !chapter.tasks[index].applied {
                chapter.tasks[index] =
                    refresh_disposition(&store, &ctx, &chapter, &chapter.tasks[index]).await?;
            }
        }
        return Ok(chapter);
    }
    chapter.error = None;
    store.save_chapter(&chapter, false).await.map_err(error)?;
    if let Err(cause) = apply_rotation(repo, &store, &wave, &ctx, &mut chapter).await {
        chapter.error = Some(cause.to_string());
        store.save_chapter(&chapter, false).await.map_err(error)?;
    }
    Ok(chapter)
}

async fn rotation_tasks(
    store: &Store,
    wave: &Wave,
    ctx: &PmContext,
    predecessors: &[PmProject],
    mut items: Vec<PmItem>,
) -> OpsResult<Vec<ChapterTask>> {
    let projects = store.list_projects(Some(wave.id())).await.map_err(error)?;
    for task in store.list_tasks(Some(wave.id())).await.map_err(error)? {
        let belongs = projects.iter().any(|project| {
            project.id == task.project_id
                && predecessors
                    .iter()
                    .any(|plan| plan.id == project.plan.id.as_str())
        });
        if !belongs || items.iter().any(|item| item.id == task.plan.id.as_str()) {
            continue;
        }
        // A list omission is not evidence that durable predecessor work disappeared.
        let (item, _) = ctx
            .client
            .issue_ownership(task.plan.id.as_str())
            .await
            .map_err(|cause| {
                error(format!(
                    "{}: chapter Task evidence unavailable: {cause}",
                    task.plan.identifier
                ))
            })?;
        if !predecessors.iter().any(|plan| plan.id == item.project_id) {
            return Err(error(format!(
                "{}: provider membership conflicts with its recorded chapter; reconcile the Task before rotating",
                task.plan.identifier
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

async fn refresh_boundary(
    repo: &Path,
    store: &Store,
    wave: &Wave,
    ctx: &PmContext,
    chapter: &mut Chapter,
) -> OpsResult<()> {
    let snapshot = fetch_pm_snapshot(repo, wave.name(), ctx).await?;
    let predecessors: Vec<_> = snapshot
        .projects
        .into_iter()
        .filter(|project| project.id != chapter.project_id)
        .collect();
    let items = snapshot
        .items
        .into_iter()
        .filter(|item| item.project_id != chapter.project_id)
        .collect();
    let tasks = rotation_tasks(store, wave, ctx, &predecessors, items).await?;
    // Keep the previous complete boundary if any fresh evidence is unavailable.
    chapter.predecessors = predecessors;
    chapter.tasks = tasks;
    Ok(())
}

async fn refresh_disposition(
    store: &Store,
    ctx: &PmContext,
    chapter: &Chapter,
    recorded: &ChapterTask,
) -> OpsResult<ChapterTask> {
    let (fresh, _) = ctx
        .client
        .issue_ownership(&recorded.task.id)
        .await
        .map_err(error)?;
    let mut decision = disposition(store, fresh).await?;
    if decision.task.project_id != chapter.project_id
        && !chapter
            .predecessors
            .iter()
            .any(|project| project.id == decision.task.project_id)
    {
        decision.disposition = TaskDisposition::Unresolved;
        decision.reason =
            "provider membership is outside this chapter transition; reconcile the Task before retrying"
                .into();
    }
    if decision.disposition == TaskDisposition::Unresolved {
        return Ok(decision);
    }
    // Preview and application reconcile the same successful, unacknowledged effects.
    if decision.task.project_id == chapter.project_id {
        decision.disposition = TaskDisposition::Move;
        decision.reason = "reconcile recorded chapter transfer".into();
    } else if !decision.task.completed
        && matches!(
            decision.task.state.as_deref(),
            Some("backlog" | "unstarted" | "triage" | "canceled")
        )
    {
        let retired = if let Some(task) = store
            .get_task_by_issue(&recorded.task.id)
            .await
            .map_err(error)?
        {
            store
                .chapter_task_evidence(&task.id)
                .await
                .map_err(error)?
                .abandoned
        } else {
            false
        };
        if retired
            || (recorded.disposition == TaskDisposition::Abandon
                && decision.task.state.as_deref() == Some("canceled"))
        {
            decision.disposition = TaskDisposition::Abandon;
            decision.reason = "reconcile recorded backlog retirement".into();
        }
    }
    Ok(decision)
}

async fn disposition(store: &Store, item: PmItem) -> OpsResult<ChapterTask> {
    let mut evidence = TaskStartEvidence {
        begun: false,
        worker_claimed: false,
        authored: Some(false),
        published: false,
        abandoned: false,
        completed: false,
    };
    if let Some(task) = store.get_task_by_issue(&item.id).await.map_err(error)? {
        evidence = store.chapter_task_evidence(&task.id).await.map_err(error)?;
        let home = crate::store::observability_home_dir();
        #[cfg(test)]
        let home = super::pm::PM_TEST_CONTEXT
            .try_with(|context| context.path.with_extension("runs"))
            .unwrap_or(home);
        evidence.begun |=
            crate::run_record::scan_runs_since(&home, task.created_at.unix_timestamp())
                .map_err(error)?
                .iter()
                .any(|run| {
                    run.subject("task").is_some_and(|subject| {
                        subject == task.id.as_str()
                            || subject == task.plan.id.as_str()
                            || subject == task.plan.identifier
                    })
                });
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
        applied: false,
        observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
        at_boundary: false,
    })
}

async fn ensure_successor(
    repo: &Path,
    store: &Store,
    wave: &Wave,
    ctx: &PmContext,
    chapter: &Chapter,
) -> OpsResult<Project> {
    let title = format!("Chapter {}", chapter.id.as_str());
    let linear_name = linear_project_name(repo, wave.name(), &title).await?;
    let project = match ctx.client.project_ownership(&chapter.project_id).await {
        Ok(project) => project,
        Err(_) => {
            let create = ctx
                .client
                .create_project(
                    &ctx.initiative,
                    &linear_name,
                    &chapter.content,
                    Some(&chapter.project_id),
                )
                .await;
            match ctx.client.project_ownership(&chapter.project_id).await {
                Ok(project) => project,
                Err(read_error) => return Err(error(create.err().unwrap_or(read_error))),
            }
        }
    };
    if project.name != linear_name || !project.team_ids.contains(&ctx.team_id) {
        return Err(error(
            "successor identity conflicts with the recorded chapter",
        ));
    }
    if !project.initiative_ids.contains(&ctx.initiative) {
        ctx.client
            .attach_project(&ctx.initiative, &chapter.project_id)
            .await
            .map_err(error)?;
    }
    if let Some(project) = store
        .get_project_by_project(&chapter.project_id)
        .await
        .map_err(error)?
    {
        return Ok(project);
    }
    let now = time::OffsetDateTime::now_utc();
    let project = Project {
        id: ProjectId::new(),
        plan: super::project::project_plan(&project, now.unix_timestamp())?,
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
    chapter: &mut Chapter,
) -> OpsResult<()> {
    // Re-enumerate before cutover: a Task created since preview belongs in the receipt.
    if chapter.activated_at.is_none() {
        refresh_boundary(repo, store, wave, ctx, chapter).await?;
        store.save_chapter(chapter, false).await.map_err(error)?;
    }
    if chapter.activated_at.is_none() {
        if let Some(task) = chapter
            .tasks
            .iter()
            .find(|task| task.disposition == TaskDisposition::Unresolved)
        {
            return Err(error(format!("{}: {}", task.task.identifier, task.reason)));
        }
    }
    let successor = ensure_successor(repo, store, wave, ctx, chapter).await?;
    if chapter.activated_at.is_none() {
        let evaluated_at = time::OffsetDateTime::now_utc();
        chapter.predecessor_metrics.clear();
        for plan in &chapter.predecessors {
            let portfolio = super::metrics::chapter_metric_portfolio(
                store,
                wave,
                &plan.metric_targets,
                evaluated_at,
            )
            .await
            .map_err(error)?;
            chapter.predecessor_metrics.push(ChapterMetricEvidence {
                project_id: plan.id.clone(),
                evaluated_at: evaluated_at.unix_timestamp(),
                portfolio,
            });
        }
        chapter.activated_at = Some(evaluated_at.unix_timestamp());
        for task in &mut chapter.tasks {
            task.at_boundary = true;
        }
        chapter.phase = ChapterPhase::Transferring;
        store.save_chapter(chapter, true).await.map_err(error)?;
    }
    for index in 0..chapter.tasks.len() {
        if chapter.tasks[index].applied {
            continue;
        }
        let original = chapter.tasks[index].task.clone();
        let mut decision = refresh_disposition(store, ctx, chapter, &chapter.tasks[index]).await?;
        let local = store.get_task_by_issue(&original.id).await.map_err(error)?;
        if decision.disposition == TaskDisposition::Abandon {
            if let Some(task) = &local {
                let retired = store
                    .chapter_task_evidence(&task.id)
                    .await
                    .map_err(error)?
                    .abandoned;
                if !retired
                    && !store
                        .retire_chapter_backlog(&task.id)
                        .await
                        .map_err(error)?
                {
                    decision = disposition(store, decision.task).await?;
                    if decision.disposition == TaskDisposition::Abandon {
                        decision.disposition = TaskDisposition::Unresolved;
                        decision.reason =
                            "Task changed during retirement; refresh its execution evidence".into();
                    }
                }
            }
        }
        chapter.tasks[index].disposition = decision.disposition;
        chapter.tasks[index].reason = decision.reason;
        store.save_chapter(chapter, false).await.map_err(error)?;
        match decision.disposition {
            TaskDisposition::Move => {
                if decision.task.project_id != chapter.project_id {
                    ctx.client
                        .move_item_to_project(&original.id, &chapter.project_id)
                        .await
                        .map_err(error)?;
                }
                if let Some(task) = local {
                    store
                        .move_chapter_task(&task.id, &successor.id)
                        .await
                        .map_err(error)?;
                }
            }
            TaskDisposition::Abandon => {
                if decision.task.state.as_deref() != Some("canceled") {
                    ctx.client.cancel_item(&original.id).await.map_err(error)?;
                }
            }
            TaskDisposition::Historical => {}
            TaskDisposition::Unresolved => {
                return Err(error(format!(
                    "{}: {}",
                    original.identifier, chapter.tasks[index].reason
                )))
            }
        }
        chapter.tasks[index].applied = true;
        store.save_chapter(chapter, false).await.map_err(error)?;
    }
    // External Task filing can race with cutover. Never archive unseen work.
    let mut added = false;
    for predecessor in &chapter.predecessors {
        for item in ctx
            .client
            .list_items(&predecessor.id)
            .await
            .map_err(error)?
        {
            if !chapter.tasks.iter().any(|task| task.task.id == item.id) {
                chapter.tasks.push(disposition(store, item).await?);
                added = true;
            }
        }
    }
    if added {
        store.save_chapter(chapter, false).await.map_err(error)?;
        return Err(error(
            "new Tasks appeared during rotation; retry this chapter to reconcile them",
        ));
    }
    for predecessor in &chapter.predecessors {
        ctx.client
            .archive_project(&predecessor.id)
            .await
            .map_err(error)?;
        if let Some(project) = store
            .get_project_by_project(&predecessor.id)
            .await
            .map_err(error)?
        {
            let work = WorkRef::Project(project.id);
            if store.work_status(&work).await.map_err(error)? == crate::durable::WorkStatus::Ready {
                store
                    .abandon(&work, "chapter replaced")
                    .await
                    .map_err(error)?;
            }
        }
    }
    refresh_pm_snapshot(repo, wave.name(), ctx).await?;
    chapter.phase = ChapterPhase::Complete;
    chapter.completed_at = Some(time::OffsetDateTime::now_utc().unix_timestamp());
    store.save_chapter(chapter, false).await.map_err(error)
}

#[cfg(test)]
#[path = "chapter_tests.rs"]
mod tests;
