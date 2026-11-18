//! `lf history` — what Sessions did, each record tagged with the Work it was
//! attributed to. Work changes no Session made live on the Task itself.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lf::commands::work_catalog::{WorkCatalog, WorkOwner};
use crate::lf::commands::WorkFilter;
use loopflow_execution::output::parse_since;
use loopflow_execution::session_record::SessionHistory;
use loopflow_model::durable::WorkRef;
use loopflow_storage::rows::unix_to_datetime;
use loopflow_storage::sqlite::SqliteStore;

const MAX_LIMIT: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkActivitySnapshot {
    /// More Session records exist in the window than `--limit` returned.
    pub truncated: bool,
    pub items: Vec<WorkActivityEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkActivityEntry {
    pub id: String,
    #[schemars(with = "String")]
    #[serde(with = "time::serde::rfc3339")]
    pub recorded_at: OffsetDateTime,
    pub summary: String,
    pub work: WorkRef,
    /// Current display label: Wave name, Project slug, or Task identifier.
    pub subject: String,
    pub fact: WorkActivityFact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkActivityFact {
    InputCaptured {
        session_id: loopflow_model::id::LfSessionId,
        captured: i64,
    },
    InputCompletionRecorded {
        session_id: loopflow_model::id::LfSessionId,
        captured: i64,
        status: String,
    },
    ProviderHistoryRecorded {
        session_id: loopflow_model::id::LfSessionId,
        captured: Option<i64>,
        reference: loopflow_execution::session_record::ProviderHistoryReference,
        lf_process_id: Option<loopflow_model::id::LfProcessId>,
        status: Option<String>,
    },
}

/// What `lf history` reads; the flags of the same name, which a `--subscribe`
/// request can replace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryWindow {
    pub since: String,
    pub limit: usize,
    pub wave: Option<String>,
    pub project: Option<String>,
    pub task: Option<String>,
}

impl HistoryWindow {
    /// The `lf history` flags that read this window.
    pub(crate) fn argv(&self) -> Vec<String> {
        let mut argv = vec![
            "session".into(),
            "history".into(),
            "--since".into(),
            self.since.clone(),
            "--limit".into(),
            self.limit.to_string(),
        ];
        for (flag, value) in [
            ("--wave", &self.wave),
            ("--project", &self.project),
            ("--task", &self.task),
        ] {
            if let Some(value) = value {
                argv.extend([flag.into(), value.clone()]);
            }
        }
        argv
    }
}

/// `lf history --subscribe`. History is committed facts only; the clock just ages
/// the window.
pub(crate) fn subscribe(
    window: HistoryWindow,
    transport: loopflow_execution::read_stream::Transport,
) -> Result<Option<loopflow_execution::read_stream::Subscription>> {
    loopflow_execution::read_stream::start(
        loopflow_execution::read_stream::Watch::History,
        window,
        |store, window| async move { read(store.as_ref().map(|store| &store.sqlite), &window) },
        transport,
    )
}

pub fn run(options: &super::history::HistoryFeed) -> Result<()> {
    let window = HistoryWindow {
        since: options.since.clone(),
        limit: options.limit,
        wave: options.wave.clone(),
        project: options.project.clone(),
        task: options.task.clone(),
    };
    if options.subscribe {
        return subscribe(window, loopflow_execution::read_stream::Transport::Stdio).map(drop);
    }
    let snapshot = snapshot(&window)?;
    if options.json {
        println!(
            "{}",
            serde_json::to_string(&loopflow_execution::read_stream::selected_answer(
                &snapshot
            )?)?
        );
    } else {
        print_snapshot(&snapshot);
    }
    Ok(())
}

/// What `lf history --json` prints for `window`.
pub(crate) fn snapshot(window: &HistoryWindow) -> Result<WorkActivitySnapshot> {
    let path = loopflow_execution::store::database_path_from_env()?;
    let store = path
        .exists()
        .then(|| loopflow_execution::store::open_sqlite(&path))
        .transpose()?;
    read(store.as_ref(), window)
}

fn read(store: Option<&SqliteStore>, window: &HistoryWindow) -> Result<WorkActivitySnapshot> {
    let limit = window.limit;
    let since = parse_since(&window.since, loopflow_execution::observe::now())?.unix_timestamp();
    if limit == 0 || limit > MAX_LIMIT {
        return Err(anyhow!(
            "--limit must be between 1 and {MAX_LIMIT}; got {}",
            limit
        ));
    }
    let filter = WorkFilter {
        wave: window.wave.as_deref(),
        project: window.project.as_deref(),
        task: window.task.as_deref(),
    };
    match store {
        Some(store) => build_snapshot(store, since, limit, filter),
        None => Ok(WorkActivitySnapshot {
            truncated: false,
            items: Vec::new(),
        }),
    }
}

/// The `limit` newest entries since `since`. Inputs decode newest bound
/// first, and reading stops once `limit + 1` entries are newer than every
/// input not yet read: nothing left could enter the result or untruncate it.
pub(crate) fn build_snapshot(
    store: &SqliteStore,
    since: i64,
    limit: usize,
    filter: WorkFilter<'_>,
) -> Result<WorkActivitySnapshot> {
    let catalog = WorkCatalog::new(store.work_identities()?)?;
    let mut pending = store.pending_conversation_history(
        filter.wave,
        filter.project,
        filter.task,
        since,
        true,
    )?;
    pending
        .inputs
        .sort_by_key(|input| std::cmp::Reverse(input.latest));

    let mut entries = Vec::new();
    for input in &pending.inputs {
        if settled(&entries, limit, input.latest) {
            break;
        }
        let snapshot =
            loopflow_execution::store::sessions::project_history_input(store, &pending, input)?;
        let mut found = Vec::new();
        let work = snapshot
            .task_id
            .clone()
            .map(WorkRef::Task)
            .or_else(|| snapshot.wave_id.clone().map(WorkRef::Wave));
        if let Some(work) = work.and_then(|work| catalog.owners.get(&work)) {
            found.extend(input_entries(&snapshot, work, since));
        }
        for provider in &snapshot.providers {
            let work = provider
                .task_id
                .clone()
                .map(WorkRef::Task)
                .or_else(|| provider.wave_id.clone().map(WorkRef::Wave));
            let Some(owner) = work.and_then(|work| catalog.owners.get(&work)) else {
                continue;
            };
            for (phase, at) in [
                ("started", provider.started_at),
                ("completed", provider.completed_at),
            ] {
                let Some(at) = at.filter(|at| *at >= since) else {
                    continue;
                };
                found.push(activity_entry(
                    format!(
                        "session:{}:{}:{phase}",
                        snapshot.session_id,
                        serde_json::to_string(&provider.reference)?
                    ),
                    at,
                    format!("{} provider {phase}", snapshot.label()),
                    owner,
                    WorkActivityFact::ProviderHistoryRecorded {
                        session_id: snapshot.session_id.clone(),
                        captured: snapshot.captured,
                        reference: provider.reference.clone(),
                        lf_process_id: provider.lf_process_id.clone(),
                        status: if phase == "completed" {
                            provider.outcome.clone()
                        } else {
                            None
                        },
                    },
                ));
            }
        }
        entries.extend(found.into_iter().filter(|entry| {
            catalog
                .owners
                .get(&entry.work)
                .is_some_and(|owner| owner.matches(filter))
        }));
    }

    Ok(finalize_snapshot(entries, limit, &catalog, filter))
}

/// Whether `limit + 1` entries are newer than `bound`, the latest time any
/// unread input can report.
fn settled(entries: &[WorkActivityEntry], limit: usize, bound: i64) -> bool {
    if entries.len() <= limit {
        return false;
    }
    let mut times: Vec<i64> = entries
        .iter()
        .map(|entry| entry.recorded_at.unix_timestamp())
        .collect();
    let (_, kth, _) = times.select_nth_unstable_by(limit, |left, right| right.cmp(left));
    *kth > bound
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
        recorded_at: unix_to_datetime(recorded_at),
        summary,
        work: owner.work.clone(),
        subject: owner.subject.clone(),
        fact,
    }
}

fn input_entries(input: &SessionHistory, work: &WorkOwner, since: i64) -> Vec<WorkActivityEntry> {
    let mut entries = Vec::new();
    let Some(captured) = input.captured else {
        return entries;
    };
    if input.observed_at >= since {
        entries.push(activity_entry(
            format!("session:{}:{captured}:captured", input.session_id),
            input.observed_at,
            format!("{} input captured", input.label()),
            work,
            WorkActivityFact::InputCaptured {
                session_id: input.session_id.clone(),
                captured,
            },
        ));
    }
    if let (Some(outcome), Some(at)) = (
        &input.recorded_outcome,
        input.recorded_at.filter(|at| *at >= since),
    ) {
        entries.push(activity_entry(
            format!("session:{}:{captured}:completion", input.session_id),
            at,
            format!("{} recorder reported {outcome}", input.label()),
            work,
            WorkActivityFact::InputCompletionRecorded {
                session_id: input.session_id.clone(),
                captured,
                status: outcome.clone(),
            },
        ));
    }
    entries
}

fn finalize_snapshot(
    mut entries: Vec<WorkActivityEntry>,
    limit: usize,
    catalog: &WorkCatalog,
    filter: WorkFilter<'_>,
) -> WorkActivitySnapshot {
    entries.retain(|entry| {
        catalog
            .owners
            .get(&entry.work)
            .is_some_and(|owner| owner.matches(filter))
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
        truncated,
        items: entries,
    }
}

fn print_snapshot(snapshot: &WorkActivitySnapshot) {
    if snapshot.items.is_empty() {
        println!("No Session activity recorded in this window.");
        return;
    }
    println!("TIME              WORK             ACTIVITY");
    for item in &snapshot.items {
        let value = item.recorded_at;
        let time = format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            value.year(),
            u8::from(value.month()),
            value.day(),
            value.hour(),
            value.minute()
        );
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
    use loopflow_storage::work::task::TaskId;

    #[test]
    fn work_activity_fixture_round_trips() {
        let fixture = include_str!("../../../../../tests/fixtures/dto/work_activity_snapshot.json");
        let snapshot = serde_json::from_str::<WorkActivitySnapshot>(fixture).unwrap();

        assert_eq!(snapshot.items.len(), 3);
        assert!(matches!(
            snapshot.items[0].fact,
            WorkActivityFact::InputCompletionRecorded { ref status, .. } if status == "ok"
        ));
        assert!(matches!(
            snapshot.items[1].fact,
            WorkActivityFact::ProviderHistoryRecorded { ref status, .. }
                if status.as_deref() == Some("completed")
        ));
        assert!(matches!(
            snapshot.items[2].fact,
            WorkActivityFact::InputCaptured { captured: 4, .. }
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
            selectors: vec![
                format!("wave:{wave}"),
                format!("project:{project}"),
                format!("task:{identifier}"),
            ],
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
            WorkActivityFact::InputCaptured {
                session_id: "session".into(),
                captured: 1,
            },
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
    fn settled_activity_does_not_read_invalid_metadata_below_its_bound() {
        let home = tempfile::tempdir().unwrap();
        let store =
            loopflow_execution::store::opening::open_ephemeral(&home.path().join("store.db"))
                .unwrap();
        std::fs::create_dir(home.path().join(".git")).unwrap();
        let repository = loopflow_execution::repository::location::ensure_repository(
            &store,
            home.path().to_str().unwrap(),
        )
        .unwrap();
        let task = store
            .create_task(&loopflow_execution::work::task::NewTask {
                repository_id: repository,
                id: loopflow_model::durable::TaskId::new(),
                project_id: None,
                title: "History".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        for at in [10, 20, 30] {
            let input = loopflow_execution::session_record::new_artifact_key();
            let template = store.test_session(
                &format!("seed-{at}"),
                &loopflow_execution::session_record::new_artifact_key(),
            );
            loopflow_execution::store::sessions::create_session(
                &store,
                loopflow_execution::session::LfSession {
                    id: format!("input-{at}").into(),
                    captured: None,
                    artifact_key: Some(input),
                    task_id: Some(task.id.clone()),
                    created_at: at,
                    ..template
                },
                None,
            )
            .unwrap();
        }
        // Selection carries this value without decoding it; only an evidence read fails.
        store.conn.lock().unwrap().execute_batch(
            "PRAGMA ignore_check_constraints=ON; UPDATE agent_sessions SET work_source='invalid-source' WHERE id='input-10'; PRAGMA ignore_check_constraints=OFF"
        ).unwrap();
        let snapshot = super::build_snapshot(&store, 0, 1, WorkFilter::default()).unwrap();
        assert!(snapshot.truncated);
        assert_eq!(snapshot.items.len(), 1);
        assert!(snapshot.items[0].id.contains("input-30"));
        assert!(loopflow_execution::store::sessions::input_history(&store, "input-10").is_err());
    }

    #[test]
    fn reading_settles_once_enough_entries_follow_every_unread_bound() {
        let owner = task_owner("W2-1", "control", "live");
        let entries = [entry("a", 30, &owner), entry("b", 20, &owner)];
        assert!(
            !settled(&entries[..1], 1, 10),
            "truncation is still unknown"
        );
        assert!(settled(&entries, 1, 19));
        assert!(
            !settled(&entries, 1, 20),
            "an unread entry could tie and win"
        );
    }

    #[test]
    fn conflicting_filters_return_no_activity() {
        let owner = task_owner("W2-1", "control", "live");
        let catalog = catalog(std::slice::from_ref(&owner));
        let snapshot = finalize_snapshot(
            vec![entry("target", 10, &owner)],
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
    fn input_capture_and_recorded_completion_are_not_provider_success() {
        let input: SessionHistory = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/session_history_summary.json"
        ))
        .unwrap();
        let entries = input_entries(&input, &task_owner("W2-1", "control", "live"), 0);
        assert!(matches!(
            entries[0].fact,
            WorkActivityFact::InputCaptured { .. }
        ));
        assert!(matches!(
            entries[1].fact,
            WorkActivityFact::InputCompletionRecorded { .. }
        ));
        assert_eq!(input.providers.len(), 2);
        assert_eq!(input.providers[0].outcome.as_deref(), Some("failed"));
        assert_eq!(input.providers[1].outcome.as_deref(), Some("completed"));
    }

    #[test]
    fn wire_reuses_work_reference_and_one_fact_tag() {
        let owner = task_owner("W2-1", "control", "live");
        let value = serde_json::to_value(entry("captured", 10, &owner)).unwrap();

        assert_eq!(value["work"]["kind"], "task");
        assert_eq!(value["work"]["id"], owner.work.id());
        assert_eq!(value["subject"], "W2-1");
        assert_eq!(value["fact"]["kind"], "input_captured");
        assert!(value.get("evidence").is_none());
        assert!(value.get("kind").is_none());
    }
}
