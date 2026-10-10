use loopflow::durable::TaskState;
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
    assert_eq!(snapshot.projects[0].workflow, "feature");
    assert_eq!(snapshot.projects[0].team_ids, ["team-loo"]);
    assert_eq!(snapshot.items[0].identifier, "LOO-2");
    assert_eq!(snapshot.items[0].state.as_deref(), Some("unstarted"));
    assert_eq!(snapshot.items[0].completed_at, None);
    assert_eq!(
        snapshot.items[0].project_id.as_deref(),
        Some("project-gmail")
    );
    assert_eq!(snapshot.items[0].team_id.as_deref(), Some("team-loo"));

    let round_trip = serde_json::to_string(&snapshot).unwrap();
    assert_eq!(
        serde_json::from_str::<PmShowResult>(&round_trip).unwrap(),
        snapshot
    );
}

#[test]
fn pm_show_preserves_an_unmapped_team_as_null() {
    let mut fixture: serde_json::Value = serde_json::from_str(PM_SHOW).unwrap();
    fixture["items"][0]["team_id"] = serde_json::Value::Null;

    let snapshot: PmShowResult = serde_json::from_value(fixture.clone()).unwrap();
    assert_eq!(snapshot.items[0].team_id, None);
    assert_eq!(
        serde_json::to_value(&snapshot.items[0]).unwrap(),
        fixture["items"][0]
    );
}

#[test]
fn wave_detail_preserves_flow_and_requires_machine() {
    let snapshot: WaveDetailSnapshot = serde_json::from_str(WAVE_DETAIL).unwrap();
    assert_eq!(snapshot.wave.machine.label.as_deref(), Some("mini"));
    assert_eq!(snapshot.wave.machine.repo.as_deref(), Some("src/project"));
    assert_eq!(
        snapshot.project_readiness.state,
        loopflow::store::sqlite::ProjectReadinessState::Ready
    );
    assert!(snapshot.project_readiness.activation.is_none());
    let Evidence::Ok {
        items: projects, ..
    } = &snapshot.projects
    else {
        panic!("missing Projects")
    };
    assert_eq!(
        projects[0].sync.as_ref().unwrap().changes[1].field,
        "task_order"
    );
    let Evidence::Ok { items: tasks, .. } = &snapshot.tasks else {
        panic!("missing Tasks")
    };
    assert_eq!(
        tasks[0].task.sync.as_ref().unwrap().changes[0].field,
        "creation"
    );
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
    assert_eq!(items[0].workflow, "task-design");
    assert!(items[0].current);
    assert_eq!(items[0].status, loopflow::pm::ProjectStatus::Started);

    let encoded = serde_json::to_string(&snapshot).unwrap();
    let decoded: WaveDetailSnapshot = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        serde_json::to_value(&decoded.projects).unwrap(),
        serde_json::to_value(&snapshot.projects).unwrap()
    );

    let mut missing_machine: serde_json::Value = serde_json::from_str(WAVE_DETAIL).unwrap();
    missing_machine["wave"]
        .as_object_mut()
        .unwrap()
        .remove("machine");
    assert!(serde_json::from_value::<WaveDetailSnapshot>(missing_machine).is_err());
}

#[test]
fn status_preserves_stranded_tasks_alongside_project_history() {
    let snapshot: WaveDetailSnapshot = serde_json::from_str(WAVE_DETAIL).unwrap();
    assert_eq!(snapshot.unavailable_tasks[0].status, TaskState::Ready);
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
        serde_json::to_value(&roadmap.waves[0].projects).unwrap()["items"][0]["workflow"],
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
    assert_eq!(
        events[0].agent_session.as_ref().unwrap().as_str(),
        "thread_fixture"
    );
    assert!(events[1].process_lfid.is_none());
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
    assert_eq!(
        page.entries[1].parent_process_lfid.as_ref(),
        Some(&page.entries[0].lfid)
    );
    assert_eq!(page.entries[1].outcome, None);
    assert_eq!(page.next.as_ref().unwrap().lfid, page.entries[1].lfid);
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
    assert!(value.providers[0].process_lfid.is_none());
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
fn flow_compositions_round_trip_distinct_compositions_and_required_children() {
    use loopflow::engine::flow_graph::FlowCatalogEntry;
    let json = include_str!("../../../tests/fixtures/dto/flow_composition.json");
    let entry: FlowCatalogEntry = serde_json::from_str(json).unwrap();
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_value(&entry).unwrap(), value);
    let mut missing = value;
    missing["composition"]["items"][2]
        .as_object_mut()
        .unwrap()
        .remove("items");
    assert!(serde_json::from_value::<FlowCatalogEntry>(missing).is_err());
    let catalog: Vec<FlowCatalogEntry> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/flow_catalog.json"
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
    assert_eq!(work.flow_processes.len(), 1);
    assert!(!work.processes.is_empty());
    let workflow = work.workflow.as_ref().expect("fixture Task has a workflow");
    assert_eq!(
        workflow.position,
        loopflow::ops::workflow::WorkflowPosition::Edge {
            edge: 2,
            process_lfid: work.flow_processes[0].summary.id.clone(),
            running: true,
        }
    );
    // A conversation's move names it; an edge's arrival names no one.
    let asked = workflow.history.last().expect("fixture has history");
    assert_eq!(
        asked.actor,
        loopflow::ops::workflow::WorkflowActor::Conversation
    );
    assert_eq!(
        asked.session_id.as_deref(),
        Some(work.sessions[0].id.as_str())
    );
    assert_eq!(
        serde_json::to_value(work).unwrap(),
        serde_json::from_str::<serde_json::Value>(input).unwrap()
    );
}

#[test]
fn task_automation_keeps_coverage_distinct_from_repair_hold() {
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
    assert_eq!((steers.count, steers.over_budget), (Some(384), false));
    assert!(report.steps[1]
        .sources
        .iter()
        .all(|usage| usage.tokens.is_none()));
    assert_eq!(ContextReport::new(report.steps.clone()), report);
}

#[test]
fn flow_detail_keeps_its_launched_graph_and_every_step() {
    let input = include_str!("../../../tests/fixtures/dto/flow_detail.json");
    let detail: loopflow::durable::FlowProcessDetail = serde_json::from_str(input).unwrap();
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
fn work_frames_keep_each_part_and_require_every_envelope_field() {
    use loopflow::lf::commands::work_watch::{WorkContent, WorkFrame};
    let json = include_str!("../../../tests/fixtures/dto/work_frame.json");
    let frames: Vec<WorkFrame> = serde_json::from_str(json).unwrap();
    let source: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_value(&frames).unwrap(), source);
    assert!(matches!(frames[0].content, WorkContent::Planning(Some(_))));
    assert_eq!(frames[0].answers, None);
    assert!(matches!(frames[2].content, WorkContent::Task(None)));
    assert!(frames[2].unavailable.is_some());
    assert!(frames[4].revisions.is_none());
    let WorkContent::Heartbeat(heartbeat) = &frames[5].content else {
        panic!("last fixture frame is a heartbeat");
    };
    assert_eq!(heartbeat.projections["planning"], 3);
    let WorkContent::Task(Some(task)) = &frames[6].content else {
        panic!("the read Task part carries its work and Flow runs");
    };
    assert_eq!(task.work.flow_processes.len(), task.flow_processes.len());
    for field in ["sequence", "home", "part"] {
        let mut missing = source[0].clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<WorkFrame>(missing).is_err(),
            "{field} must be required"
        );
    }
}

#[test]
fn separate_workflow_catalog_preserves_invalid_sources() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/workflow_catalog.json"
    ))
    .unwrap();
    let entries: Vec<loopflow::engine::workflow::WorkflowCatalogEntry> =
        serde_json::from_value(value.clone()).unwrap();
    assert!(entries[0].workflow.is_some());
    assert!(entries[1].workflow.is_none() && entries[1].unavailable.is_some());
    assert_eq!(serde_json::to_value(entries).unwrap(), value);
}

#[test]
fn task_delivery_preserves_pending_filings_links_and_due_unstarted_follow_ups() {
    use loopflow::lf::commands::waves::RoadmapTask;
    let json = include_str!("../../../tests/fixtures/dto/task_delivery_rows.json");
    let rows: std::collections::BTreeMap<String, RoadmapTask> = serde_json::from_str(json).unwrap();
    assert_eq!(
        rows["conversion"].follow_through.scope_notes,
        ["Verify the installed release. Evidence: the command succeeds on the released version."]
    );
    assert!(!rows["pending"].follow_through.resolved());
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
    assert_eq!(rows["pending"].follow_through.intents.len(), 1);
    assert!(rows["done"].follow_through.resolved());
    assert_eq!(rows["done"].follow_through.links[0].identifier, "W2-FOLLOW");
    assert!(rows["none"].follow_through.resolved());
    assert!(rows["none"].follow_through.links.is_empty());
    assert!(rows["due"].runtime.is_none());
    assert_eq!(rows["due"].task.due_date.as_deref(), Some("2026-10-08"));
    assert_eq!(
        rows["due"].task.follow_up_sources[0].identifier,
        "W2-SOURCE"
    );
    assert!(rows["unrelated"].task.follow_up_sources.is_empty());
    assert_eq!(
        serde_json::to_value(&rows).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    let mut missing = serde_json::to_value(&rows["pending"]).unwrap();
    missing.as_object_mut().unwrap().remove("follow_through");
    assert!(serde_json::from_value::<RoadmapTask>(missing).is_err());
}

#[test]
fn composed_lifecycle_preserves_completion_before_workflow_arrival() {
    use loopflow::lf::commands::waves::RoadmapTask;
    use loopflow::lf::commands::work_watch::{WorkContent, WorkFrame};
    use loopflow::ops::task::TaskStatus;
    let captures: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/task_lifecycle.json"
    ))
    .unwrap();
    for phase in ["merged", "completed", "arrived"] {
        let capture = &captures[phase];
        let status: TaskStatus = serde_json::from_value(capture["status"].clone()).unwrap();
        let row: RoadmapTask = serde_json::from_value(capture["row"].clone()).unwrap();
        let planning: WorkFrame =
            serde_json::from_value(capture["planning_frame"].clone()).unwrap();
        let task: WorkFrame = serde_json::from_value(capture["task_frame"].clone()).unwrap();
        assert!(planning.unavailable.is_none() && task.unavailable.is_none());
        let WorkContent::Task(Some(task)) = task.content else {
            panic!("Task frame missing")
        };
        let WorkContent::Planning(Some(plan)) = planning.content else {
            panic!("planning frame missing")
        };
        let execution = status.execution.unwrap();
        assert_eq!(task.work.workflow, execution.work.workflow);
        assert_eq!(row.runtime.as_ref().unwrap().status, execution.status);
        assert_eq!(row.follow_through, execution.follow_through);
        let loopflow::lf::commands::waves::Evidence::Ok { items, .. } =
            &plan.roadmap.waves[0].tasks
        else {
            panic!("Task plan unavailable")
        };
        let projected = items
            .iter()
            .find(|row| row.task.identifier == "FIX-1")
            .unwrap();
        assert_eq!(projected.runtime.as_ref().unwrap().status, execution.status);
        assert_eq!(projected.follow_through, execution.follow_through);
        if phase != "merged" {
            assert_eq!(execution.status, loopflow::durable::TaskState::Done);
            assert!(execution.follow_through.resolved());
            assert_eq!(execution.follow_through.links.len(), 1);
            assert!(execution.follow_through.links[0]
                .identifier
                .starts_with("lf-"));
            assert_eq!(
                execution.follow_through.links[0].due.as_deref(),
                Some("2026-10-09")
            );
        }
    }
    assert_eq!(
        captures["merged"]["status"]["execution"]["work"]["workflow"],
        captures["completed"]["status"]["execution"]["work"]["workflow"]
    );
    assert_eq!(
        captures["completed"]["status"]["execution"]["work"]["workflow"]["position"]["running"],
        true
    );
    assert_eq!(
        captures["arrived"]["status"]["execution"]["work"]["workflow"]["position"],
        serde_json::json!({"kind":"node","node":"end"})
    );
}

#[test]
fn planning_sync_preserves_delivery_and_losing_values() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/planning_sync.json"
    ))
    .unwrap();
    let sync: loopflow::planning::PlanningSyncStatus =
        serde_json::from_value(value.clone()).unwrap();
    assert!(sync.connected);
    assert_eq!(
        sync.changes[1].state,
        loopflow::planning::PlanningSyncState::Uncertain
    );
    assert_eq!(sync.changes[2].local_value, "Local title");
    assert_eq!(
        sync.changes[2].linear_value,
        Some(serde_json::json!("Linear title"))
    );
    assert_eq!(serde_json::to_value(&sync).unwrap(), value);
    assert!(sync.lines()[3].contains("Linear: null"));
}
