//! `lf activity` — one ordered record of durable Work facts.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::durable::{Author, Steer, SteerId, WorkRef};
use crate::lf::commands::runs::{collect_run_activity_since, SkillRunEntry};
use crate::lf::commands::util::parse_since;
use crate::lf::commands::waves::PrMergeRequestSnapshot;
use crate::lf::commands::WorkFilter;
use crate::project::Project;
use crate::store::sqlite::SqliteStore;
use crate::task::{GithubPr, Task, TaskPr, TaskPrId};
use crate::wave::Wave;

const MAX_LIMIT: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkActivitySnapshot {
    pub generated_at: i64,
    pub since: i64,
    pub limit: usize,
    pub truncated: bool,
    pub items: Vec<WorkActivityEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkActivityEntry {
    pub id: String,
    pub recorded_at: i64,
    pub summary: String,
    pub work: WorkRef,
    /// Current human label: Wave name, Project slug, or Task identifier.
    pub subject: String,
    pub fact: WorkActivityFact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkActivityFact {
    WorkCreated,
    RunStarted {
        invocation_id: String,
        trace_id: String,
        exec_id: String,
    },
    RunFinished {
        invocation_id: String,
        trace_id: String,
        exec_id: String,
        status: String,
    },
    PrStarted {
        id: TaskPrId,
    },
    PrPublishRequested {
        id: TaskPrId,
        github: Option<GithubPr>,
    },
    PrMergeRequested {
        id: TaskPrId,
        request: PrMergeRequestSnapshot,
        github: Option<GithubPr>,
    },
    PrMerged {
        id: TaskPrId,
        github: Option<GithubPr>,
        merge_commit: String,
    },
    PrAbandoned {
        id: TaskPrId,
        github: Option<GithubPr>,
    },
    SteerIssued {
        id: SteerId,
        author: Author,
    },
}

#[derive(Debug)]
struct WorkCatalog {
    owners: HashMap<WorkRef, WorkOwner>,
}

#[derive(Debug, Clone)]
struct WorkOwner {
    work: WorkRef,
    subject: String,
    wave: String,
    project: Option<String>,
    task: Option<String>,
    created_at: Option<i64>,
}

pub fn run(
    since_value: &str,
    limit: usize,
    wave: Option<&str>,
    project: Option<&str>,
    task: Option<&str>,
    json: bool,
) -> Result<()> {
    let generated_at = OffsetDateTime::now_utc();
    let since = parse_since(since_value, generated_at)?.unix_timestamp();
    if limit == 0 || limit > MAX_LIMIT {
        return Err(anyhow!(
            "--limit must be between 1 and {MAX_LIMIT}; got {}",
            limit
        ));
    }

    let filter = WorkFilter {
        wave,
        project,
        task,
    };
    let path = crate::store::database_path_from_env()?;
    let snapshot = if path.exists() {
        let store = SqliteStore::new(&path)?;
        build_snapshot(&store, generated_at.unix_timestamp(), since, limit, filter)?
    } else {
        WorkActivitySnapshot {
            generated_at: generated_at.unix_timestamp(),
            since,
            limit,
            truncated: false,
            items: Vec::new(),
        }
    };

    if json {
        println!("{}", serde_json::to_string(&snapshot)?);
    } else {
        print_snapshot(&snapshot);
    }
    Ok(())
}

fn build_snapshot(
    store: &SqliteStore,
    generated_at: i64,
    since: i64,
    limit: usize,
    filter: WorkFilter<'_>,
) -> Result<WorkActivitySnapshot> {
    let waves = store
        .list_waves(None)
        .map_err(|error| anyhow!("failed to read Waves: {error}"))?;
    let projects = store
        .list_projects(None)
        .map_err(|error| anyhow!("failed to read Projects: {error}"))?;
    let tasks = store
        .list_tasks(None)
        .map_err(|error| anyhow!("failed to read Tasks: {error}"))?;
    let catalog = WorkCatalog::new(&waves, &projects, &tasks)?;

    let mut entries = catalog.creation_entries(since);
    for task in &tasks {
        let work = catalog
            .owners
            .get(&WorkRef::Task(task.id.clone()))
            .ok_or_else(|| anyhow!("Task {} is missing from the Work catalog", task.id))?;
        for pr in store
            .task_prs(&task.id)
            .map_err(|error| anyhow!("failed to read PRs for {}: {error}", task.plan.identifier))?
        {
            entries.extend(pr_entries(&pr, work, since));
        }
    }

    let runs = collect_run_activity_since(store, filter, since)?;
    for run in runs {
        if let Some(work) = catalog.resolve_run(&run) {
            entries.extend(run_entries(&run, work, since));
        }
    }

    let steers = store
        .list_steers_since(since)
        .map_err(|error| anyhow!("failed to read Steers: {error}"))?;
    for steer in steers {
        if let Some(work) = catalog.owners.get(&steer.work) {
            entries.push(steer_entry(&steer, work));
        }
    }

    Ok(finalize_snapshot(
        entries,
        generated_at,
        since,
        limit,
        &catalog,
        filter,
    ))
}

impl WorkCatalog {
    fn new(waves: &[Wave], projects: &[Project], tasks: &[Task]) -> Result<Self> {
        let mut catalog = Self {
            owners: HashMap::new(),
        };
        for wave in waves {
            let work = WorkRef::Wave(wave.id().clone());
            catalog.owners.insert(
                work.clone(),
                WorkOwner {
                    work,
                    subject: wave.name().to_string(),
                    wave: wave.name().to_string(),
                    project: None,
                    task: None,
                    created_at: wave.created_at().map(OffsetDateTime::unix_timestamp),
                },
            );
        }
        for project in projects {
            let wave = catalog
                .owners
                .get(&WorkRef::Wave(project.wave_id.clone()))
                .ok_or_else(|| {
                    anyhow!(
                        "Project {} refers to missing Wave {}",
                        project.plan.slug,
                        project.wave_id
                    )
                })?
                .wave
                .clone();
            let work = WorkRef::Project(project.id.clone());
            catalog.owners.insert(
                work.clone(),
                WorkOwner {
                    work,
                    subject: project.plan.slug.clone(),
                    wave,
                    project: Some(project.plan.slug.clone()),
                    task: None,
                    created_at: Some(project.created_at.unix_timestamp()),
                },
            );
        }
        for task in tasks {
            let project = catalog
                .owners
                .get(&WorkRef::Project(task.project_id.clone()))
                .ok_or_else(|| {
                    anyhow!(
                        "Task {} refers to missing Project {}",
                        task.plan.identifier,
                        task.project_id
                    )
                })?;
            let work = WorkRef::Task(task.id.clone());
            catalog.owners.insert(
                work.clone(),
                WorkOwner {
                    work,
                    subject: task.plan.identifier.clone(),
                    wave: project.wave.clone(),
                    project: project.project.clone(),
                    task: Some(task.plan.identifier.clone()),
                    created_at: Some(task.created_at.unix_timestamp()),
                },
            );
        }
        Ok(catalog)
    }

    fn resolve_run(&self, run: &SkillRunEntry) -> Option<&WorkOwner> {
        if let Some(task) = run.task.as_deref() {
            return unique_match(self.owners.values().filter(|owner| {
                owner.task.as_deref() == Some(task)
                    && run
                        .project
                        .as_deref()
                        .is_none_or(|value| owner.project.as_deref() == Some(value))
                    && run.wave.as_deref().is_none_or(|value| value == owner.wave)
            }));
        }
        if let Some(project) = run.project.as_deref() {
            return unique_match(self.owners.values().filter(|owner| {
                owner.task.is_none()
                    && owner.project.as_deref() == Some(project)
                    && run.wave.as_deref().is_none_or(|value| value == owner.wave)
            }));
        }
        let wave = run.wave.as_deref()?;
        unique_match(
            self.owners
                .values()
                .filter(|owner| owner.project.is_none() && owner.wave == wave),
        )
    }

    fn creation_entries(&self, since: i64) -> Vec<WorkActivityEntry> {
        self.owners
            .values()
            .filter_map(|owner| {
                let recorded_at = owner.created_at.filter(|value| *value >= since)?;
                let kind = match &owner.work {
                    WorkRef::Wave(_) => "Wave",
                    WorkRef::Project(_) => "Project",
                    WorkRef::Task(_) => "Task",
                };
                Some(activity_entry(
                    format!("{}:{}:created", owner.work.kind(), owner.work.id()),
                    recorded_at,
                    format!("{kind} {} created", owner.subject),
                    owner,
                    WorkActivityFact::WorkCreated,
                ))
            })
            .collect()
    }
}

fn unique_match<'a>(mut matches: impl Iterator<Item = &'a WorkOwner>) -> Option<&'a WorkOwner> {
    let value = matches.next()?;
    matches.next().is_none().then_some(value)
}

fn activity_entry(
    id: String,
    recorded_at: i64,
    summary: String,
    owner: &WorkOwner,
    fact: WorkActivityFact,
) -> WorkActivityEntry {
    WorkActivityEntry {
        id,
        recorded_at,
        summary,
        work: owner.work.clone(),
        subject: owner.subject.clone(),
        fact,
    }
}

fn run_entries(run: &SkillRunEntry, work: &WorkOwner, since: i64) -> Vec<WorkActivityEntry> {
    let label = run
        .flow
        .as_deref()
        .filter(|flow| *flow != run.skill)
        .map(|flow| format!("{flow}/{}", run.skill))
        .unwrap_or_else(|| run.skill.clone());
    let mut entries = Vec::new();
    if run.started >= since {
        entries.push(activity_entry(
            format!("run:{}:started", run.id),
            run.started,
            format!("{label} started"),
            work,
            WorkActivityFact::RunStarted {
                invocation_id: run.id.clone(),
                trace_id: run.trace_id.clone(),
                exec_id: run.exec_id.clone(),
            },
        ));
    }
    if let Some(ended) = run.ended.filter(|ended| *ended >= since) {
        entries.push(activity_entry(
            format!("run:{}:finished", run.id),
            ended,
            format!("{label} finished {}", run.status),
            work,
            WorkActivityFact::RunFinished {
                invocation_id: run.id.clone(),
                trace_id: run.trace_id.clone(),
                exec_id: run.exec_id.clone(),
                status: run.status.clone(),
            },
        ));
    }
    entries
}

fn pr_entries(pr: &TaskPr, work: &WorkOwner, since: i64) -> Vec<WorkActivityEntry> {
    let github = pr.github();
    let reference = github
        .map(|github| format!("PR #{}", github.number))
        .unwrap_or_else(|| format!("PR {}", pr.slug));
    let mut entries = Vec::new();
    if pr.created_at.unix_timestamp() >= since {
        entries.push(activity_entry(
            format!("pr:{}:started", pr.id),
            pr.created_at.unix_timestamp(),
            format!("PR work started on {}", pr.branch),
            work,
            WorkActivityFact::PrStarted { id: pr.id.clone() },
        ));
    }
    if let Some(publication) = pr
        .publication
        .as_ref()
        .filter(|publication| publication.requested_at.unix_timestamp() >= since)
    {
        entries.push(activity_entry(
            format!("pr:{}:publish_requested", pr.id),
            publication.requested_at.unix_timestamp(),
            format!("Publication requested for {reference}"),
            work,
            WorkActivityFact::PrPublishRequested {
                id: pr.id.clone(),
                github: publication.github.clone(),
            },
        ));
    }
    if let Some(request) = pr
        .publication
        .as_ref()
        .and_then(|publication| publication.merge.as_ref())
        .filter(|request| request.requested_at.unix_timestamp() >= since)
    {
        entries.push(activity_entry(
            format!("pr:{}:merge_requested", pr.id),
            request.requested_at.unix_timestamp(),
            format!("Merge requested for {reference}"),
            work,
            WorkActivityFact::PrMergeRequested {
                id: pr.id.clone(),
                request: PrMergeRequestSnapshot::from(request),
                github: github.cloned(),
            },
        ));
    }
    if let Some(merge_commit) = pr
        .merge_commit
        .as_ref()
        .filter(|_| pr.updated_at.unix_timestamp() >= since)
    {
        entries.push(activity_entry(
            format!("pr:{}:merged", pr.id),
            pr.updated_at.unix_timestamp(),
            format!("{reference} merged"),
            work,
            WorkActivityFact::PrMerged {
                id: pr.id.clone(),
                github: github.cloned(),
                merge_commit: merge_commit.clone(),
            },
        ));
    }
    if let Some(abandoned_at) = pr
        .abandoned_at
        .filter(|abandoned_at| abandoned_at.unix_timestamp() >= since)
    {
        entries.push(activity_entry(
            format!("pr:{}:abandoned", pr.id),
            abandoned_at.unix_timestamp(),
            format!("{reference} abandoned"),
            work,
            WorkActivityFact::PrAbandoned {
                id: pr.id.clone(),
                github: github.cloned(),
            },
        ));
    }
    entries
}

fn steer_entry(steer: &Steer, work: &WorkOwner) -> WorkActivityEntry {
    activity_entry(
        format!("steer:{}", steer.id),
        steer.issued_at.unix_timestamp(),
        format!("Steered: {}", steer.text),
        work,
        WorkActivityFact::SteerIssued {
            id: steer.id.clone(),
            author: steer.author.clone(),
        },
    )
}

fn finalize_snapshot(
    mut entries: Vec<WorkActivityEntry>,
    generated_at: i64,
    since: i64,
    limit: usize,
    catalog: &WorkCatalog,
    filter: WorkFilter<'_>,
) -> WorkActivitySnapshot {
    entries.retain(|entry| {
        catalog.owners.get(&entry.work).is_some_and(|owner| {
            filter.matches(
                Some(&owner.wave),
                owner.project.as_deref(),
                owner.task.as_deref(),
            )
        })
    });
    entries.sort_by(|left, right| {
        right
            .recorded_at
            .cmp(&left.recorded_at)
            .then_with(|| right.id.cmp(&left.id))
    });
    let truncated = entries.len() > limit;
    entries.truncate(limit);
    WorkActivitySnapshot {
        generated_at,
        since,
        limit,
        truncated,
        items: entries,
    }
}

fn print_snapshot(snapshot: &WorkActivitySnapshot) {
    if snapshot.items.is_empty() {
        println!("No durable Work activity recorded in this window.");
        return;
    }
    println!("TIME              WORK             ACTIVITY");
    for item in &snapshot.items {
        let time = OffsetDateTime::from_unix_timestamp(item.recorded_at)
            .map(|value| {
                format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}",
                    value.year(),
                    u8::from(value.month()),
                    value.day(),
                    value.hour(),
                    value.minute()
                )
            })
            .unwrap_or_else(|_| item.recorded_at.to_string());
        println!(
            "{time:<17} {work:<16} {summary}",
            work = item.subject,
            summary = item.summary
        );
    }
    if snapshot.truncated {
        println!("… more activity exists; increase --limit to inspect it.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{
        AfterMerge, GithubPr, PrMergeMode, PrMergeRequest, PrPresentation, PrPublication, TaskId,
        TaskPrId,
    };

    #[test]
    fn work_activity_fixture_round_trips() {
        let fixture = include_str!("../../../../../tests/fixtures/dto/work_activity_snapshot.json");
        let snapshot = serde_json::from_str::<WorkActivitySnapshot>(fixture).unwrap();

        assert_eq!(snapshot.limit, 50);
        assert_eq!(snapshot.items.len(), 5);
        assert!(matches!(
            snapshot.items[0].fact,
            WorkActivityFact::RunFinished { ref status, .. } if status == "ok"
        ));
        assert!(matches!(
            snapshot.items[1].fact,
            WorkActivityFact::PrMerged { ref github, .. }
                if github.as_ref().map(|pr| pr.number) == Some(1144)
        ));
        assert!(matches!(
            snapshot.items[2].fact,
            WorkActivityFact::PrMergeRequested { ref request, .. }
                if request.requested_at == "2026-07-21T18:35:51Z"
        ));
        assert_eq!(
            serde_json::from_str::<WorkActivitySnapshot>(
                &serde_json::to_string(&snapshot).unwrap()
            )
            .unwrap(),
            snapshot
        );
    }

    fn task_owner(identifier: &str, project: &str, wave: &str) -> WorkOwner {
        WorkOwner {
            work: WorkRef::Task(TaskId::new()),
            subject: identifier.to_string(),
            wave: wave.to_string(),
            project: Some(project.to_string()),
            task: Some(identifier.to_string()),
            created_at: None,
        }
    }

    fn catalog(owners: &[WorkOwner]) -> WorkCatalog {
        WorkCatalog {
            owners: owners
                .iter()
                .map(|owner| (owner.work.clone(), owner.clone()))
                .collect(),
        }
    }

    fn entry(id: &str, recorded_at: i64, owner: &WorkOwner) -> WorkActivityEntry {
        activity_entry(
            id.to_string(),
            recorded_at,
            id.to_string(),
            owner,
            WorkActivityFact::WorkCreated,
        )
    }

    #[test]
    fn filters_apply_before_ordering_and_cap() {
        let other = task_owner("W2-2", "other", "live");
        let target = task_owner("W2-1", "control", "live");
        let catalog = catalog(&[other.clone(), target.clone()]);
        let entries = vec![
            entry("other-new", 30, &other),
            entry("target-old", 10, &target),
            entry("target-new", 20, &target),
        ];
        let snapshot = finalize_snapshot(
            entries,
            40,
            0,
            1,
            &catalog,
            WorkFilter {
                wave: Some("live"),
                project: Some("control"),
                task: Some("W2-1"),
            },
        );

        assert!(snapshot.truncated);
        assert_eq!(snapshot.items[0].id, "target-new");
    }

    #[test]
    fn ordering_has_a_stable_tie_breaker() {
        let owner = task_owner("W2-1", "control", "live");
        let catalog = catalog(std::slice::from_ref(&owner));
        let snapshot = finalize_snapshot(
            vec![entry("a", 10, &owner), entry("b", 10, &owner)],
            20,
            0,
            50,
            &catalog,
            WorkFilter::default(),
        );
        assert_eq!(
            snapshot
                .items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["b", "a"]
        );
    }

    #[test]
    fn conflicting_filters_return_no_activity() {
        let owner = task_owner("W2-1", "control", "live");
        let catalog = catalog(std::slice::from_ref(&owner));
        let snapshot = finalize_snapshot(
            vec![entry("target", 10, &owner)],
            20,
            0,
            50,
            &catalog,
            WorkFilter {
                wave: Some("another-wave"),
                project: Some("control"),
                task: Some("W2-1"),
            },
        );
        assert!(snapshot.items.is_empty());
        assert!(!snapshot.truncated);
    }

    #[test]
    fn run_start_and_finish_are_distinct_traceable_claims() {
        let run = SkillRunEntry {
            id: "invocation-1".to_string(),
            trace_id: "trace-1".to_string(),
            exec_id: "exec-1".to_string(),
            parent_exec_id: None,
            repo: "/repo".to_string(),
            worktree: "/repo.task".to_string(),
            wave: Some("live".to_string()),
            project: Some("control".to_string()),
            task: Some("W2-1".to_string()),
            flow: Some("task_pursue".to_string()),
            skill: "implement".to_string(),
            status: "complete".to_string(),
            started: 10,
            ended: Some(20),
            turns: 1,
            system_tokens: 0,
            task_tokens: 0,
            supplied_context_tokens: 0,
            input_tokens: None,
            output_tokens: None,
            reasoning_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            cost_usd: None,
            duration_secs: Some(10.0),
            provider: "codex".to_string(),
            model: None,
            surface: "cli".to_string(),
            capture_status: "complete".to_string(),
        };

        let entries = run_entries(&run, &task_owner("W2-1", "control", "live"), 0);
        assert!(matches!(
            entries[0].fact,
            WorkActivityFact::RunStarted { .. }
        ));
        assert!(matches!(
            &entries[1].fact,
            WorkActivityFact::RunFinished {
                invocation_id,
                trace_id,
                exec_id,
                status,
            } if invocation_id == "invocation-1"
                && trace_id == "trace-1"
                && exec_id == "exec-1"
                && status == "complete"
        ));
    }

    #[test]
    fn wire_reuses_work_reference_and_one_fact_tag() {
        let owner = task_owner("W2-1", "control", "live");
        let value = serde_json::to_value(entry("created", 10, &owner)).unwrap();

        assert_eq!(value["work"]["kind"], "task");
        assert_eq!(value["work"]["id"], owner.work.id());
        assert_eq!(value["subject"], "W2-1");
        assert_eq!(value["fact"]["kind"], "work_created");
        assert!(value.get("evidence").is_none());
        assert!(value.get("kind").is_none());
    }

    #[test]
    fn serial_pr_stages_preserve_their_distinct_evidence() {
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_id: TaskId::new(),
            sequence: 2,
            slug: "activity-query".to_string(),
            branch: "jack/activity-query".to_string(),
            base_commit: "base".to_string(),
            parent_pr_id: None,
            publication: Some(PrPublication {
                requested_at: OffsetDateTime::from_unix_timestamp(20).unwrap(),
                presentation: Some(PrPresentation {
                    title: "Activity query".to_string(),
                    body: "Reviewer context".to_string(),
                    head_sha: "head".to_string(),
                }),
                github: Some(GithubPr {
                    number: 140,
                    url: "https://github.com/loopflowstudio/loopflow/pull/140".to_string(),
                    head_sha: Some("head".to_string()),
                }),
                merge: Some(PrMergeRequest {
                    mode: PrMergeMode::Auto,
                    requested_at: OffsetDateTime::from_unix_timestamp(30).unwrap(),
                    head_sha: "head".to_string(),
                    after_merge: AfterMerge::ContinueTask,
                    next_slug: None,
                }),
            }),
            merge_commit: Some("merge".to_string()),
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: OffsetDateTime::from_unix_timestamp(10).unwrap(),
            updated_at: OffsetDateTime::from_unix_timestamp(40).unwrap(),
        };

        let entries = pr_entries(&pr, &task_owner("W2-1", "control", "live"), 0);

        assert!(matches!(
            entries[0].fact,
            WorkActivityFact::PrStarted { .. }
        ));
        assert!(matches!(
            entries[1].fact,
            WorkActivityFact::PrPublishRequested { .. }
        ));
        assert!(matches!(
            entries[2].fact,
            WorkActivityFact::PrMergeRequested { .. }
        ));
        let merge_request = serde_json::to_value(&entries[2]).unwrap();
        assert_eq!(
            merge_request["fact"]["request"]["requested_at"],
            "1970-01-01T00:00:30Z"
        );
        assert!(matches!(
            &entries[3].fact,
            WorkActivityFact::PrMerged {
                github: Some(github),
                merge_commit,
                ..
            } if github.url.ends_with("/140") && merge_commit == "merge"
        ));
        assert!(entries
            .iter()
            .all(|entry| entry.id.contains(pr.id.as_str())));
    }
}
