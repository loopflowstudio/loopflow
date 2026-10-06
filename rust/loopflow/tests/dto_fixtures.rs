use loopflow::durable::WorkStatus;
use loopflow::lf::commands::waves::{Evidence, RoadmapSnapshot, WaveDetailSnapshot};
use loopflow::ops::pm::PmShowResult;
use loopflow::work::wave::metrics::MetricPortfolioDto;

const PM_SHOW: &str = include_str!("../../../tests/fixtures/dto/pm_show.json");
const WAVE_DETAIL: &str = include_str!("../../../tests/fixtures/dto/wave_detail.json");
const ROADMAP: &str = include_str!("../../../tests/fixtures/dto/roadmap_snapshot.json");
const METRIC_PORTFOLIO: &str = include_str!("../../../tests/fixtures/dto/metric_portfolio.json");

#[test]
fn active_sessions_preserve_identity_waiting_clients_and_incomplete_evidence() {
    use loopflow::lf::commands::session_history::ActiveSessionsSnapshot;
    use loopflow::lf::commands::top::ActivityState;
    let json = include_str!("../../../tests/fixtures/dto/active_runs.json");
    let snapshot: ActiveSessionsSnapshot = serde_json::from_str(json).unwrap();
    assert_eq!(
        snapshot.discovery,
        loopflow::lf::commands::session_history::DiscoveryState::Ready
    );
    assert_eq!(snapshot.sessions[0].work, snapshot.task);
    assert_eq!(
        snapshot.sessions[0].processes[0].state,
        ActivityState::Waiting
    );
    assert_eq!(snapshot.gaps.len(), 1);
    assert_eq!(
        serde_json::from_str::<ActiveSessionsSnapshot>(&serde_json::to_string(&snapshot).unwrap())
            .unwrap(),
        snapshot
    );
    let mut missing: serde_json::Value = serde_json::from_str(json).unwrap();
    missing.as_object_mut().unwrap().remove("discovery");
    assert!(serde_json::from_value::<ActiveSessionsSnapshot>(missing).is_err());
}

#[test]
fn task_comments_keep_authorship_and_require_every_field() {
    use loopflow::ops::pm::{TaskCommentAuthor, TaskComments};
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
fn pm_show_preserves_repository_team_and_project_ownership() {
    let snapshot: PmShowResult = serde_json::from_str(PM_SHOW).unwrap();

    assert_eq!(snapshot.wave, "survival/infrastructure");
    assert_eq!(
        snapshot.projects[0].initiative_ids,
        ["initiative-infrastructure"]
    );
    assert_eq!(snapshot.projects[0].name, "Gmail");
    assert_eq!(snapshot.projects[0].flow, "feature");
    assert_eq!(snapshot.projects[0].team_ids, ["team-loo"]);
    assert_eq!(snapshot.items[0].identifier, "LOO-2");
    assert_eq!(snapshot.items[0].state.as_deref(), Some("unstarted"));
    assert_eq!(snapshot.items[0].completed_at, None);
    assert_eq!(
        snapshot.items[0].project_id.as_deref(),
        Some("project-gmail")
    );
    assert_eq!(snapshot.items[0].team_id, "team-loo");

    let round_trip = serde_json::to_string(&snapshot).unwrap();
    assert_eq!(
        serde_json::from_str::<PmShowResult>(&round_trip).unwrap(),
        snapshot
    );
}

#[test]
fn pm_show_requires_team_identity_even_without_project_ownership() {
    let mut fixture: serde_json::Value = serde_json::from_str(PM_SHOW).unwrap();
    fixture["items"][0]
        .as_object_mut()
        .unwrap()
        .remove("team_id");

    let error = serde_json::from_value::<PmShowResult>(fixture).unwrap_err();
    assert!(error.to_string().contains("team_id"));
}

#[test]
fn wave_detail_preserves_flow_and_requires_home() {
    let snapshot: WaveDetailSnapshot = serde_json::from_str(WAVE_DETAIL).unwrap();
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
    assert_eq!(items[0].flow, "task-design");
    assert!(items[0].current);
    assert_eq!(items[0].status, loopflow::pm::ProjectStatus::Started);

    let encoded = serde_json::to_string(&snapshot).unwrap();
    let decoded: WaveDetailSnapshot = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        serde_json::to_value(&decoded.projects).unwrap(),
        serde_json::to_value(&snapshot.projects).unwrap()
    );

    let mut missing_home: serde_json::Value = serde_json::from_str(WAVE_DETAIL).unwrap();
    missing_home["wave"].as_object_mut().unwrap().remove("home");
    assert!(serde_json::from_value::<WaveDetailSnapshot>(missing_home).is_err());
}

#[test]
fn status_preserves_stranded_tasks_alongside_project_history() {
    let snapshot: WaveDetailSnapshot = serde_json::from_str(WAVE_DETAIL).unwrap();
    assert_eq!(snapshot.unavailable_tasks[0].status, WorkStatus::Ready);
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
    let Evidence::Ok { items, .. } = &roadmap.waves[0].tasks else {
        panic!("fixture has current Project evidence")
    };
    assert!(!items.is_empty());
    assert_eq!(
        serde_json::to_value(&roadmap.waves[0].projects).unwrap()["items"][0]["flow"],
        "feature"
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
fn session_history_retains_receipts_and_unknown_driver() {
    let input = include_str!("../../../tests/fixtures/dto/session_history.json");
    let events: Vec<loopflow::session::SessionEvent> = serde_json::from_str(input).unwrap();
    assert_eq!(
        events[1].kind,
        loopflow::session::SessionEventKind::Completed
    );
    assert!(events[1].exec_id.is_none());
    assert_eq!(events[0].payload["total"]["inputTokens"], 40);
    assert_eq!(
        events[2].kind,
        loopflow::session::SessionEventKind::Observed
    );
    assert!(events[2].provider_turn.is_none());
    assert!(events[2].provider_thread.is_none());
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
fn exec_page_retains_outcomes_unknowns_and_continuation() {
    let json = include_str!("../../../tests/fixtures/dto/exec_page.json");
    let page: loopflow::exec::ExecPage = serde_json::from_str(json).unwrap();
    assert_eq!(page.entries[0].exit_code, Some(42));
    assert_eq!(page.entries[0].via_agent, None);
    assert_eq!(
        page.entries[1].parent_exec_id.as_ref(),
        Some(&page.entries[0].id)
    );
    assert_eq!(page.entries[1].outcome, None);
    assert_eq!(page.next.as_ref().unwrap().id, page.entries[1].id);
    assert_eq!(
        serde_json::to_value(page).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    assert!(serde_json::from_str::<loopflow::exec::ExecPage>("{}").is_err());
}

#[test]
fn session_input_history_retains_distinct_native_results_and_unknown_exec() {
    let value: loopflow::lf::commands::session_history::SessionHistory = serde_json::from_str(
        include_str!("../../../tests/fixtures/dto/session_history_summary.json"),
    )
    .unwrap();
    assert_eq!(value.providers.len(), 2);
    assert_eq!(value.providers[0].outcome.as_deref(), Some("failed"));
    assert!(value.providers[0].exec_id.is_none());
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
fn task_status_preserves_planning_freshness_without_execution() {
    use loopflow::ops::task::TaskStatus;
    let json = include_str!("../../../tests/fixtures/dto/task_status.json");
    let states: Vec<TaskStatus> = serde_json::from_str(json).unwrap();
    assert_eq!(
        states[0]
            .planning
            .as_ref()
            .unwrap()
            .item
            .branch_name
            .as_deref(),
        Some("dev/fix-1-existing")
    );
    assert_eq!(
        states
            .iter()
            .map(|state| state.planning_state)
            .collect::<Vec<_>>(),
        vec![
            loopflow::store::PlanningState::Available,
            loopflow::store::PlanningState::Unavailable,
            loopflow::store::PlanningState::Unavailable,
            loopflow::store::PlanningState::Invalid,
            loopflow::store::PlanningState::Removed,
            loopflow::store::PlanningState::Absent,
        ]
    );
    for state in &states[3..] {
        assert_eq!(state.planning, states[0].planning);
    }
    assert!(!states[0].planning_stale);
    assert!(states[0].planning_error.is_none());
    assert!(states[0].execution.is_none());
    assert!(states[1].planning_stale);
    assert_eq!(
        states[1].planning_error.as_deref(),
        Some("Linear unavailable")
    );
    assert_eq!(states[0].planning, states[1].planning);
    assert!(states[2].planning.is_none());
    assert!(states[2].planning_error.is_some());
    assert_eq!(
        serde_json::to_value(states).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    let mut missing: serde_json::Value = serde_json::from_str(json).unwrap();
    missing[0].as_object_mut().unwrap().remove("planning_stale");
    assert!(serde_json::from_value::<Vec<TaskStatus>>(missing).is_err());
}

#[test]
fn flow_templates_round_trip_distinct_compositions_and_required_children() {
    use loopflow::engine::flow_graph::FlowCatalogEntry;
    let json = include_str!("../../../tests/fixtures/dto/flow_template.json");
    let entry: FlowCatalogEntry = serde_json::from_str(json).unwrap();
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_value(&entry).unwrap(), value);
    let mut missing = value;
    missing["template"]["items"][2]
        .as_object_mut()
        .unwrap()
        .remove("items");
    assert!(serde_json::from_value::<FlowCatalogEntry>(missing).is_err());
    let catalog: Vec<FlowCatalogEntry> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/flow_catalog.json"
    ))
    .unwrap();
    assert!(catalog[0].template.is_some() && catalog[1].template.is_none());
}

#[test]
fn prepared_checkout_retains_owning_home_without_starting_execution() {
    let json = include_str!("../../../tests/fixtures/dto/task_checkout.json");
    let snapshot: loopflow::ops::task::TaskSnapshot = serde_json::from_str(json).unwrap();
    assert_eq!(
        snapshot.home_id.as_ref().unwrap().as_str(),
        "home_00000000000000000000000000000001"
    );
    assert_eq!(snapshot.worktree, "/src/loopflow.workspace");
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
fn task_work_preserves_all_owners_and_managed_marker() {
    let input = include_str!("../../../tests/fixtures/dto/task_work.json");
    let work: loopflow::task_work::TaskWork = serde_json::from_str(input).unwrap();
    assert_eq!(work.sessions.len(), 2);
    assert!(!work.sessions[0].managed);
    assert!(work.sessions[1].managed);
    assert!(work.flows[0].managed);
    assert!(!work.execs.is_empty());
    assert_eq!(
        serde_json::to_value(work).unwrap(),
        serde_json::from_str::<serde_json::Value>(input).unwrap()
    );
}

#[test]
fn task_automation_keeps_coverage_distinct_from_held_and_reviewing_work() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/task_automation.json"
    ))
    .unwrap();
    let status: loopflow::ops::task_automation::AutomationStatus =
        serde_json::from_value(value.clone()).unwrap();
    assert_eq!(status.coverage, "overdue");
    assert_eq!(status.tasks[1].enabled, Some(false));
    assert_eq!(serde_json::to_value(status).unwrap(), value);
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
    assert_eq!((steers.count, steers.over_budget), (Some(384), true));
    assert!(report.steps[1]
        .sources
        .iter()
        .all(|usage| usage.tokens.is_none()));
    assert_eq!(ContextReport::new(report.steps.clone()), report);
}

#[test]
fn workspace_frames_keep_each_part_and_require_every_envelope_field() {
    use loopflow::lf::commands::workspace_watch::{WorkspaceContent, WorkspaceFrame};
    let json = include_str!("../../../tests/fixtures/dto/workspace_frame.json");
    let frames: Vec<WorkspaceFrame> = serde_json::from_str(json).unwrap();
    let source: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_value(&frames).unwrap(), source);
    assert!(matches!(
        frames[0].content,
        WorkspaceContent::Planning(Some(_))
    ));
    assert_eq!(frames[0].answers, None);
    assert!(matches!(frames[2].content, WorkspaceContent::Task(None)));
    assert!(frames[2].unavailable.is_some());
    assert!(frames[4].revisions.is_none());
    let WorkspaceContent::Heartbeat(heartbeat) = &frames[5].content else {
        panic!("last fixture frame is a heartbeat");
    };
    assert_eq!(heartbeat.projections["planning"], 3);
    for field in ["sequence", "home", "part"] {
        let mut missing = source[0].clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<WorkspaceFrame>(missing).is_err(),
            "{field} must be required"
        );
    }
}
