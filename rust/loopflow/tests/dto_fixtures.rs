use loopflow::lf::commands::waves::{Evidence, RoadmapSnapshot, WaveDetailSnapshot};

const WAVE_DETAIL: &str = include_str!("../../../tests/fixtures/dto/wave_detail.json");
const ROADMAP: &str = include_str!("../../../tests/fixtures/dto/roadmap_snapshot.json");

#[test]
fn task_steer_receipt_requires_its_delivery_and_nullable_machine() {
    use loopflow_execution::engine::task::TaskSteerReceipt;
    let wire: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/task_steer_receipt.json"
    ))
    .unwrap();
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<TaskSteerReceipt>(wire.clone()).unwrap())
            .unwrap(),
        wire
    );
    let mut missing = wire;
    missing["delivery"]
        .as_object_mut()
        .unwrap()
        .remove("runs_on");
    assert!(serde_json::from_value::<TaskSteerReceipt>(missing).is_err());
}

#[test]
fn roadmap_requires_shared_lifecycle_and_selection_states() {
    use loopflow::lf::commands::waves::RoadmapTask;
    let wire: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/task_concepts.json"
    ))
    .unwrap();
    let row = &wire[0]["tasks"][0];
    for field in ["lifecycle", "selection_states"] {
        let mut missing = row.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<RoadmapTask>(missing).is_err());
    }
    let task: RoadmapTask = serde_json::from_value(row.clone()).unwrap();
    assert_eq!(serde_json::to_value(task).unwrap(), *row);
}

#[test]
fn task_comments_keep_authorship_and_require_every_field() {
    use loopflow_execution::work::task::comment::TaskCommentAuthor;
    use loopflow_execution::work::task::comment::TaskComments;
    let json = include_str!("../../../tests/fixtures/dto/task_comments.json");
    let thread: TaskComments = serde_json::from_str(json).unwrap();
    assert_eq!(thread.comments[0].author, TaskCommentAuthor::Integration);
    assert_eq!(
        thread.comments[2].author,
        TaskCommentAuthor::Person { name: None }
    );
    assert_eq!(
        thread.comments[3].author,
        TaskCommentAuthor::Session {
            session_id: "session_05214bdabd2e4f66b4a8951aac87e9e7".into(),
            requester: Some("Maya".into())
        }
    );
    // What `lf task steer ISSUE MESSAGE --json` prints is exactly what Swift decodes,
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
    assert_eq!(snapshot.setup.graph.value.as_deref(), Some("code"));
    assert_eq!(
        snapshot.setup.skill_agents["implement"].source,
        loopflow_execution::config::ConfigSource::Repository
    );
    let loopflow::lf::commands::waves::Reading::Ok {
        value: Some(project),
    } = &snapshot.project
    else {
        panic!("missing Project")
    };
    assert_eq!(project.sync.actions[1].change, "order");
    let Evidence::Ok { items: tasks, .. } = &snapshot.tasks else {
        panic!("missing Tasks")
    };
    assert_eq!(tasks[0].task.sync.actions[0].change, "create");
    assert_eq!(tasks[0].task.id, tasks[0].runtime.as_ref().unwrap().work_id);
    assert!(tasks[0].runtime.as_ref().unwrap().provider.is_none());
    assert_eq!(tasks[0].task.tracker_id.as_deref(), Some("issue-1"));
    assert_eq!(tasks[0].task.parent_id, None);
    assert_eq!(tasks[1].task.parent_id.as_ref(), Some(&tasks[0].task.id));
    let Evidence::Ok { items: runs, .. } = &snapshot.history else {
        panic!("fixture contains recorded Runs");
    };
    assert_eq!(runs[0].first_provider_attempt_at, Some(1784052010));
    assert_eq!(
        runs[0].task_pr_id.as_ref().map(|id| id.as_str()),
        Some("pr_33333333333333333333333333333333")
    );

    let loopflow::lf::commands::waves::Reading::Ok {
        value: Some(project),
    } = &snapshot.project
    else {
        panic!("missing Project")
    };
    assert_eq!(
        project.status,
        loopflow_execution::work::project::ProjectStatus::Started
    );

    let encoded = serde_json::to_string(&snapshot).unwrap();
    let decoded: WaveDetailSnapshot = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        serde_json::to_value(&decoded.project).unwrap(),
        serde_json::to_value(&snapshot.project).unwrap()
    );
}

#[test]
fn status_keeps_task_runtime_alongside_project_status() {
    let snapshot: WaveDetailSnapshot = serde_json::from_str(WAVE_DETAIL).unwrap();
    let Evidence::Ok { items, .. } = snapshot.tasks else {
        panic!("available tasks")
    };
    assert_eq!(items[0].runtime.as_ref().unwrap().reason, "ready");
    let encoded = serde_json::from_str::<serde_json::Value>(WAVE_DETAIL).unwrap();
    assert_eq!(encoded["project"]["value"]["status"], "started");
}

#[test]
fn status_and_roadmap_round_trip_and_keep_workspace_facts() {
    let detail: WaveDetailSnapshot = serde_json::from_str(WAVE_DETAIL).unwrap();
    let roadmap: RoadmapSnapshot = serde_json::from_str(ROADMAP).unwrap();
    // Each Task is written once, under the roadmap; Waves do not repeat them.
    assert_eq!(
        serde_json::to_value(&roadmap).unwrap(),
        serde_json::from_str::<serde_json::Value>(ROADMAP).unwrap()
    );
    let Evidence::Ok { items, .. } = roadmap.wave_tasks(&roadmap.waves[0]) else {
        panic!("fixture has Project evidence")
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
        serde_json::to_value(&roadmap.waves[0].project).unwrap()["value"]
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
}

#[test]
fn session_history_lists_turns_beside_observations() {
    let input = include_str!("../../../tests/fixtures/dto/session_history.json");
    let history: serde_json::Value = serde_json::from_str(input).unwrap();
    let turns: Vec<loopflow_execution::session::Turn> =
        serde_json::from_value(history["turns"].clone()).unwrap();
    let events: Vec<loopflow_execution::session::SessionEvent> =
        serde_json::from_value(history["events"].clone()).unwrap();
    assert_eq!(turns[0].thread.as_str(), "thread_fixture");
    assert_eq!(turns[0].status.as_deref(), Some("completed"));
    assert!(turns[0].lf_process_id.is_none());
    assert_eq!(
        turns[0].usage.as_ref().unwrap().usage.total_input_tokens,
        Some(20)
    );
    assert_eq!(
        events[0].kind,
        loopflow_execution::session::SessionEventKind::Observed
    );
    assert!(events[0].provider_turn.is_none());
    assert_eq!(
        serde_json::to_value(&turns).unwrap(),
        history["turns"],
        "a turn round-trips"
    );
}

#[test]
fn task_files_share_exact_bases_rename_paths_and_lossless_revisions() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Files {
        directory: loopflow_execution::ops::task::TaskDirectory,
        changes: loopflow_execution::ops::task::TaskChangesSnapshot,
        diff: loopflow_execution::ops::task::TaskDiffSnapshot,
        file: loopflow_execution::ops::task::TaskFileSnapshot,
        image: loopflow_execution::ops::task::TaskFileSnapshot,
        save: loopflow_execution::ops::task::TaskFileSave,
    }
    let json = include_str!("../../../tests/fixtures/dto/task_files.json");
    let files: Files = serde_json::from_str(json).unwrap();
    assert_eq!(files.changes.base_commit, files.diff.base_commit);
    assert_eq!(files.changes.commits[0].parent, files.changes.base_commit);
    assert_eq!(files.changes.files[0].old_path.as_deref(), Some("old.txt"));
    assert_eq!(files.changes.files[0].changes, 1);
    assert_eq!(files.file.content.as_deref(), Some("\u{feff}notes\r\n"));
    assert_eq!(files.image.data.as_deref(), Some("iVBORw0KGgo="));
    assert_eq!(
        serde_json::to_value(files).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn session_input_history_retains_distinct_native_results_and_unknown_process() {
    let value: loopflow::lf::commands::session_history::SessionHistory = serde_json::from_str(
        include_str!("../../../tests/fixtures/dto/session_history_summary.json"),
    )
    .unwrap();
    assert_eq!(value.origin.as_ref().unwrap().name(), "mini");
    assert_eq!(value.providers.len(), 2);
    assert_eq!(value.providers[0].outcome.as_deref(), Some("failed"));
    assert!(value.providers[0].lf_process_id.is_none());
    assert_eq!(value.providers[0].usage.input_tokens, None);
    assert_eq!(value.providers[1].usage.input_tokens, Some(0));
    assert_eq!(value.status(), "failed → completed");
    let encoded = serde_json::to_value(&value).unwrap();
    let mut missing = encoded.clone();
    missing.as_object_mut().unwrap().remove("origin");
    assert!(
        serde_json::from_value::<loopflow::lf::commands::session_history::SessionHistory>(missing)
            .is_err()
    );
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
    assert_eq!(
        detail.task.state,
        Some(loopflow_execution::durable::TaskState::Active)
    );
    assert_eq!(
        detail.task.graph.as_ref().unwrap().step.as_deref(),
        Some("demo")
    );
    let graph = detail.graph.as_ref().unwrap();
    assert_eq!(graph.current_step, Some(2));
    assert_eq!(graph.sentence, "Waiting for you at demo.");
    let conversation = detail
        .sessions
        .iter()
        .find(|session| Some(&session.id) == detail.conversation.as_ref())
        .unwrap();
    assert_eq!(
        conversation.status_line.as_deref(),
        Some("died 12:01 (exit 137) · starts again 12:02"),
        "the server's sentence, printed as given"
    );
    assert_eq!(
        detail.auto.as_ref().unwrap().by.to_string(),
        "wave config",
        "a Wave's `auto: on` turned it on"
    );
    assert_eq!(
        serde_json::to_value(&detail).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn task_detail_requires_where_it_runs_and_whose_reading_it_is() {
    use loopflow::lf::commands::task_row::{TaskDetail, TaskReading};
    let wire: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/dto/task_detail.json")).unwrap();
    let mut missing = wire.clone();
    missing.as_object_mut().unwrap().remove("reading");
    assert!(serde_json::from_value::<TaskDetail>(missing).is_err());
    let mut missing = wire.clone();
    missing["task"].as_object_mut().unwrap().remove("runs_on");
    assert!(serde_json::from_value::<TaskDetail>(missing).is_err());
    let mut missing = wire.clone();
    missing["task"].as_object_mut().unwrap().remove("state");
    assert!(serde_json::from_value::<TaskDetail>(missing).is_err());

    let mut elsewhere = wire;
    elsewhere["task"]["state"] = serde_json::Value::Null;
    elsewhere["task"]["runs_on"] = serde_json::json!({
        "machine": {"id": "home_0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b", "label": "modal-3"},
        "since": 1_763_900_000
    });
    elsewhere["reading"] =
        serde_json::json!({"kind": "unobserved", "reason": "modal-3 is not registered here"});
    let detail: TaskDetail = serde_json::from_value(elsewhere.clone()).unwrap();
    assert!(detail.task.state.is_none());
    assert_eq!(
        detail.reading,
        TaskReading::Unobserved {
            reason: "modal-3 is not registered here".into()
        }
    );
    assert_eq!(
        detail.task.runs_on.as_ref().unwrap().machine.name(),
        "modal-3"
    );
    assert_eq!(serde_json::to_value(&detail).unwrap(), elsewhere);
}

#[test]
fn task_status_preserves_canonical_work_and_tracker_state() {
    use loopflow::lf::commands::waves::DesktopTaskPane;
    let json = include_str!("../../../tests/fixtures/dto/task_status.json");
    let wire: serde_json::Value = serde_json::from_str(json).unwrap();
    for (record, field) in [
        ("tracker_state", "synced_at"),
        ("task", "tracker_id"),
        ("project", "tracker_id"),
    ] {
        let mut missing = wire.clone();
        missing[record].as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<DesktopTaskPane>(missing).is_err(),
            "{record}.{field}"
        );
    }
    let pane: DesktopTaskPane = serde_json::from_value(wire).unwrap();
    // A follow-up names the Task it was filed from and lists its own.
    assert_eq!(pane.parent.as_ref().unwrap().identifier, "LOO-1");
    assert_eq!(
        pane.children[0].parent_id.as_deref(),
        Some("task_11111111111111111111111111111111")
    );
    assert_eq!(pane.children[0].identifier, "LOO-3");
    let status = &pane.status;
    let task = status.task.as_ref().unwrap();
    let project = status.project.as_ref().unwrap();
    assert_eq!(task.title, "Route incoming mail");
    assert_eq!(task.project_id, Some(project.id.clone()));
    assert_ne!(task.id.as_str(), task.tracker_id.as_ref().unwrap().as_str());
    assert_eq!(project.description, "Retained authored prose.");
    assert_eq!(status.tracker_state.synced_at, Some(1790683200));
    assert_eq!(
        status.tracker_state.state,
        loopflow_execution::store::PlanningState::Available
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
            loopflow_execution::durable::OwnershipKind::Claim,
            loopflow_execution::durable::OwnershipKind::Claim
        ]
    );
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_value(&pane).unwrap(), value);
    for field in ["plan_stale", "graph_processes", "parent", "children"] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<DesktopTaskPane>(missing).is_err(),
            "{field}"
        );
    }
}

#[test]
fn a_died_run_carries_where_it_died_and_the_conversations_outlook() {
    use loopflow_execution::ops::human_session::SessionOutlookKind;
    use loopflow_execution::ops::task_execution::TaskExecutionSnapshot;
    let json = include_str!("../../../tests/fixtures/dto/task_execution_died.json");
    let execution: TaskExecutionSnapshot = serde_json::from_str(json).unwrap();
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_value(&execution).unwrap(), value);
    assert_eq!(execution.died.as_ref().unwrap().at_step, "implement");
    let outlook = execution.outlook.as_ref().unwrap();
    assert_eq!(outlook.kind, SessionOutlookKind::WaitingForYou);
    assert_eq!(outlook.command.as_deref(), Some("lf task run LOO-12"));
    for key in ["died", "outlook"] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(key);
        assert!(serde_json::from_value::<TaskExecutionSnapshot>(missing).is_err());
    }
}

#[test]
fn graph_compositions_round_trip_distinct_compositions_and_required_children() {
    use loopflow_execution::graph::compiled::GraphCatalogEntry;
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
    for field in ["included_by", "runs", "workflow"] {
        let mut missing = serde_json::to_value(&catalog[0]).unwrap();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<GraphCatalogEntry>(missing).is_err(),
            "{field}"
        );
    }
}

#[test]
fn prepared_checkout_retains_owning_home_without_starting_execution() {
    let json = include_str!("../../../tests/fixtures/dto/task_checkout.json");
    let snapshot: loopflow_execution::ops::task::TaskSnapshot = serde_json::from_str(json).unwrap();
    assert_eq!(
        snapshot.machine_id.as_ref().unwrap().as_str(),
        "home_00000000000000000000000000000001"
    );
    assert_eq!(
        snapshot.worktree.as_deref(),
        Some("/src/loopflow.workspace")
    );
    assert!(snapshot.provider.is_none());
    assert!(snapshot.pr.is_none());
    let parent = snapshot.stack_parent.as_ref().unwrap();
    assert_eq!(parent.identifier, "LOO-290");
    assert_eq!(parent.pr_number, None);
    assert!(!snapshot.branch.as_ref().unwrap().is_empty());
    assert_eq!(
        snapshot.execution.state,
        loopflow_execution::ops::task_execution::TaskExecutionState::Idle
    );
    assert_eq!(
        snapshot.started_by.as_ref().unwrap().to_string(),
        "Session \"LOO-291 task\" for Maya"
    );
    let auto = snapshot.auto.as_ref().unwrap();
    assert_eq!(auto.state, loopflow_storage::sqlite::AutoState::Off);
    assert_eq!(auto.by.to_string(), "Maya");
    assert_eq!(
        serde_json::to_value(&snapshot).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn task_work_preserves_sessions_graph_runs_and_graph() {
    let input = include_str!("../../../tests/fixtures/dto/task_work.json");
    let work: loopflow_execution::task_work::TaskWork = serde_json::from_str(input).unwrap();
    assert_eq!(work.sessions.len(), 2);
    assert_eq!(work.graph_processes.len(), 1);
    let graph = work.graph.as_ref().expect("fixture Task has a graph");
    assert_eq!(
        graph.position,
        loopflow_execution::task_graph::TaskGraphPosition::Run {
            step: "review-design".into(),
            at: "implement".into(),
            lf_process_id: work.graph_processes[0].summary.id.to_string(),
            running: true,
        }
    );
    assert_eq!(graph.step(), "review-design");
    assert!(graph.waiting_step().is_none());
    let line: Vec<_> = graph
        .steps
        .iter()
        .map(|step| (step.name.as_str(), step.interactive))
        .collect();
    assert_eq!(
        line,
        [
            ("plan", false),
            ("review-design", true),
            ("build", false),
            ("demo", true)
        ]
    );
    assert_eq!(graph.current_step, Some(2));
    assert_eq!(
        graph.sentence,
        "build is running implement. You are next needed at demo."
    );
    assert_eq!(
        serde_json::to_value(work).unwrap(),
        serde_json::from_str::<serde_json::Value>(input).unwrap()
    );
}

#[test]
fn context_report_keeps_unknown_sources_distinct_from_zero() {
    use loopflow_execution::context_usage::{ContextReport, ContextSource};
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
    let detail: loopflow_execution::durable::GraphProcessDetail =
        serde_json::from_str(input).unwrap();
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

/// Every state Rust can send, so Swift's decoder is tested against all of them.
#[test]
fn selection_states_fixture_lists_every_state() {
    use loopflow_execution::work::selection::SelectionState;
    fn collect(schema: &serde_json::Value, names: &mut Vec<String>) {
        match schema {
            serde_json::Value::Object(fields) => {
                for (key, value) in fields {
                    match (key.as_str(), value) {
                        ("const", serde_json::Value::String(name)) => names.push(name.clone()),
                        ("enum", serde_json::Value::Array(values)) => names
                            .extend(values.iter().filter_map(|v| v.as_str().map(str::to_owned))),
                        _ => collect(value, names),
                    }
                }
            }
            serde_json::Value::Array(values) => values.iter().for_each(|v| collect(v, names)),
            _ => {}
        }
    }
    let mut sent = Vec::new();
    collect(
        &serde_json::to_value(schemars::schema_for!(SelectionState)).unwrap(),
        &mut sent,
    );
    sent.sort();
    let mut fixture: Vec<String> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/selection_states.json"
    ))
    .unwrap();
    fixture.sort();
    assert_eq!(fixture, sent);
}

#[test]
fn subscribe_frames_carry_each_reads_json_result() {
    use loopflow_execution::read_stream::SubscriptionFrame;
    let entries: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/subscribe_frames.json"
    ))
    .unwrap();
    let envelope = |frame: &serde_json::Value| {
        let mut keys: Vec<_> = frame.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    };
    let written = serde_json::to_value(SubscriptionFrame::<()> {
        answers: None,
        unavailable: Some("gone".into()),
        body: None,
        work: None,
        read_at: None,
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
            "selections" => read::<Vec<loopflow_execution::work::selection::ViewPlan>>(&body),
            // Desktop's Session rows are pinned by `sessions.json`; the subscription carries them as listed.
            "sessions" => assert!(body
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["id"].is_string())),
            "task" => read::<loopflow::lf::commands::waves::DesktopTaskPane>(&body),
            "work_activity" => {
                read::<loopflow::lf::commands::activity::WorkActivitySnapshot>(&body)
            }
            "plan_home" => {
                assert_eq!(body["tracker"], serde_json::Value::Null);
                assert_eq!(body["plan"]["git"]["remote"], "origin");
                assert_eq!(body["plan"]["hub"]["machine"], "mini");
                assert_eq!(body["plan"]["hub"]["unsent"], 1);
                assert_eq!(body["plan"]["git"]["not_taken"]["count"], 3);
                assert!(body["plan"]["git"]["not_taken"]["reason"]
                    .as_str()
                    .unwrap()
                    .starts_with("wave design from mini "));
                assert_eq!(body["plan"]["hub"]["not_taken"], serde_json::Value::Null);
            }
            other => panic!("unknown subscription {other}"),
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
        loopflow_execution::durable::TaskState::Done
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
    use loopflow_execution::ops::task::TaskStatus;
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
            assert_eq!(
                execution.status,
                loopflow_execution::durable::TaskState::Done
            );
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
fn tracker_sync_preserves_sent_actions_and_losing_values() {
    use loopflow_execution::store::planning_sync::{ActionOwnerKind, TrackerKind};
    use loopflow_execution::store::sqlite::tracker_actions::ActionPhase;
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/tracker_sync.json"
    ))
    .unwrap();
    let sync: loopflow_execution::store::planning_sync::TrackerSync =
        serde_json::from_value(value.clone()).unwrap();
    assert_eq!(sync.actions[1].phase, ActionPhase::Sent);
    assert_eq!(sync.actions[2].change, "title");
    assert_eq!(sync.actions[2].local_value, "Local title");
    assert_eq!(
        sync.actions[2].tracker_value,
        Some(serde_json::json!("Linear title"))
    );
    assert_eq!(serde_json::to_value(&sync).unwrap(), value);
    assert_eq!(
        sync.lines()[2],
        r#"title · conflicted · lost: "Local title" · linear: "Linear title""#
    );
    assert!(sync.lines()[3].contains("linear: null"));
    assert_eq!(sync.actions[1].owner_kind, ActionOwnerKind::Project);
    assert_eq!(sync.actions[0].tracker, TrackerKind::Linear);
    let mut unknown = value;
    // Released Git lines are gone with Git planning.
    unknown["actions"][0]["tracker"] = serde_json::json!("git");
    assert!(
        serde_json::from_value::<loopflow_execution::store::planning_sync::TrackerSync>(unknown)
            .is_err()
    );
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
fn context_explanation_reports_the_tracker_sync_time() {
    use loopflow_execution::ops::context::ContextExplanation;

    let json = include_str!("../../../tests/fixtures/dto/context_explanation.json");
    let mut wire: serde_json::Value = serde_json::from_str(json).unwrap();
    let report: ContextExplanation = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(report.tracker_synced_at, Some(1_791_499_900));
    assert!(report
        .render()
        .lines()
        .any(|line| line == "Tracker synced: 1791499900"));
    assert_eq!(serde_json::to_value(report).unwrap(), wire);

    wire["tracker_synced_at"] = serde_json::Value::Null;
    let report: ContextExplanation = serde_json::from_value(wire.clone()).unwrap();
    assert!(report
        .render()
        .lines()
        .any(|line| line == "Tracker synced: unknown"));
    wire.as_object_mut().unwrap().remove("tracker_synced_at");
    assert!(serde_json::from_value::<ContextExplanation>(wire).is_err());
}

#[test]
fn task_run_explanations_preserve_actions_and_missing_evidence() {
    use loopflow_execution::ops::task::{TaskRunAction, TaskRunExplanation};
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
    assert!(links[0].route.host.is_none());
    assert_eq!(links[1].conversation, ConversationRelation::Unknown);
    assert!(links[1].url.contains("?repository="));
    assert_eq!(
        serde_json::to_value(links).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    let mut missing: serde_json::Value = serde_json::from_str(json).unwrap();
    missing[0].as_object_mut().unwrap().remove("fallback_argv");
    assert!(serde_json::from_value::<Vec<WorkLink>>(missing).is_err());
    let mut missing: serde_json::Value = serde_json::from_str(json).unwrap();
    missing[0]["route"].as_object_mut().unwrap().remove("host");
    assert!(serde_json::from_value::<Vec<WorkLink>>(missing).is_err());
}

#[test]
fn session_resume_explanations_preserve_intent_and_unavailable_evidence() {
    use loopflow::lf::commands::context::{SessionResumeExplanation, SessionResumeIntent};
    let wire: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/session_resume_explanations.json"
    ))
    .unwrap();
    let reports: Vec<SessionResumeExplanation> = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(
        reports[0].action.as_ref().unwrap().intent,
        SessionResumeIntent::Start
    );
    assert!(!reports[0].action.as_ref().unwrap().fork);
    assert_eq!(
        reports[1].action.as_ref().unwrap().intent,
        SessionResumeIntent::Resume
    );
    assert!(!reports[1].action.as_ref().unwrap().fork);
    assert!(reports[1].action.as_ref().unwrap().prepare_only);
    assert_eq!(
        reports[2].action.as_ref().unwrap().intent,
        SessionResumeIntent::View
    );
    assert!(!reports[2].action.as_ref().unwrap().fork);
    assert!(!reports[2].unavailable.is_empty());
    let new_thread = reports[4].action.as_ref().unwrap();
    assert_eq!(new_thread.intent, SessionResumeIntent::NewThread);
    assert_eq!(new_thread.agent.as_deref(), Some("codex"));
    assert!(reports[3].action.is_none());
    assert!(reports[3].state.is_none());
    assert!(reports[3].opening.is_none());
    assert_eq!(serde_json::to_value(&reports).unwrap(), wire);
    for field in ["resolution", "opening", "impediments", "unavailable"] {
        let mut missing = wire[0].clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<SessionResumeExplanation>(missing).is_err());
    }
}

#[test]
fn work_authors_preserve_names_and_historical_attribution() {
    let fixture = include_str!("../../../tests/fixtures/dto/work_authors.json");
    let authors: Vec<loopflow_execution::durable::Author> = serde_json::from_str(fixture).unwrap();
    assert_eq!(
        authors[1],
        loopflow_execution::durable::Author::Named("Fixture author".into())
    );
    assert_eq!(
        serde_json::to_value(authors).unwrap(),
        serde_json::from_str::<serde_json::Value>(fixture).unwrap()
    );
}

#[test]
fn session_steers_say_when_each_was_read() {
    let fixture = include_str!("../../../tests/fixtures/dto/session_steers.json");
    let page: loopflow_execution::steer::SessionSteerPage = serde_json::from_str(fixture).unwrap();
    assert_eq!(page.items[0].delivered_at, Some(1794000000));
    assert_eq!((page.items[1].delivered_at, page.pending), (None, 1));
    assert_eq!(
        serde_json::to_value(page).unwrap(),
        serde_json::from_str::<serde_json::Value>(fixture).unwrap()
    );
    // A page missing a nullable key is not a complete page.
    let partial = fixture.replace(r#""delivered_at": null"#, r#""unrelated": null"#);
    assert!(serde_json::from_str::<loopflow_execution::steer::SessionSteerPage>(&partial).is_err());
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
    use loopflow_execution::durable::RepositoryIdentity;
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
    use loopflow_execution::ops::task::{TaskPlanningAction, TaskPlanningExplanation};
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
    use loopflow_execution::ops::task::{TaskCheckoutBehavior, TaskCheckoutExplanation};
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
    use loopflow_execution::durable::{TaskLocation, TaskLocationObservation};
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
fn task_overview_excludes_diagnostic_process_history() {
    let input = include_str!("../../../tests/fixtures/dto/task_overview.json");
    let overview: loopflow_execution::task_work::TaskOverview =
        serde_json::from_str(input).unwrap();
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
    assert_eq!(
        window.workspaces[0].layout.children[0].attachment,
        Some(loopflow::lf::commands::desktop::DesktopOpeningStatus::Usable)
    );
    assert_eq!(window.workspaces[0].hidden_panes, ["files-pane"]);
    assert!(window.view_parts.is_empty());
    let parts = &reading.windows[1].view_parts;
    assert_eq!(
        parts
            .iter()
            .map(|part| part.name.as_str())
            .collect::<Vec<_>>(),
        ["backlog", "session", "tiles"]
    );
    assert!(parts[0].text.as_deref().unwrap().contains("All 0 · Mine 0"));
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
fn retention_summary_keeps_states_expiries_and_policies_exactly() {
    use loopflow_execution::transcript::summary::{AgeBucket, RetentionSummary};
    use loopflow_execution::transcript::RetentionState;
    use loopflow_harness::transcript::ExpiryPolicy;
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
    use loopflow_execution::ops::people::RepoPerson;
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
fn wave_removal_preview_round_trips() {
    use loopflow_execution::ops::wave_lifecycle::WaveRemoval;
    let json = include_str!("../../../tests/fixtures/dto/wave_removal.json");
    let removal: WaveRemoval = serde_json::from_str(json).unwrap();
    assert!(removal.moved[1].started);
    assert_eq!(
        serde_json::from_str::<WaveRemoval>(&serde_json::to_string(&removal).unwrap()).unwrap(),
        removal
    );
}

#[test]
fn wave_added_receipt_round_trips_with_its_leftover() {
    use loopflow_execution::ops::wave_lifecycle::WaveAdded;
    let json = include_str!("../../../tests/fixtures/dto/wave_added.json");
    let added: WaveAdded = serde_json::from_str(json).unwrap();
    assert!(added.created);
    assert_eq!(added.summary.len(), 1);
    assert_eq!(
        serde_json::to_value(&added).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn wave_renamed_receipt_round_trips_with_its_leftovers() {
    use loopflow_execution::ops::wave_lifecycle::WaveRenamed;
    let json = include_str!("../../../tests/fixtures/dto/wave_renamed.json");
    let renamed: WaveRenamed = serde_json::from_str(json).unwrap();
    assert!(renamed.renamed);
    assert_eq!(renamed.summary.len(), 1);
    assert_eq!(
        serde_json::to_value(&renamed).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn task_create_answers_with_the_conversation_record_and_row() {
    use loopflow::lf::commands::task_create::TaskCreated;
    let json = include_str!("../../../tests/fixtures/dto/task_create_receipt.json");
    let answers: Vec<TaskCreated> = serde_json::from_str(json).unwrap();
    let admitted = &answers[0];
    assert_eq!(admitted.receipt.milestone, "admitted");
    assert_eq!(admitted.receipt.graph.as_deref(), Some("code"));
    let session = admitted.session.as_ref().unwrap();
    assert_eq!(Some(&session.id), admitted.receipt.session_id.as_ref());
    assert!(!session.open_argv.is_empty());
    // The row a client selects before planning lists the Task.
    assert_eq!(answers[0].row.task.id, admitted.receipt.task.id.as_str());
    assert_eq!(
        answers[0].row.section,
        loopflow_execution::durable::TaskSection::Working
    );
    // A headless `-b` run has no conversation.
    assert_eq!(answers[1].receipt.session_id, None);
    assert!(answers[1].session.is_none());
    for key in ["session", "row", "parent"] {
        let mut absent: serde_json::Value = serde_json::from_str(json).unwrap();
        absent[1].as_object_mut().unwrap().remove(key);
        assert!(serde_json::from_value::<Vec<TaskCreated>>(absent).is_err());
    }
    assert_eq!(
        serde_json::to_value(&answers).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn wave_graph_reports_its_default_with_or_without_a_wave() {
    let json = include_str!("../../../tests/fixtures/dto/wave_graph.json");
    let graphs: Vec<loopflow_execution::ops::task::WaveGraph> = serde_json::from_str(json).unwrap();
    assert_eq!(graphs[0].default.source.as_str(), "repository");
    assert!(graphs[1].wave.is_none());
    assert_eq!(
        serde_json::to_value(&graphs).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn creation_recovery_dispositions_round_trip() {
    let json = include_str!("../../../tests/fixtures/dto/creation_lookup.json");
    let receipts: Vec<loopflow_execution::store::sqlite::creation::CreationReceipt> =
        serde_json::from_str(json).unwrap();
    assert_eq!(
        serde_json::to_value(receipts).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn opening_receipts_distinguish_focus_from_attachment() {
    use loopflow::lf::commands::open::{OpenReceipt, OpenStatus};
    let json = include_str!("../../../tests/fixtures/dto/open_receipts.json");
    let receipts: Vec<OpenReceipt> = serde_json::from_str(json).unwrap();
    assert_eq!(
        receipts
            .iter()
            .map(|receipt| receipt.status)
            .collect::<Vec<_>>(),
        [
            OpenStatus::Opening,
            OpenStatus::Usable,
            OpenStatus::Focused,
            OpenStatus::Failed
        ]
    );
    assert_eq!(
        serde_json::to_value(receipts).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}
