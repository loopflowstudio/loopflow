use loopflow::durable::TaskState;
use loopflow::lf::commands::waves::{Evidence, RoadmapSnapshot, WaveDetailSnapshot};
use loopflow::work::wave::metrics::MetricPortfolioDto;

const WAVE_DETAIL: &str = include_str!("../../../tests/fixtures/dto/wave_detail.json");
const ROADMAP: &str = include_str!("../../../tests/fixtures/dto/roadmap_snapshot.json");
const METRIC_PORTFOLIO: &str = include_str!("../../../tests/fixtures/dto/metric_portfolio.json");

#[test]
fn activity_retains_unknown_record_identity_without_a_pid() {
    use loopflow::lf::commands::top::{ActivitySnapshot, ActivityState};
    let snapshot: ActivitySnapshot = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/activity_snapshot.json"
    ))
    .unwrap();
    let unknown = &snapshot.nodes[2];
    assert_eq!(unknown.state, ActivityState::Unknown);
    assert_eq!(unknown.pid, None);
    assert_eq!(unknown.parent_id.as_ref(), Some(&snapshot.nodes[0].id));
    assert_eq!(unknown.id, "33333333-3333-4333-8333-333333333333");
    assert_eq!(
        serde_json::from_str::<ActivitySnapshot>(&serde_json::to_string(&snapshot).unwrap())
            .unwrap(),
        snapshot
    );
}

#[test]
fn task_comments_keep_authorship_and_require_every_field() {
    use loopflow::work::task::comment::TaskCommentAuthor;
    use loopflow::work::task::comment::TaskComments;
    let json = include_str!("../../../tests/fixtures/dto/task_comments.json");
    let thread: TaskComments = serde_json::from_str(json).unwrap();
    assert_eq!(thread.comments[0].author, TaskCommentAuthor::Integration);
    assert_eq!(
        thread.comments[2].author,
        TaskCommentAuthor::Person { name: None }
    );
    // What `lf task comment ISSUE --json` prints is exactly what Swift decodes,
    // including explicit nulls Swift requires to be present.
    assert_eq!(
        serde_json::to_value(&thread).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    let mut missing: serde_json::Value = serde_json::from_str(json).unwrap();
    missing["comments"][0]
        .as_object_mut()
        .unwrap()
        .remove("author");
    assert!(serde_json::from_value::<TaskComments>(missing).is_err());
}

#[test]
fn wave_detail_preserves_graph_and_machine() {
    let snapshot: WaveDetailSnapshot = serde_json::from_str(WAVE_DETAIL).unwrap();
    assert_eq!(
        snapshot.wave.machine.as_ref().unwrap().label.as_deref(),
        Some("mini")
    );
    assert_eq!(
        snapshot.project_readiness.state,
        loopflow::store::sqlite::ProjectReadinessState::Ready
    );
    assert!(snapshot.project_readiness.activation.is_none());
    assert_eq!(snapshot.setup.graph.value.as_deref(), Some("code"));
    assert_eq!(
        snapshot.setup.skill_agents["implement"].source,
        loopflow::work::wave::setup::SetupSource::Wave
    );
    assert_eq!(snapshot.setup.accounts["claude"].accounts, ["work"]);
    let Evidence::Ok {
        items: projects, ..
    } = &snapshot.projects
    else {
        panic!("missing Projects")
    };
    assert_eq!(projects[0].sync.changes[1].field, "task_order");
    let Evidence::Ok { items: tasks, .. } = &snapshot.tasks else {
        panic!("missing Tasks")
    };
    assert_eq!(tasks[0].task.sync.changes[0].field, "creation");
    assert_eq!(tasks[0].task.id, tasks[0].runtime.as_ref().unwrap().work_id);
    assert_eq!(tasks[0].task.linear_id.as_deref(), Some("issue-1"));
    let Evidence::Ok { items: runs, .. } = &snapshot.history else {
        panic!("fixture contains recorded Runs");
    };
    assert_eq!(runs[0].first_provider_attempt_at, Some(1784052010));
    assert_eq!(
        runs[0].task_pr_id.as_ref().map(|id| id.as_str()),
        Some("pr_33333333333333333333333333333333")
    );

    let loopflow::lf::commands::waves::Evidence::Ok { items, .. } = &snapshot.projects else {
        panic!("missing Projects")
    };

    assert!(items[0].current);
    assert_eq!(
        items[0].status,
        loopflow::work::project::ProjectStatus::Started
    );

    let encoded = serde_json::to_string(&snapshot).unwrap();
    let decoded: WaveDetailSnapshot = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        serde_json::to_value(&decoded.projects).unwrap(),
        serde_json::to_value(&snapshot.projects).unwrap()
    );
}

#[test]
fn status_preserves_stranded_tasks_alongside_project_history() {
    let snapshot: WaveDetailSnapshot = serde_json::from_str(WAVE_DETAIL).unwrap();
    assert_eq!(snapshot.unavailable_tasks[0].status, TaskState::Unstarted);
    let Evidence::Ok { items, .. } = snapshot.tasks else {
        panic!("available tasks")
    };
    assert_eq!(items[0].runtime.as_ref().unwrap().reason, "ready");
    let encoded = serde_json::from_str::<serde_json::Value>(WAVE_DETAIL).unwrap();
    assert_eq!(encoded["projects"]["items"][0]["status"], "started");
}

#[test]
fn status_and_roadmap_require_the_shared_metric_portfolio() {
    let detail: WaveDetailSnapshot = serde_json::from_str(WAVE_DETAIL).unwrap();
    assert!(matches!(
        detail.metric_portfolio.metrics.as_slice(),
        [metric] if metric.identity.metric_id == "task-loop-trust"
            && matches!(metric.evidence, loopflow::work::wave::metrics::MetricEvidenceDto::Met { .. })
    ));

    let roadmap: RoadmapSnapshot = serde_json::from_str(ROADMAP).unwrap();
    // Each Task is written once, under the roadmap; Waves do not repeat them.
    assert_eq!(
        serde_json::to_value(&roadmap).unwrap(),
        serde_json::from_str::<serde_json::Value>(ROADMAP).unwrap()
    );
    let Evidence::Ok { items, .. } = roadmap.wave_tasks(&roadmap.waves[0]) else {
        panic!("fixture has current Project evidence")
    };
    assert!(!items.is_empty());
    assert_eq!(
        items[0]
            .reference
            .workspace
            .as_ref()
            .unwrap()
            .machine_label
            .as_deref(),
        Some("build-server")
    );
    assert!(
        serde_json::to_value(&roadmap.waves[0].projects).unwrap()["items"][0]
            .get("graph")
            .is_none()
    );
    assert_eq!(
        items[0].reference.workspace.as_ref().unwrap().local_exists,
        Some(false)
    );
    let Evidence::Ok {
        items: detail_tasks,
        ..
    } = &detail.tasks
    else {
        panic!("fixture has Task evidence")
    };
    assert_eq!(
        detail_tasks[0]
            .reference
            .workspace
            .as_ref()
            .unwrap()
            .local_exists,
        Some(true)
    );
    assert_eq!(
        roadmap.waves[0].metric_portfolio.metrics[0]
            .identity
            .metric_id,
        "task-loop-trust"
    );
    assert!(roadmap.waves[1].metric_portfolio.metrics.is_empty());

    let mut detail_without_metrics: serde_json::Value = serde_json::from_str(WAVE_DETAIL).unwrap();
    detail_without_metrics
        .as_object_mut()
        .unwrap()
        .remove("metric_portfolio");
    let error = serde_json::from_value::<WaveDetailSnapshot>(detail_without_metrics).unwrap_err();
    assert!(error.to_string().contains("metric_portfolio"));

    let mut roadmap_without_metrics: serde_json::Value = serde_json::from_str(ROADMAP).unwrap();
    roadmap_without_metrics["waves"][0]
        .as_object_mut()
        .unwrap()
        .remove("metric_portfolio");
    let error = serde_json::from_value::<RoadmapSnapshot>(roadmap_without_metrics).unwrap_err();
    assert!(error.to_string().contains("metric_portfolio"));
}

#[test]
fn metric_portfolio_fixture_locks_every_tagged_payload() {
    let portfolio: MetricPortfolioDto = serde_json::from_str(METRIC_PORTFOLIO).unwrap();
    assert_eq!(portfolio.metrics.len(), 11);
    assert!(matches!(
        &portfolio.metrics[10].evidence,
        loopflow::work::wave::metrics::MetricEvidenceDto::Unknown {
            cause: loopflow::work::wave::metrics::MetricUnknownCauseDto::TargetUnavailable {
                value: 1.0,
                ..
            }
        }
    ));
    assert_eq!(portfolio.contract_issues.len(), 5);
    assert_eq!(
        portfolio.metrics[0].description,
        "Fraction of qualifying events that settled successfully."
    );

    let value: serde_json::Value = serde_json::from_str(METRIC_PORTFOLIO).unwrap();
    let evidence = value["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|metric| metric["evidence"]["kind"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        evidence,
        [
            "met",
            "missed",
            "unknown",
            "unknown",
            "unknown",
            "unknown",
            "unknown",
            "unknown",
            "unavailable",
            "untargeted",
            "unknown",
        ]
    );

    let mut missing_required = value;
    missing_required["metrics"][0]
        .as_object_mut()
        .unwrap()
        .remove("description");
    let error = serde_json::from_value::<MetricPortfolioDto>(missing_required).unwrap_err();
    assert!(error.to_string().contains("description"));

    let mut with_unknown_field: serde_json::Value = serde_json::from_str(METRIC_PORTFOLIO).unwrap();
    with_unknown_field["metrics"][0]["future_field"] = serde_json::json!(true);
    serde_json::from_value::<MetricPortfolioDto>(with_unknown_field).unwrap();
}

#[test]
fn session_history_retains_receipts_and_unknown_lf_process() {
    let input = include_str!("../../../tests/fixtures/dto/session_history.json");
    let events: Vec<loopflow::session::SessionEvent> = serde_json::from_str(input).unwrap();
    assert_eq!(
        events[1].kind,
        loopflow::session::SessionEventKind::Completed
    );
    assert_eq!(
        events[0].agent_session.as_ref().unwrap().as_str(),
        "thread_fixture"
    );
    assert!(events[1].lf_process_id.is_none());
    assert_eq!(events[0].payload["total"]["inputTokens"], 40);
    assert_eq!(
        events[2].kind,
        loopflow::session::SessionEventKind::Observed
    );
    assert!(events[2].provider_turn.is_none());
    assert!(events[2].agent_session.is_none());
    assert_eq!(
        serde_json::to_value(events).unwrap(),
        serde_json::from_str::<serde_json::Value>(input).unwrap()
    );
}

#[test]
fn task_files_share_exact_bases_rename_paths_and_lossless_revisions() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Files {
        directory: loopflow::ops::task::TaskDirectory,
        changes: loopflow::ops::task::TaskChangesSnapshot,
        diff: loopflow::ops::task::TaskDiffSnapshot,
        file: loopflow::ops::task::TaskFileSnapshot,
        save: loopflow::ops::task::TaskFileSave,
    }
    let json = include_str!("../../../tests/fixtures/dto/task_files.json");
    let files: Files = serde_json::from_str(json).unwrap();
    assert_eq!(files.changes.base_commit, files.diff.base_commit);
    assert_eq!(files.changes.files[0].old_path.as_deref(), Some("old.txt"));
    assert_eq!(files.file.content.as_deref(), Some("\u{feff}notes\r\n"));
    assert_eq!(
        serde_json::to_value(files).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn process_page_retains_outcomes_unknowns_and_continuation() {
    let json = include_str!("../../../tests/fixtures/dto/process_page.json");
    let page: loopflow::process::LfProcessPage = serde_json::from_str(json).unwrap();
    assert_eq!(page.entries[0].exit_code, Some(42));
    assert_eq!(page.entries[0].via_agent, None);
    assert_eq!(page.entries[0].pid, None);
    assert_eq!(page.entries[1].pid, Some(4242));
    assert_eq!(page.entries[1].kind, loopflow::process::ProcessKind::Agent);
    assert_eq!(
        page.entries[1].agent_session_id.as_deref(),
        Some("conversation")
    );
    assert_eq!(page.entries[1].os_started_at, Some(1790640000));
    assert_eq!(
        page.entries[1].parent_lf_process_id.as_ref(),
        Some(&page.entries[0].id)
    );
    assert_eq!(page.entries[1].outcome, None);
    assert_eq!(page.next.as_ref().unwrap().id, page.entries[1].id);
    assert_eq!(
        serde_json::to_value(page).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    assert!(serde_json::from_str::<loopflow::process::LfProcessPage>("{}").is_err());
}

#[test]
fn session_input_history_retains_distinct_native_results_and_unknown_process() {
    let value: loopflow::lf::commands::session_history::SessionHistory = serde_json::from_str(
        include_str!("../../../tests/fixtures/dto/session_history_summary.json"),
    )
    .unwrap();
    assert_eq!(value.providers.len(), 2);
    assert_eq!(value.providers[0].outcome.as_deref(), Some("failed"));
    assert!(value.providers[0].lf_process_id.is_none());
    assert_eq!(value.providers[0].usage.input_tokens, None);
    assert_eq!(value.providers[1].usage.input_tokens, Some(0));
    assert_eq!(value.status(), "failed → completed");
    let encoded = serde_json::to_value(&value).unwrap();
    assert!(encoded.get("subjects").is_none());
    assert!(encoded.get("outcome").is_none());
    assert_eq!(
        serde_json::from_value::<loopflow::lf::commands::session_history::SessionHistory>(encoded)
            .unwrap(),
        value
    );
}

#[test]
fn task_detail_says_each_fact_once() {
    use loopflow::lf::commands::task_row::TaskDetail;
    let json = include_str!("../../../tests/fixtures/dto/task_detail.json");
    let detail: TaskDetail = serde_json::from_str(json).unwrap();
    assert_eq!(detail.task.state, loopflow::durable::TaskState::Active);
    assert_eq!(
        detail.task.graph.as_ref().unwrap().step.as_deref(),
        Some("demo")
    );
    assert_eq!(
        serde_json::to_value(&detail).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn task_status_preserves_canonical_work_and_acquisition_evidence() {
    use loopflow::ops::task::TaskStatus;
    let json = include_str!("../../../tests/fixtures/dto/task_status.json");
    let status: TaskStatus = serde_json::from_str(json).unwrap();
    let task = status.task.as_ref().unwrap();
    let project = status.project.as_ref().unwrap();
    assert_eq!(task.title, "Route incoming mail");
    assert_eq!(task.project_id, Some(project.id.clone()));
    assert_ne!(task.id.as_str(), task.linear_id.as_ref().unwrap().as_str());
    assert_eq!(project.description, "Retained authored prose.");
    assert_eq!(status.observed_at, Some(1790683200));
    assert_eq!(
        status.planning_state,
        loopflow::store::PlanningState::Available
    );
    assert!(!status.planning_stale);
    assert!(status.planning_error.is_none());
    assert!(status.execution.is_none());
    assert_eq!(status.overview.as_ref().unwrap().sessions.len(), 1);
    assert_eq!(status.comments.as_ref().unwrap().comments.len(), 1);
    let ownership = status.ownership.as_ref().unwrap();
    assert_eq!(
        ownership.owner.as_ref().unwrap().to_string(),
        "Jack Heart on mini"
    );
    assert_eq!(
        ownership
            .log
            .iter()
            .map(|entry| entry.kind)
            .collect::<Vec<_>>(),
        [
            loopflow::durable::OwnershipKind::Claim,
            loopflow::durable::OwnershipKind::Handoff
        ]
    );
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_value(status).unwrap(), value);
    for field in ["planning_stale", "graph_processes"] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<TaskStatus>(missing).is_err(),
            "{field}"
        );
    }
}

#[test]
fn graph_compositions_round_trip_distinct_compositions_and_required_children() {
    use loopflow::graph::compiled::GraphCatalogEntry;
    let json = include_str!("../../../tests/fixtures/dto/graph_composition.json");
    let entry: GraphCatalogEntry = serde_json::from_str(json).unwrap();
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_value(&entry).unwrap(), value);
    let mut missing = value;
    missing["composition"]["items"][2]
        .as_object_mut()
        .unwrap()
        .remove("items");
    assert!(serde_json::from_value::<GraphCatalogEntry>(missing).is_err());
    let catalog: Vec<GraphCatalogEntry> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/graph_catalog.json"
    ))
    .unwrap();
    assert!(catalog[0].composition.is_some() && catalog[1].composition.is_none());
}

#[test]
fn prepared_checkout_retains_owning_home_without_starting_execution() {
    let json = include_str!("../../../tests/fixtures/dto/task_checkout.json");
    let snapshot: loopflow::ops::task::TaskSnapshot = serde_json::from_str(json).unwrap();
    assert_eq!(
        snapshot.machine_id.as_ref().unwrap().as_str(),
        "home_00000000000000000000000000000001"
    );
    assert_eq!(
        snapshot.worktree.as_deref(),
        Some("/src/loopflow.workspace")
    );
    assert!(snapshot.pr.is_none());
    assert!(!snapshot.branch.as_ref().unwrap().is_empty());
    assert_eq!(
        snapshot.execution.state,
        loopflow::ops::task_execution::TaskExecutionState::Idle
    );
    assert_eq!(
        serde_json::to_value(&snapshot).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn task_work_preserves_all_owners() {
    let input = include_str!("../../../tests/fixtures/dto/task_work.json");
    let work: loopflow::task_work::TaskWork = serde_json::from_str(input).unwrap();
    assert_eq!(work.sessions.len(), 2);
    assert_eq!(work.graph_processes.len(), 1);
    assert!(!work.processes.is_empty());
    let graph = work.graph.as_ref().expect("fixture Task has a graph");
    assert_eq!(
        graph.position,
        loopflow::task_graph::TaskGraphPosition::Run {
            step: "review-design".into(),
            lf_process_id: work.graph_processes[0].summary.id.to_string(),
            running: true,
        }
    );
    assert_eq!(graph.step(), "review-design");
    assert!(graph.waiting_step().is_none());
    assert_eq!(
        serde_json::to_value(work).unwrap(),
        serde_json::from_str::<serde_json::Value>(input).unwrap()
    );
}

#[test]
fn context_report_keeps_unknown_sources_distinct_from_zero() {
    use loopflow::context_usage::{ContextReport, ContextSource};
    let input = include_str!("../../../tests/fixtures/dto/context_report.json");
    let report: ContextReport = serde_json::from_str(input).unwrap();
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        serde_json::from_str::<serde_json::Value>(input).unwrap()
    );
    let steers = &report.steps[0].sources[4];
    assert_eq!(steers.source, ContextSource::Steers);
    assert_eq!((steers.count, steers.over_budget), (Some(384), false));
    assert!(report.steps[1]
        .sources
        .iter()
        .all(|usage| usage.tokens.is_none()));
    assert_eq!(ContextReport::new(report.steps.clone()), report);
}

#[test]
fn graph_detail_keeps_its_launched_graph_and_every_step() {
    let input = include_str!("../../../tests/fixtures/dto/graph_detail.json");
    let detail: loopflow::durable::GraphProcessDetail = serde_json::from_str(input).unwrap();
    assert_eq!(detail.steps.len(), 6);
    // The running step is the second pass of the node the loop returned to.
    let running = detail.steps.last().unwrap();
    assert_eq!((running.key, running.completed_at), (0, None));
    assert_eq!(detail.current, Some(running.key));
    assert_eq!(
        serde_json::to_value(detail).unwrap(),
        serde_json::from_str::<serde_json::Value>(input).unwrap()
    );
}

#[test]
fn watch_frames_carry_each_reads_json_result() {
    use loopflow::lf::commands::watch::WatchFrame;
    let entries: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/watch_frames.json"
    ))
    .unwrap();
    let envelope = |frame: &serde_json::Value| {
        let mut keys: Vec<_> = frame.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    };
    let written = serde_json::to_value(WatchFrame::<()> {
        answers: None,
        unavailable: Some("gone".into()),
        body: None,
    })
    .unwrap();
    for entry in &entries {
        let frame = &entry["frame"];
        assert_eq!(envelope(frame), envelope(&written), "{}", entry["read"]);
        let body = frame["body"].clone();
        assert_eq!(body.is_null(), frame["unavailable"].is_string());
        if body.is_null() {
            continue;
        }
        fn read<T: serde::de::DeserializeOwned + serde::Serialize>(body: &serde_json::Value) {
            let typed: T = serde_json::from_value(body.clone()).unwrap();
            assert_eq!(&serde_json::to_value(typed).unwrap(), body);
        }
        match entry["part"].as_str().unwrap() {
            "planning" => read::<RoadmapSnapshot>(&body),
            "selections" => read::<Vec<loopflow::work::selection::ViewPlan>>(&body),
            // Desktop's Session rows are pinned by `sessions.json`; the watch carries them as listed.
            "sessions" => assert!(body
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["id"].is_string())),
            "task" => read::<loopflow::lf::commands::waves::DesktopTaskPane>(&body),
            "work_activity" => {
                read::<loopflow::lf::commands::activity::WorkActivitySnapshot>(&body)
            }
            "activity" => read::<loopflow::lf::commands::top::ActivitySnapshot>(&body),
            "peer_planning" => {
                read::<Vec<loopflow::store::PeerPlanningStatus>>(&body["destinations"]);
                assert_eq!(body["transport"], "git");
            }
            other => panic!("unknown watch {other}"),
        }
    }
}

#[test]
fn task_delivery_preserves_execution_and_due_unstarted_tasks() {
    use loopflow::lf::commands::waves::RoadmapTask;
    let json = include_str!("../../../tests/fixtures/dto/task_delivery_rows.json");
    let rows: std::collections::BTreeMap<String, RoadmapTask> = serde_json::from_str(json).unwrap();
    assert!(rows["pending"]
        .runtime
        .as_ref()
        .unwrap()
        .completion_pending
        .as_ref()
        .unwrap()
        .contains("lf task complete"));
    assert_eq!(
        rows["completed_running"].runtime.as_ref().unwrap().status,
        loopflow::durable::TaskState::Done
    );
    assert_eq!(
        rows["completed_running"].execution,
        rows["pending"].execution
    );
    assert!(rows["due"].runtime.is_none());
    assert_eq!(rows["due"].task.due_date.as_deref(), Some("2026-10-08"));
    assert_eq!(
        serde_json::to_value(&rows).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    let mut missing = serde_json::to_value(&rows["pending"]).unwrap();
    missing.as_object_mut().unwrap().remove("run_control");
    assert!(serde_json::from_value::<RoadmapTask>(missing).is_err());
}

#[test]
fn composed_lifecycle_preserves_completion_before_graph_arrival() {
    use loopflow::lf::commands::waves::RoadmapTask;
    use loopflow::ops::task::TaskStatus;
    let captures: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/task_lifecycle.json"
    ))
    .unwrap();
    for phase in ["merged", "completed", "arrived"] {
        let capture = &captures[phase];
        let status: TaskStatus = serde_json::from_value(capture["status"].clone()).unwrap();
        let row: RoadmapTask = serde_json::from_value(capture["row"].clone()).unwrap();
        let overview = status.overview.unwrap();
        assert_eq!(overview.graph_processes.len(), status.graph_processes.len());
        let execution = status.execution.unwrap();
        assert_eq!(overview.graph, execution.work.graph);
        assert_eq!(row.runtime.as_ref().unwrap().status, execution.status);
        if phase != "merged" {
            assert_eq!(execution.status, loopflow::durable::TaskState::Done);
        }
    }
    assert_eq!(
        captures["merged"]["status"]["execution"]["work"]["graph"],
        captures["completed"]["status"]["execution"]["work"]["graph"]
    );
    assert_eq!(
        captures["completed"]["status"]["execution"]["work"]["graph"]["position"]["running"],
        true
    );
    assert_eq!(
        captures["arrived"]["status"]["execution"]["work"]["graph"]["position"],
        serde_json::json!({"kind":"at","step":"end"})
    );
}

#[test]
fn planning_sync_preserves_delivery_and_losing_values() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/planning_sync.json"
    ))
    .unwrap();
    let sync: loopflow::store::planning_sync::PlanningSyncStatus =
        serde_json::from_value(value.clone()).unwrap();
    assert!(sync.connected);
    assert_eq!(
        sync.changes[1].state,
        loopflow::store::planning_sync::PlanningSyncState::Uncertain
    );
    assert_eq!(sync.changes[2].field, "task_title");
    assert_eq!(sync.changes[3].field, "task_assignee");
    assert_eq!(sync.changes[2].local_value, "Local title");
    assert_eq!(
        sync.changes[2].linear_value,
        Some(serde_json::json!("Linear title"))
    );
    assert_eq!(serde_json::to_value(&sync).unwrap(), value);
    assert!(sync.lines()[3].contains("provider: null"));
}

#[test]
fn desktop_actions_name_an_exact_window_and_report_the_next_reading() {
    use loopflow::lf::commands::desktop::{
        DesktopActionDescriptor, DesktopActionReceipt, DesktopActionRequest,
    };
    let json = include_str!("../../../tests/fixtures/dto/desktop_action.json");
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    let actions: Vec<DesktopActionDescriptor> =
        serde_json::from_value(value["actions"].clone()).unwrap();
    assert_eq!(serde_json::to_value(&actions).unwrap(), value["actions"]);
    assert!(actions.iter().any(|action| action.name == "goto"
        && action.keys.len() == 9
        && action.arguments[0].required));
    let requests: Vec<DesktopActionRequest> =
        serde_json::from_value(value["requests"].clone()).unwrap();
    assert_eq!(serde_json::to_value(&requests).unwrap(), value["requests"]);
    assert!(requests[0].pane.is_none());
    assert_eq!(
        requests[2].pane.as_ref().unwrap().tab.as_deref(),
        Some("shell-tab")
    );
    for field in ["repository", "window", "action", "args"] {
        let mut missing = value["requests"][2].clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<DesktopActionRequest>(missing).is_err(),
            "{field}"
        );
    }
    let receipt: DesktopActionReceipt = serde_json::from_value(value["receipt"].clone()).unwrap();
    assert_eq!(serde_json::to_value(&receipt).unwrap(), value["receipt"]);
    assert_eq!(receipt.tab.as_deref(), Some("page-1"));
    assert_eq!(
        receipt.applies_to,
        loopflow::lf::commands::desktop::DesktopAppliesTo::Window
    );
    for field in [
        "action",
        "destination",
        "history",
        "observed_at",
        "applies_to",
    ] {
        let mut missing = value["receipt"].clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<DesktopActionReceipt>(missing).is_err(),
            "{field}"
        );
    }
}

#[test]
fn desktop_text_distinguishes_empty_and_unavailable() {
    use loopflow::lf::commands::desktop::{
        DesktopTextReading, DesktopTextResult, DesktopTextUnavailable,
    };
    let json = include_str!("../../../tests/fixtures/dto/desktop_text_readings.json");
    let readings: Vec<DesktopTextReading> = serde_json::from_str(json).unwrap();
    assert_eq!(
        serde_json::to_value(&readings).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    assert_eq!(
        readings[0].result,
        DesktopTextResult::Available {
            text: String::new(),
            truncated: false
        }
    );
    assert_eq!(
        readings[2].result,
        DesktopTextResult::Unavailable {
            reason: DesktopTextUnavailable::MissingSurface
        }
    );
    for reading in &readings {
        let wire = serde_json::to_value(reading).unwrap();
        for owner in ["request", "result"] {
            for field in wire[owner].as_object().unwrap().keys() {
                let mut missing = wire.clone();
                missing[owner].as_object_mut().unwrap().remove(field);
                assert!(serde_json::from_value::<DesktopTextReading>(missing).is_err());
            }
        }
    }
}

#[test]
fn desktop_opening_distinguishes_launch_acceptance_from_usable_and_failed() {
    use loopflow::lf::commands::desktop::{DesktopOpening, DesktopOpeningStatus};
    let json = include_str!("../../../tests/fixtures/dto/desktop_openings.json");
    let readings: Vec<DesktopOpening> = serde_json::from_str(json).unwrap();
    assert_eq!(
        readings.iter().map(|r| r.status).collect::<Vec<_>>(),
        vec![
            DesktopOpeningStatus::Opening,
            DesktopOpeningStatus::Usable,
            DesktopOpeningStatus::Failed
        ]
    );
    assert!(readings[0].reason.is_none());
    assert!(readings[2].reason.is_some());
    assert_eq!(
        serde_json::to_value(readings).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn task_run_explanations_preserve_actions_and_missing_evidence() {
    use loopflow::ops::task::{TaskRunAction, TaskRunExplanation};
    let json = include_str!("../../../tests/fixtures/dto/task_run_explanations.json");
    let reports: Vec<TaskRunExplanation> = serde_json::from_str(json).unwrap();
    assert!(matches!(
        &reports[0].action,
        Some(TaskRunAction::Run { take_up: true, stops_at: Some(step), .. }) if step == "explore"
    ));
    assert!(matches!(
        &reports[1].action,
        Some(TaskRunAction::Wait { take_up: false, step, .. }) if step == "demo"
    ));
    assert!(matches!(
        &reports[2].action,
        Some(TaskRunAction::End { take_up: false, .. })
    ));
    assert!(reports[3].action.is_none());
    assert_eq!(
        serde_json::to_value(reports).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    let mut missing: serde_json::Value = serde_json::from_str(json).unwrap();
    missing[0].as_object_mut().unwrap().remove("unavailable");
    assert!(serde_json::from_value::<Vec<TaskRunExplanation>>(missing).is_err());
}

#[test]
fn work_links_carry_readable_labels_routes_and_conversation_relation() {
    use loopflow::lf::commands::open::{ConversationRelation, WorkLink};
    let json = include_str!("../../../tests/fixtures/dto/work_links.json");
    let links: Vec<WorkLink> = serde_json::from_str(json).unwrap();
    assert_eq!(
        links[0].conversation,
        ConversationRelation::OtherConversation
    );
    assert!(links[0].target.execution_machine.is_some());
    assert_eq!(links[1].conversation, ConversationRelation::Unknown);
    assert!(links[1].url.contains("?repository="));
    assert_eq!(
        serde_json::to_value(links).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    let mut missing: serde_json::Value = serde_json::from_str(json).unwrap();
    missing[0].as_object_mut().unwrap().remove("fallback_argv");
    assert!(serde_json::from_value::<Vec<WorkLink>>(missing).is_err());
}

#[test]
fn session_connect_explanations_preserve_intent_and_unavailable_evidence() {
    use loopflow::lf::commands::context::{
        OpenMode, SessionConnectExplanation, SessionConnectIntent,
    };
    let wire: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/session_connect_explanations.json"
    ))
    .unwrap();
    let reports: Vec<SessionConnectExplanation> = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(
        reports[0].action.as_ref().unwrap().intent,
        SessionConnectIntent::Start
    );
    assert_eq!(reports[0].action.as_ref().unwrap().mode, OpenMode::Refuse);
    assert_eq!(
        reports[1].action.as_ref().unwrap().intent,
        SessionConnectIntent::Resume
    );
    assert_eq!(reports[1].action.as_ref().unwrap().mode, OpenMode::Takeover);
    assert!(reports[1].action.as_ref().unwrap().prepare_only);
    assert_eq!(
        reports[2].action.as_ref().unwrap().intent,
        SessionConnectIntent::ConnectOrResume
    );
    assert_eq!(reports[2].action.as_ref().unwrap().mode, OpenMode::Try);
    assert!(!reports[2].unavailable.is_empty());
    assert!(reports[3].action.is_none());
    assert!(reports[3].state.is_none());
    assert!(reports[3].actions.is_empty());
    assert_eq!(serde_json::to_value(&reports).unwrap(), wire);
    for field in ["resolution", "actions", "impediments", "unavailable"] {
        let mut missing = wire[0].clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<SessionConnectExplanation>(missing).is_err());
    }
}

#[test]
fn work_authors_preserve_names_and_historical_attribution() {
    let fixture = include_str!("../../../tests/fixtures/dto/work_authors.json");
    let authors: Vec<loopflow::durable::Author> = serde_json::from_str(fixture).unwrap();
    assert_eq!(
        authors[1],
        loopflow::durable::Author::Named("Fixture author".into())
    );
    assert_eq!(
        serde_json::to_value(authors).unwrap(),
        serde_json::from_str::<serde_json::Value>(fixture).unwrap()
    );
}

#[test]
fn session_steer_status_preserves_excerpt_evidence() {
    let fixture = include_str!("../../../tests/fixtures/dto/session_steers.json");
    let page: loopflow::steer::SessionSteerPage = serde_json::from_str(fixture).unwrap();
    assert_eq!(page.items[0].excerpted, Some(true));
    assert_eq!(page.items[0].evidence, "emitted");
    assert_eq!(
        serde_json::to_value(page).unwrap(),
        serde_json::from_str::<serde_json::Value>(fixture).unwrap()
    );
}

#[test]
fn peer_planning_status_keeps_independent_receipts() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/peer_planning_status.json"
    ))
    .unwrap();
    let statuses: Vec<loopflow::store::PeerPlanningStatus> =
        serde_json::from_value(value.clone()).unwrap();
    assert!(statuses[0].pending_local);
    assert_eq!(
        statuses[0].publication_state.as_deref(),
        Some("unconfirmed")
    );
    assert_eq!(statuses[0].conflicts.len(), 1);
    assert!(statuses[1].pending_local);
    assert_eq!(serde_json::to_value(&statuses).unwrap(), value);
    for field in ["conflicts", "pending_local"] {
        let mut missing = value[1].clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<loopflow::store::PeerPlanningStatus>(missing).is_err());
    }
}

#[test]
fn planning_journal_keeps_losing_values_and_its_winners() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/planning_journal.json"
    ))
    .unwrap();
    let journals: Vec<loopflow::store::PlanningJournal> =
        serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        journals[1].changes["comment-head"]
            .edit
            .change
            .value()
            .unwrap()["author"],
        serde_json::json!({"kind": "person", "name": "Maya"})
    );
    assert_eq!(
        journals[1].changes["title-loser"]
            .edit
            .change
            .value()
            .unwrap(),
        "Retained title"
    );
    assert!(!journals[1].candidates.contains("title-loser"));
    assert!(journals[1].changes["title-head"]
        .parents
        .contains("title-loser"));
    for journal in &journals {
        let snapshot = loopflow::planning_exchange::PlanningSnapshot {
            changes: journal.changes.clone(),
        };
        snapshot.validate().unwrap();
        assert_eq!(
            journal.candidates,
            snapshot.winners().map(|(id, _)| id.to_owned()).collect()
        );
    }
    assert_eq!(serde_json::to_value(&journals).unwrap(), value);
}

#[test]
fn unplaced_wave_preserves_nullable_machine() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/unplaced_wave.json"
    ))
    .unwrap();
    let wave: loopflow::lf::commands::waves::WaveSnapshot =
        serde_json::from_value(fixture.clone()).unwrap();
    assert!(wave.machine.is_none());
    assert_eq!(wave.active_tasks, 1);
    assert_eq!(serde_json::to_value(wave).unwrap(), fixture);
}

#[test]
fn repository_identity_preserves_explicit_local_locators() {
    use loopflow::durable::RepositoryIdentity;
    let json = include_str!("../../../tests/fixtures/dto/repository_identity.json");
    let identity: RepositoryIdentity = serde_json::from_str(json).unwrap();
    assert_eq!(identity.locators.len(), 2);
    assert_eq!(identity.id, identity.locators[1]);
    assert_eq!(
        serde_json::to_value(&identity).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    let mut missing = serde_json::to_value(&identity).unwrap();
    missing.as_object_mut().unwrap().remove("locators");
    assert!(serde_json::from_value::<RepositoryIdentity>(missing).is_err());
}

#[test]
fn task_planning_explanations_preserve_intent_and_effects() {
    use loopflow::ops::task::{TaskPlanningAction, TaskPlanningExplanation};
    let json = include_str!("../../../tests/fixtures/dto/task_planning_explanations.json");
    let reports: Vec<TaskPlanningExplanation> = serde_json::from_str(json).unwrap();
    assert!(matches!(
        &reports[0].action,
        Some(TaskPlanningAction::Create {
            project: Some(_),
            ..
        })
    ));
    assert!(
        matches!(&reports[1].action, Some(TaskPlanningAction::Edit { revision: 3, fields }) if fields == &["title", "assignee"])
    );
    assert!(matches!(
        &reports[2].action,
        Some(TaskPlanningAction::Comment {
            message: Some(_),
            steer: true,
            refresh: false
        })
    ));
    assert!(matches!(
        &reports[3].action,
        Some(TaskPlanningAction::Comment {
            message: None,
            refresh: true,
            ..
        })
    ));
    assert!(
        matches!(&reports[5].action, Some(TaskPlanningAction::Move { wave, project, previous_project })
        if wave.as_deref() == Some("destination-wave") && project.as_deref() == Some("destination-project") && previous_project.as_deref() == Some("source-project"))
    );
    assert!(
        matches!(&reports[6].action, Some(TaskPlanningAction::Save { path, revision, draft_bytes: 42 })
        if path == "scratch/note.md" && revision == "observed-file-revision")
    );
    assert!(reports[4].action.is_none());
    assert!(!reports[4].impediments.is_empty());
    assert_eq!(
        serde_json::to_value(reports).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    let mut missing: serde_json::Value = serde_json::from_str(json).unwrap();
    missing[0].as_object_mut().unwrap().remove("effects");
    assert!(serde_json::from_value::<Vec<TaskPlanningExplanation>>(missing).is_err());
}

#[test]
fn task_checkout_explanations_keep_proposals_separate_from_admission() {
    use loopflow::ops::task::{TaskCheckoutBehavior, TaskCheckoutExplanation};
    let json = include_str!("../../../tests/fixtures/dto/task_checkout_explanations.json");
    let reports: Vec<TaskCheckoutExplanation> = serde_json::from_str(json).unwrap();
    for (report, behavior) in reports.iter().zip([
        TaskCheckoutBehavior::Prepare,
        TaskCheckoutBehavior::Reuse,
        TaskCheckoutBehavior::Restore,
    ]) {
        assert_eq!(report.action.as_ref().unwrap().behavior, behavior);
        assert!(!report.unavailable.is_empty());
    }
    assert!(reports[3].action.is_none());
    assert_eq!(
        serde_json::to_value(reports).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn task_locations_preserve_owner_freshness_and_unavailable_evidence() {
    use loopflow::durable::{TaskLocation, TaskLocationObservation};
    let wire: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/task_locations.json"
    ))
    .unwrap();
    let readings: Vec<TaskLocationObservation> = serde_json::from_value(wire.clone()).unwrap();
    assert!(matches!(
        readings[0].location,
        TaskLocation::Recorded {
            checkout: Some(_),
            ..
        }
    ));
    assert!(matches!(
        readings[1].location,
        TaskLocation::Recorded { checkout: None, .. }
    ));
    assert!(matches!(readings[2].location, TaskLocation::Unrecorded));
    assert!(matches!(
        readings[3].location,
        TaskLocation::Unavailable { .. }
    ));
    assert_eq!(serde_json::to_value(readings).unwrap(), wire);
}

#[test]
fn task_files_frames_preserve_checkout_identity_and_heartbeat() {
    use loopflow::ops::task::TaskFilesFrame;
    let wire: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/task_files_frames.json"
    ))
    .unwrap();
    let frames: Vec<TaskFilesFrame> = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(frames[0].paths, ["note.txt", ".git/index", ""]);
    assert!(frames[1].paths.is_empty());
    assert_eq!(frames[0].request, frames[1].request);
    assert_eq!(serde_json::to_value(frames).unwrap(), wire);
    for field in ["request", "task_id", "machine_id", "checkout", "paths"] {
        let mut missing = wire[0].clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<TaskFilesFrame>(missing).is_err());
    }
}

#[test]
fn paused_task_state_is_unfinished() {
    let state: TaskState = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/task_deferred.json"
    ))
    .unwrap();
    assert_eq!(state, TaskState::Deferred);
    assert!(!state.is_terminal());
    assert_eq!(serde_json::to_string(&state).unwrap(), "\"deferred\"");
}

#[test]
fn task_overview_excludes_diagnostic_process_history() {
    let input = include_str!("../../../tests/fixtures/dto/task_overview.json");
    let overview: loopflow::task_work::TaskOverview = serde_json::from_str(input).unwrap();
    assert!(overview.graph.is_some());
    assert_eq!(
        serde_json::to_value(overview).unwrap(),
        serde_json::from_str::<serde_json::Value>(input).unwrap()
    );
}

#[test]
fn desktop_state_retains_outer_and_inner_layouts() {
    use loopflow::lf::commands::desktop::DesktopInspection;
    let reading: DesktopInspection = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/desktop_inspection.json"
    ))
    .unwrap();
    let window = &reading.windows[0];
    assert_eq!(
        window.destination,
        loopflow::lf::commands::desktop::DesktopDestination::Entered {
            from: loopflow::lf::commands::desktop::DesktopBoard::Dashboard("attention".into()),
            work: window.workspaces[0].work.clone(),
        }
    );
    assert_eq!(window.history.back, 1);
    assert_eq!(window.open_work, [window.workspaces[0].work.clone()]);
    assert_eq!(window.workspaces[0].layout.ratio, Some(0.6));
    assert_eq!(window.workspaces[0].hidden_panes, ["files-pane"]);
    assert!(window.view_parts.is_empty());
    let parts = &reading.windows[1].view_parts;
    assert_eq!(
        parts
            .iter()
            .map(|part| part.name.as_str())
            .collect::<Vec<_>>(),
        ["board", "session", "tiles"]
    );
    assert_eq!(parts[0].done_days, Some(7));
    assert_eq!(parts[0].section.as_deref(), Some("backlog"));
    assert_eq!(parts[2].ratio, 0.382);
    assert_eq!(
        parts.iter().map(|part| part.zoomed).collect::<Vec<_>>(),
        [false, false, true]
    );
    assert_eq!((parts[2].page, parts[2].pages), (Some(2), Some(3)));
    assert_eq!(
        parts.iter().map(|part| part.hidden).collect::<Vec<_>>(),
        [false, true, false]
    );
    let encoded = serde_json::to_string(&reading).unwrap();
    assert_eq!(
        serde_json::from_str::<DesktopInspection>(&encoded).unwrap(),
        reading
    );
}

#[test]
fn closed_task_pages_round_trip_plan_rows_and_their_continuation() {
    use loopflow::lf::commands::waves::ClosedTaskPageSnapshot;
    let fixture = include_str!("../../../tests/fixtures/dto/closed_task_page.json");
    let page: ClosedTaskPageSnapshot = serde_json::from_str(fixture).unwrap();
    assert_eq!(
        page.tasks[0].task.completed_at.as_deref(),
        Some("2026-07-14T18:30:00Z")
    );
    assert_eq!(page.next.as_deref(), Some(page.tasks[0].task.id.as_str()));
    assert_eq!(
        serde_json::to_value(&page).unwrap(),
        serde_json::from_str::<serde_json::Value>(fixture).unwrap()
    );
}

#[test]
fn session_rows_say_each_fact_once_and_require_every_field() {
    use loopflow::lf::commands::session::SessionRow;
    let fixture = include_str!("../../../tests/fixtures/dto/session_rows.json");
    let value: serde_json::Value = serde_json::from_str(fixture).unwrap();
    let rows: Vec<SessionRow> = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(&rows).unwrap(), value);
    for field in ["id", "title", "interactive", "state", "available", "cwd"] {
        let mut missing = value[0].clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<SessionRow>(missing).is_err(),
            "{field}"
        );
    }
}

#[test]
fn retention_summary_keeps_states_expiries_and_policies_exactly() {
    use loopflow::transcript::summary::{AgeBucket, RetentionSummary};
    use loopflow::transcript::{ExpiryPolicy, RetentionState};
    let json = include_str!("../../../tests/fixtures/dto/retention_summary.json");
    let summary: RetentionSummary = serde_json::from_str(json).unwrap();
    assert_eq!(summary.groups[1].age, AgeBucket::UnderQuarter);
    assert_eq!(summary.groups[1].state, RetentionState::Facts);
    assert_eq!(summary.homes[1].policy, ExpiryPolicy::Kept);
    assert_eq!(summary.harnesses[1].next_expiry, None);
    // What `lf history retention --json` prints is exactly what Swift decodes.
    assert_eq!(
        serde_json::to_value(&summary).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    let mut missing: serde_json::Value = serde_json::from_str(json).unwrap();
    missing["harnesses"][0]
        .as_object_mut()
        .unwrap()
        .remove("unknown");
    assert!(serde_json::from_value::<RetentionSummary>(missing).is_err());
}

#[test]
fn repo_people_keep_unresolved_github_identity_explicit() {
    use loopflow::ops::people::RepoPerson;
    let json = include_str!("../../../tests/fixtures/dto/repo_people.json");
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    let people: Vec<RepoPerson> = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(people[1].github.as_ref().unwrap().name, None);
    assert_eq!(people[2].github, None);
    assert_eq!(serde_json::to_value(&people).unwrap(), value);
    let mut missing = value[2].clone();
    missing.as_object_mut().unwrap().remove("email");
    assert!(serde_json::from_value::<RepoPerson>(missing).is_err());
}

#[test]
fn task_create_receipt_names_the_graph_and_admitted_conversation_or_a_failed_run() {
    let json = include_str!("../../../tests/fixtures/dto/task_create_receipt.json");
    let receipts: Vec<loopflow::lf::commands::task_create::TaskCreateReceipt> =
        serde_json::from_str(json).unwrap();
    assert_eq!(receipts[0].graph.as_deref(), Some("code"));
    assert!(receipts[0].session_id.is_some());
    // The run failed after the Task was filed.
    assert_eq!(receipts[1].graph, None);
    assert_eq!(receipts[1].session_id, None);
    assert_eq!(
        serde_json::to_value(&receipts).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn wave_graph_reports_its_default_with_or_without_a_wave() {
    let json = include_str!("../../../tests/fixtures/dto/wave_graph.json");
    let graphs: Vec<loopflow::ops::project::WaveGraph> = serde_json::from_str(json).unwrap();
    assert_eq!(graphs[0].default.source, "repo");
    assert!(graphs[1].wave.is_none());
    assert_eq!(
        serde_json::to_value(&graphs).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}
