//! `lf history` — what Sessions did, each record tagged with the Work it was
//! attributed to. Work changes no Session made live on the Task itself.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::durable::WorkRef;
use crate::lf::commands::util::parse_since;
use crate::lf::commands::work_catalog::{WorkCatalog, WorkOwner};
use crate::lf::commands::WorkFilter;
use crate::session_record::SessionHistory;
use crate::store::rows::unix_to_datetime;
use crate::store::sqlite::SqliteStore;

const MAX_LIMIT: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkActivitySnapshot {
    /// More Session records exist in the window than `--limit` returned.
    pub truncated: bool,
    pub items: Vec<WorkActivityEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkActivityEntry {
    pub id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub recorded_at: OffsetDateTime,
    pub summary: String,
    pub work: WorkRef,
    /// Current display label: Wave name, Project slug, or Task identifier.
    pub subject: String,
    pub fact: WorkActivityFact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkActivityFact {
    InputCaptured {
        session_id: crate::id::LfSessionId,
        captured: i64,
    },
    InputCompletionRecorded {
        session_id: crate::id::LfSessionId,
        captured: i64,
        status: String,
    },
    ProviderHistoryRecorded {
        session_id: crate::id::LfSessionId,
        captured: Option<i64>,
        reference: crate::session_record::ProviderHistoryReference,
        lf_process_id: Option<crate::id::LfProcessId>,
        status: Option<String>,
    },
}

/// What `lf history` reads; the flags of the same name, which a `--watch`
/// request can replace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// `lf history --watch`. History is committed facts only; the clock just ages
/// the window.
pub(crate) fn watch(
    window: HistoryWindow,
    transport: super::watch::Transport,
) -> Result<Option<super::watch::Subscription>> {
    super::watch::start(
        window,
        super::watch::Watched::every(super::watch::PLANNING_CLOCK),
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
    if options.watch {
        return watch(window, super::watch::Transport::Stdio).map(drop);
    }
    let path = crate::store::database_path_from_env()?;
    let store = path.exists().then(|| SqliteStore::new(&path)).transpose()?;
    let snapshot = read(store.as_ref(), &window)?;
    if options.json {
        println!("{}", serde_json::to_string(&snapshot)?);
    } else {
        print_snapshot(&snapshot);
    }
    Ok(())
}

fn read(store: Option<&SqliteStore>, window: &HistoryWindow) -> Result<WorkActivitySnapshot> {
    let limit = window.limit;
    let since = parse_since(&window.since, OffsetDateTime::now_utc())?.unix_timestamp();
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

pub(crate) fn build_snapshot(
    store: &SqliteStore,
    since: i64,
    limit: usize,
    filter: WorkFilter<'_>,
) -> Result<WorkActivitySnapshot> {
    let catalog = WorkCatalog::new(store.work_identities()?)?;

    let mut entries = Vec::new();

    let history =
        store.conversation_history(filter.wave, filter.project, filter.task, None, since, true)?;
    for snapshot in history {
        let work = snapshot
            .task_id
            .clone()
            .map(WorkRef::Task)
            .or_else(|| snapshot.wave_id.clone().map(WorkRef::Wave));
        if let Some(work) = work.and_then(|work| catalog.owners.get(&work)) {
            entries.extend(input_entries(&snapshot, work, since));
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
                entries.push(activity_entry(
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
    }

    Ok(finalize_snapshot(entries, limit, &catalog, filter))
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
    use crate::work::task::TaskId;

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
