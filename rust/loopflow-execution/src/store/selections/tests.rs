use crate::process::attachment;
use crate::store::selections;
use crate::task_graph::tests::ships_before_review;

use loopflow_model::durable::RepositoryId;
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::selection::{
    Arrangement, ArrangementMode, DashboardId, Selection, SelectionError, SelectionExpression,
    SelectionId, WorkWorkspaceKind,
};

fn view_plan(
    store: &SqliteStore,
    repo: &RepositoryId,
    query: Option<&str>,
) -> crate::store::StoreResult<loopflow_storage::work::selection::ViewPlan> {
    selections::view_plan(store, repo, query, &mut ships_before_review)
}

fn fixture() -> (tempfile::TempDir, SqliteStore, RepositoryId) {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&dir.path().join("db")).unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute_batch(include_str!(
            "../../../tests/fixtures/saved_selection_work.sql"
        ))
        .unwrap();
    let repo = RepositoryId::parse("repo_11111111111111111111111111111111").unwrap();
    (dir, store, repo)
}

fn selection() -> Selection {
    Selection {
        id: SelectionId::new(),
        expression: SelectionExpression::Repository,
    }
}

#[test]
fn selection_edits_can_repair_ambiguity_without_blocking_other_dashboards() {
    let (_dir, store, repo) = fixture();
    let id = DashboardId("picked".into());
    let saved = selections::edit_dashboard(&store, &repo, &id, "task:#1", None).unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE tasks SET issue_identifier='OTHER-1' WHERE issue_identifier='FIX-2'",
            [],
        )
        .unwrap();
    assert!(selections::read_dashboard(&store, &repo, &id).is_err());

    let independent = selections::edit_dashboard(
        &store,
        &repo,
        &DashboardId("backlog".into()),
        "state:backlog",
        None,
    )
    .unwrap();
    // FIX-1 is claimed with no checkout and FIX-3 unclaimed: both wait in Backlog.
    assert_eq!(independent.arrangement.order.len(), 2);
    let repaired =
        selections::edit_dashboard(&store, &repo, &id, "task:FIX-1", Some(saved.revision)).unwrap();
    assert_eq!(repaired.arrangement, saved.arrangement);
    assert_eq!(
        selections::read_dashboard(&store, &repo, &id)
            .unwrap()
            .unwrap()
            .saved,
        repaired
    );
}

#[test]
fn selection_revisit_recovers_authored_geometry_and_stale_edits_change_nothing() {
    let (_dir, store, repo) = fixture();
    let id = DashboardId("revisit".into());
    let first = selections::edit_dashboard(&store, &repo, &id, "state:open or state:backlog", None)
        .unwrap();
    let mut arranged = first.arrangement.clone();
    arranged.hide(&arranged.order[0].clone(), true).unwrap();
    arranged.order.reverse();
    let first = selections::save_dashboard(
        &store,
        &repo,
        &id,
        Some(first.revision),
        &first.selection,
        &arranged.to_edit(),
    )
    .unwrap();
    let next =
        selections::edit_dashboard(&store, &repo, &id, "state:done", Some(first.revision)).unwrap();
    let restored = selections::edit_dashboard(
        &store,
        &repo,
        &id,
        "  state:open   or state:backlog  ",
        Some(next.revision),
    )
    .unwrap();
    assert_eq!(restored.selection, first.selection);
    assert_eq!(restored.arrangement, first.arrangement);
    assert_eq!(restored.arrangement.order, arranged.order);
    let revisions = store.revisions().unwrap();
    assert!(
        selections::edit_dashboard(&store, &repo, &id, "state:deferred", Some(next.revision))
            .is_err()
    );
    assert!(selections::save_dashboard(
        &store,
        &repo,
        &id,
        Some(next.revision),
        &next.selection,
        &next.arrangement.to_edit()
    )
    .is_err());
    let invalid = Arrangement::automatic(&[]);
    assert!(selections::save_dashboard(
        &store,
        &repo,
        &id,
        Some(restored.revision),
        &restored.selection,
        &invalid.to_edit()
    )
    .is_err());
    assert_eq!(store.revisions().unwrap(), revisions);
    assert_eq!(
        selections::read_dashboard(&store, &repo, &id)
            .unwrap()
            .unwrap()
            .saved,
        restored
    );
    assert_eq!(
        store
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM dashboard_arrangements", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn dashboard_order_moves_under_its_revision_and_only_reordering_touches() {
    let (_dir, store, repo) = fixture();
    let id = DashboardId("ordered".into());
    let saved = selections::edit_dashboard(
        &store,
        &repo,
        &id,
        "task:FIX-1 or task:FIX-2 or task:FIX-3",
        None,
    )
    .unwrap();
    let [a, b, c] = [0, 1, 2].map(|i| saved.arrangement.order[i].clone());
    assert_eq!(saved.arrangement.mode, ArrangementMode::Auto);

    // Hiding and restoring keep the automatic order.
    let mut hidden = saved.arrangement.clone();
    hidden.hide(&b, true).unwrap();
    let hidden = selections::save_dashboard(
        &store,
        &repo,
        &id,
        Some(saved.revision),
        &saved.selection,
        &hidden.to_edit(),
    )
    .unwrap();
    assert_eq!(hidden.arrangement.mode, ArrangementMode::Auto);
    let mut restored = hidden.arrangement.clone();
    restored.hide(&b, false).unwrap();
    let restored = selections::save_dashboard(
        &store,
        &repo,
        &id,
        Some(hidden.revision),
        &saved.selection,
        &restored.to_edit(),
    )
    .unwrap();
    assert_eq!(restored.arrangement.mode, ArrangementMode::Auto);

    let mut edit = restored.arrangement.to_edit();
    edit.order.rotate_right(1);
    let save_order = || {
        selections::save_dashboard(
            &store,
            &repo,
            &id,
            Some(restored.revision),
            &restored.selection,
            &edit,
        )
    };
    let moved = save_order().unwrap();
    assert_eq!(moved.arrangement.order, [c.clone(), a.clone(), b.clone()]);
    assert_eq!(moved.arrangement.mode, ArrangementMode::Touched);
    assert_eq!(moved.revision, restored.revision + 1);

    let revisions = store.revisions().unwrap();
    assert!(save_order().is_err());
    assert_eq!(store.revisions().unwrap(), revisions);
    assert_eq!(
        selections::read_dashboard(&store, &repo, &id)
            .unwrap()
            .unwrap()
            .saved,
        moved
    );
}

#[test]
fn owned_unstarted_task_is_open_but_excluded_from_working_queries() {
    let (_dir, store, repo) = fixture();
    let task = "task_33333333333333333333333333333333";
    store.conn.lock().unwrap().execute(
        "INSERT INTO task_ownership(id,task_id,kind,person,machine_id,at,ordered_at,writer_machine_id,written_here)
         VALUES('own_33333333333333333333333333333333',?1,'recorded',NULL,NULL,2,2000,
             (SELECT id FROM machines WHERE is_local=1),1)",
        [task],
    ).unwrap();
    for expression in [
        None,
        Some("state:working"),
        Some("wave:Product state:working"),
    ] {
        let plan = view_plan(&store, &repo, expression).unwrap();
        assert_eq!(
            plan.resolution.task_ids(),
            ["task_22222222222222222222222222222222"]
        );
        assert!(plan.resolution.unavailable.is_empty());
    }
    for expression in [
        "state:open",
        "wave:Product state:open",
        "state:unstarted state:backlog",
    ] {
        let plan = view_plan(&store, &repo, Some(expression)).unwrap();
        assert!(
            plan.resolution.task_ids().iter().any(|id| id == task),
            "{expression}"
        );
        assert!(plan.resolution.unavailable.is_empty());
    }
}

#[test]
fn one_task_order_reorders_working_tasks_and_every_plan_and_arrivals_prepend() {
    let (_dir, store, repo) = fixture();
    let (one, two, three) = (
        "task_11111111111111111111111111111111",
        "task_22222222222222222222222222222222",
        "task_33333333333333333333333333333333",
    );
    // With a checkout, FIX-1 is idle and Working.
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO task_checkouts(task_id,worktree) SELECT id,'/repo/fix-1' FROM tasks WHERE issue_identifier='FIX-1'",
            [],
        )
        .unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute("UPDATE tasks SET planning_rank=0 WHERE id=?1", [two])
        .unwrap();
    assert_eq!(
        view_plan(&store, &repo, None)
            .unwrap()
            .resolution
            .task_ids(),
        [one, two]
    );
    assert_eq!(
        store.reorder_working_task(&repo, two, one, None).unwrap(),
        [two, one]
    );
    assert_eq!(
        view_plan(&store, &repo, None)
            .unwrap()
            .resolution
            .task_ids(),
        [two, one]
    );
    let plans = selections::selection_plans(&store, &repo, &mut ships_before_review).unwrap();
    let wave = plans
        .iter()
        .find(|plan| {
            plan.selection.id.0.starts_with("wave-default:") && plan.resolution.members.len() > 1
        })
        .unwrap();
    // The Wave strip lists Working Tasks; unstarted FIX-3 waits in Backlog.
    assert_eq!(wave.resolution.task_ids(), [two, one]);

    store
        .conn
        .lock()
        .unwrap()
        .execute_batch(&format!(
            "INSERT INTO task_events(task_id,kind_json,created_at) VALUES('{three}','{{\"kind\":\"started\"}}',2); INSERT INTO task_ownership(id,task_id,kind,person,machine_id,at,ordered_at,writer_machine_id,written_here) VALUES('own_33333333333333333333333333333333','{three}','recorded',NULL,NULL,2,2000,(SELECT id FROM machines WHERE is_local=1),1); INSERT INTO task_checkouts(task_id,worktree) VALUES('{three}','/repo/fix-3');"
        ))
        .unwrap();
    assert_eq!(
        view_plan(&store, &repo, None)
            .unwrap()
            .resolution
            .task_ids(),
        [three, two, one]
    );
    let revisions = store.revisions().unwrap();
    // A done Task is not a Working Tasks member, as either side of a drop; the
    // refusal names it and nothing is written.
    let done = "task_44444444444444444444444444444444";
    for (task, to) in [(done, one), (one, done)] {
        assert!(matches!(store.reorder_working_task(&repo, task, to, None),
            Err(crate::store::StoreError::Selection(SelectionError::NotWorking(id))) if id == done));
    }
    assert_eq!(store.revisions().unwrap(), revisions);
    assert_eq!(
        view_plan(&store, &repo, None)
            .unwrap()
            .resolution
            .task_ids(),
        [three, two, one]
    );
}

#[test]
fn sidebar_step_follows_the_captured_cursor_and_disappears_without_a_graph() {
    let (_dir, store, repo) = fixture();
    let id =
        loopflow_model::durable::TaskId::parse("task_22222222222222222222222222222222").unwrap();
    let row = || {
        view_plan(&store, &repo, None)
            .unwrap()
            .contents
            .into_iter()
            .find(|content| content.work.id == id.as_str())
            .unwrap()
    };
    assert_eq!(row().graph_step, None);
    let graph = crate::task_graph::CapturedGraph::load("code", _dir.path())
        .unwrap()
        .unwrap();
    use loopflow_storage::execution::WorkingGroup::{Active, Paused, Shipping};
    assert_eq!(row().working_group, Some(Active));
    let cursor = graph.find("implement").unwrap().cursor();
    store
        .take_up_task_graph(&id, &serde_json::to_string(&graph).unwrap())
        .unwrap();
    assert_eq!(row().graph_step, None, "before the first step");
    store.set_task_graph_step(&id, "end").unwrap();
    assert_eq!(row().graph_step, None, "after the last step");
    assert_eq!(row().working_group, Some(Shipping));
    store.set_task_graph_step(&id, "ship-review").unwrap();
    assert_eq!(row().working_group, Some(Active));
    store.set_task_graph_step(&id, "review").unwrap();
    assert_eq!(row().graph_step.as_deref(), Some("review"));
    let carrier = store.test_lf_process();
    let started = loopflow_storage::graph_process::ExecutionStart::reserved(1);
    let run_from = |step: &str, cursor: &str| {
        assert!(store
            .start_task_graph_run(&id, step, None, cursor, (&carrier, &started))
            .unwrap());
    };
    run_from("review", &serde_json::to_string(&cursor).unwrap());
    assert_eq!(row().graph_step.as_deref(), Some("implement"));
    assert_eq!(row().working_group, Some(Active));
    let deliver = serde_json::to_string(&graph.find("deliver").unwrap().cursor()).unwrap();
    store
        .advance_task_graph_run(&id, &carrier, &deliver)
        .unwrap();
    assert_eq!(row().working_group, Some(Shipping));
    let past_end = serde_json::to_string(&crate::graph::runner::ExecutionCursor {
        index: graph.steps.len(),
        ..Default::default()
    })
    .unwrap();
    store
        .advance_task_graph_run(&id, &carrier, &past_end)
        .unwrap();
    assert_eq!(row().graph_step.as_deref(), Some("review"));
    store.set_task_graph_step(&id, "end").unwrap();
    run_from("end", &past_end);
    assert_eq!(row().graph_step, None);
    store.conn.lock().unwrap().execute(
        "INSERT INTO task_pauses(task_id,paused_at,paused_by) VALUES(?1,1,json_object('kind','person','name','Ada'))", [id.as_str()]
    ).unwrap();
    let held = row();
    assert!(held
        .states
        .contains(&loopflow_storage::work::selection::SelectionState::Working));
    assert_eq!(held.working_group, Some(Paused));
}

#[test]
fn repository_and_wave_tiles_open_only_the_unfinished_primary() {
    let (_dir, store, repo) = fixture();
    let session = store.test_session("conversation", &crate::session_record::new_artifact_key());
    let db = || store.conn.lock().unwrap();
    db().execute(
        "INSERT INTO task_primary_sessions(task_id,session_id)
        SELECT id,?1 FROM tasks WHERE issue_identifier='FIX-2'",
        [&session.id],
    )
    .unwrap();

    let check = |primary: Option<&str>| {
        let tiles: Vec<_> = selections::selection_plans(&store, &repo, &mut ships_before_review)
            .unwrap()
            .into_iter()
            .flat_map(|plan| plan.contents)
            .filter(|content| content.identifier.as_deref() == Some("FIX-2"))
            .collect();
        assert_eq!(tiles.len(), 2);
        for tile in tiles {
            assert_eq!(tile.title, "Idle");
            assert_eq!(tile.location.unwrap().cwd, "/repo/fix-2");
            assert_eq!(tile.primary_session.as_deref(), primary);
            assert_eq!(
                tile.argv.and_then(|args| args.last().cloned()).as_deref(),
                primary
            );
        }
    };
    check(Some("conversation"));
    db().execute(
        "UPDATE agent_sessions SET completed_at=2 WHERE id=?1",
        [&session.id],
    )
    .unwrap();
    check(None);
}

#[test]
fn a_wave_scoped_reading_skips_repositories_without_that_wave() {
    let (_dir, store, _repo) = fixture();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_22222222222222222222222222222222','/other',1)",
            [],
        )
        .unwrap();
    let repos = ["/repo".to_string(), "/other".to_string()];
    let all = selections::all_view_plans(
        &store,
        Some("kind:task state:working"),
        &repos,
        &mut ships_before_review,
    )
    .unwrap();
    assert_eq!(
        all.iter()
            .map(|plan| plan.repo_path.as_str())
            .collect::<Vec<_>>(),
        ["/repo", "/other"]
    );
    let scoped = selections::all_view_plans(
        &store,
        Some("wave:11111111-1111-1111-1111-111111111111 kind:task state:working"),
        &repos,
        &mut ships_before_review,
    )
    .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].repo_path, "/repo");
    assert!(scoped[0].resolution.unavailable.is_empty());
}

#[test]
fn wave_default_plan_uses_three_slot_strip() {
    use loopflow_storage::work::selection::{SlotLayout, SlotNode, SplitAxis};
    let (_dir, store, repo) = fixture();
    let plans = selections::selection_plans(&store, &repo, &mut ships_before_review).unwrap();
    let wave = plans
        .iter()
        .find(|plan| {
            plan.selection
                .id
                .0
                .starts_with(loopflow_storage::sqlite::selections::WAVE_DEFAULT_PREFIX)
        })
        .expect("fixture has a Wave");
    assert_eq!(wave.slots, SlotLayout::strip(3));
    let SlotNode::Split { axis, weights, .. } = &wave.slots.root else {
        panic!("a strip splits");
    };
    assert_eq!(*axis, SplitAxis::Vertical);
    assert_eq!(weights, &[1.0; 3]);
    assert_eq!(wave.slots.slots(), [0, 1, 2]);
    // The repository page is a Wave view too.
    assert_eq!(plans[0].slots, SlotLayout::strip(3));
}

#[test]
fn resizing_two_slots_reweights_only_the_split_between_them() {
    use loopflow_storage::work::selection::{SlotLayout, SlotNode};
    let golden = SlotLayout::golden();
    // Slots 1 and 2 share the inner top split; the bottom band and slot 0 keep theirs.
    let resized = golden.resized(1, 2, 0.75).unwrap();
    let SlotNode::Split { children, .. } = &resized.root else {
        panic!("golden root splits");
    };
    let SlotNode::Split {
        children: top,
        weights: top_weights,
        ..
    } = &children[0]
    else {
        panic!("golden top splits");
    };
    assert_eq!(top_weights, &[0.618, 1.0 - 0.618]);
    let SlotNode::Split { weights, .. } = &top[1] else {
        panic!("slots 1 and 2 split");
    };
    assert_eq!(weights, &[1.5, 0.5]);
    let SlotNode::Split { children: band, .. } = &golden.root else {
        panic!("golden root splits");
    };
    assert_eq!(children[1], band[1]);
    // Slots 0 and 5 meet only at the root.
    let SlotNode::Split { weights, .. } = &golden.resized(0, 5, 0.5).unwrap().root else {
        panic!("golden root splits");
    };
    assert_eq!(weights, &[0.5, 0.5]);
    assert!(golden.resized(3, 3, 0.5).is_none());
    assert!(golden.resized(3, 9, 0.5).is_none());
}

#[test]
fn dashboard_slots_round_trip_survive_query_edits_and_reject_invalid_layouts() {
    use loopflow_storage::work::selection::{SlotLayout, SlotNode, SplitAxis};
    let (_dir, store, repo) = fixture();
    assert_eq!(
        view_plan(&store, &repo, Some("state:open")).unwrap().slots,
        SlotLayout::golden()
    );
    assert_eq!(SlotLayout::golden().slots().len(), 7);
    let id = DashboardId("slots".into());
    let saved = selections::edit_dashboard(&store, &repo, &id, "state:open", None).unwrap();
    assert_eq!(saved.slots, None);
    let query = format!("query:{}", id.0);
    assert_eq!(
        view_plan(&store, &repo, Some(&query)).unwrap().slots,
        SlotLayout::golden()
    );

    let two = SlotLayout {
        root: SlotNode::Split {
            axis: SplitAxis::Vertical,
            weights: vec![2.0, 1.0],
            children: vec![SlotNode::Slot { slot: 1 }, SlotNode::Slot { slot: 0 }],
        },
    };
    let arranged = store
        .set_dashboard_slots(&repo, &id, saved.revision, Some(&two))
        .unwrap();
    assert_eq!(arranged.revision, saved.revision + 1);
    assert_eq!(view_plan(&store, &repo, Some(&query)).unwrap().slots, two);
    let edited =
        selections::edit_dashboard(&store, &repo, &id, "state:backlog", Some(arranged.revision))
            .unwrap();
    assert_eq!(edited.slots, Some(two.clone()));

    let revisions = store.revisions().unwrap();
    let duplicate = SlotLayout {
        root: SlotNode::Split {
            axis: SplitAxis::Vertical,
            weights: vec![1.0, 1.0],
            children: vec![SlotNode::Slot { slot: 0 }, SlotNode::Slot { slot: 0 }],
        },
    };
    assert!(store
        .set_dashboard_slots(&repo, &id, edited.revision, Some(&duplicate))
        .is_err());
    assert!(store
        .set_dashboard_slots(&repo, &id, arranged.revision, None)
        .is_err());
    assert_eq!(store.revisions().unwrap(), revisions);
    let reset = store
        .set_dashboard_slots(&repo, &id, edited.revision, None)
        .unwrap();
    assert_eq!(reset.slots, None);
    assert_eq!(
        view_plan(&store, &repo, Some(&query)).unwrap().slots,
        SlotLayout::golden()
    );
}

#[test]
fn repository_queries_include_tasks_without_a_wave() {
    let (_dir, store, repo) = fixture();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE tasks SET project_id=NULL WHERE issue_identifier='FIX-2'",
            [],
        )
        .unwrap();
    let plan = view_plan(&store, &repo, Some("task:FIX-2")).unwrap();
    assert_eq!(plan.contents.len(), 1);
    assert_eq!(plan.contents[0].title, "Idle");
    assert!(view_plan(&store, &repo, None)
        .unwrap()
        .contents
        .iter()
        .any(|content| content.work == plan.contents[0].work));
    let wave = view_plan(&store, &repo, Some("wave:Product state:open")).unwrap();
    assert!(!wave
        .contents
        .iter()
        .any(|content| content.work == plan.contents[0].work));
}

#[test]
fn dashboards_leave_repository_and_wave_defaults_unchanged() {
    let (_dir, store, repo) = fixture();
    let defaults = selections::selection_plans(&store, &repo, &mut ships_before_review).unwrap();
    let id = DashboardId("backlog".into());
    let saved = selections::edit_dashboard(&store, &repo, &id, "state:backlog", None).unwrap();
    let named = store
        .set_dashboard_metadata(&repo, &id, saved.revision, Some("Later"), Some(true))
        .unwrap();
    let revisions = store.revisions().unwrap();
    let dashboard = view_plan(&store, &repo, Some("query:backlog")).unwrap();
    assert_eq!(dashboard.dashboard_id, Some(id.clone()));
    assert_eq!(dashboard.name.as_deref(), Some("Later"));
    assert!(dashboard.pinned);
    assert_eq!(dashboard.contents.len(), 2);
    assert!(dashboard.contents[0].primary_session.is_none());
    let plans = selections::selection_plans(&store, &repo, &mut ships_before_review).unwrap();
    assert_eq!(plans.len(), defaults.len() + 1);
    for (before, after) in defaults.iter().zip(&plans) {
        assert_eq!(before.selection, after.selection);
        assert_eq!(before.resolution, after.resolution);
        assert_eq!(before.arrangement, after.arrangement);
        assert!(after.dashboard_id.is_none());
    }
    assert_eq!(
        view_plan(&store, &repo, None).unwrap().resolution,
        defaults[0].resolution
    );
    assert_eq!(store.revisions().unwrap(), revisions);
    assert!(store
        .set_dashboard_metadata(&repo, &id, saved.revision, Some("stale"), Some(false),)
        .is_err());
    assert_eq!(store.dashboards(&repo).unwrap(), [named]);
    assert_eq!(store.revisions().unwrap(), revisions);
}

#[test]
fn query_selection_identical_saved_expressions_keep_distinct_sources() {
    let (_dir, store, repo) = fixture();
    for id in ["first", "second"] {
        selections::edit_dashboard(&store, &repo, &DashboardId(id.into()), "state:open", None)
            .unwrap();
    }
    let first = view_plan(&store, &repo, Some("query:first")).unwrap();
    let second = view_plan(&store, &repo, Some("query:second")).unwrap();
    assert_eq!(
        first.resolution.members[0].work,
        second.resolution.members[0].work
    );
    assert_ne!(
        first.resolution.members[0].source,
        second.resolution.members[0].source
    );
    let edited = selections::edit_dashboard(
        &store,
        &repo,
        &DashboardId("first".into()),
        "state:open or state:backlog",
        first.saved_revision,
    )
    .unwrap();
    assert_eq!(edited.arrangement.order[0], first.resolution.members[0]);
}

#[test]
fn query_selection_uses_current_reported_attention_and_prunes_ended_activity() {
    let (_dir, store, repo) = fixture();
    store.test_session(
        "selection-session",
        &crate::session_record::new_artifact_key(),
    );
    {
        let conn = store.conn.lock().unwrap();
        conn.execute("UPDATE agent_sessions SET repo='/repo', task_id='task_22222222222222222222222222222222', wave_id='11111111-1111-1111-1111-111111111111' WHERE id='selection-session'", []).unwrap();
    }
    let attachment = attachment::claim_session_attachment(
        &store,
        "selection-session",
        None,
        &store.test_lf_process(),
    )
    .unwrap();
    store
        .record_agent_process_identity(
            "selection-session",
            &attachment,
            std::process::id(),
            crate::process::os_process::process_started_at(std::process::id())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    store
        .begin_program_status(
            "selection-session",
            Some(&attachment.agent_process_id),
            "selection-surface",
        )
        .unwrap();
    let report = loopflow_model::program_status::Records {
        seen: true,
        records: vec![loopflow_model::program_status::Report {
            state: loopflow_model::program_status::State::Blocked,
            id: None,
            kind: Some(loopflow_model::program_status::Kind::Question),
            progress: None,
            app: None,
            title: None,
            msg: None,
        }],
    };
    store
        .record_program_status(
            "selection-session",
            Some(&attachment.agent_process_id),
            "selection-surface",
            1,
            &report,
        )
        .unwrap();
    let available = Selection::parse("state:available", None).unwrap();
    let members = selections::resolve_selection(&store, &repo, &available).unwrap();
    assert_eq!(members.members.len(), 1);
    assert_eq!(
        members.members[0].work.id,
        "task_22222222222222222222222222222222"
    );
    let default = selections::resolve_selection(&store, &repo, &selection()).unwrap();
    assert_eq!(default.members[0].work, members.members[0].work);
    let id = DashboardId("available".into());
    selections::save_dashboard(
        &store,
        &repo,
        &id,
        None,
        &available,
        &Arrangement::automatic(&members.members).to_edit(),
    )
    .unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE processes SET completed_at=2,outcome='succeeded' WHERE id=?1",
            [&attachment.agent_process_id],
        )
        .unwrap();
    loopflow_storage::query_plans::assert_no_empty_writes(&store, || {
        let ended = selections::read_dashboard(&store, &repo, &id)
            .unwrap()
            .unwrap();
        assert!(ended.resolution.unavailable.is_empty());
        assert!(ended.arrangement.order.is_empty());
        // The same Task is still open; only its current attention departed.
        assert_eq!(
            selections::resolve_selection(&store, &repo, &selection())
                .unwrap()
                .members
                .len(),
            3
        );
    });
}

#[test]
fn stale_conversations_leave_task_activity_known() {
    let (_dir, store, repo) = fixture();
    let task = "task_22222222222222222222222222222222";
    let running = Selection::parse("state:busy", None).unwrap();
    for id in ["never-attached", "before-boot"] {
        store.test_session(id, &crate::session_record::new_artifact_key());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET repo='/repo', task_id=?2, wave_id='11111111-1111-1111-1111-111111111111' WHERE id=?1",
                [id, task],
            )
            .unwrap();
    }
    let unattached = selections::resolve_selection(&store, &repo, &running).unwrap();
    let attachment =
        attachment::claim_session_attachment(&store, "before-boot", None, &store.test_lf_process())
            .unwrap();
    let started: i64 = store
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT started_at FROM processes WHERE id=?1",
            [&attachment.agent_process_id],
            |row| row.get(0),
        )
        .unwrap();
    crate::process::os_process::set_test_machine_booted_at(Some(started - 1));
    let unknown = selections::resolve_selection(&store, &repo, &running).unwrap();
    crate::process::os_process::set_test_machine_booted_at(Some(started + 1));
    let restarted = selections::resolve_selection(&store, &repo, &running).unwrap();
    crate::process::os_process::set_test_machine_booted_at(None);
    // Only the attached Process without OS identity, begun since boot, is unknown.
    assert!(unattached.unavailable.is_empty());
    assert_eq!(unknown.unavailable, ["Task FIX-2 activity is unavailable"]);
    assert!(restarted.unavailable.is_empty());
}

#[test]
fn a_published_pr_opens_a_task_that_waits_in_backlog_until_claimed() {
    let (_dir, store, repo) = fixture();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,
                parent_pr_id,publication_requested_at,created_at,updated_at)
             SELECT 'pr_backlog',id,1,'backlog','','',NULL,1,1,1
             FROM tasks WHERE issue_identifier='FIX-3'",
            [],
        )
        .unwrap();
    assert!(view_plan(&store, &repo, Some("state:backlog"))
        .unwrap()
        .contents
        .iter()
        .any(|content| content.title == "Backlog"));
    assert!(view_plan(&store, &repo, Some("state:open"))
        .unwrap()
        .contents
        .iter()
        .any(|content| content.title == "Backlog"));
}

#[test]
fn query_plans_include_backlog_without_creating_execution_and_keep_saved_edits() {
    let (_dir, store, repo) = fixture();
    let before = store.revisions().unwrap();
    let plan = view_plan(&store, &repo, Some("state:backlog")).unwrap();
    assert_eq!(plan.contents.len(), 2);
    let backlog = plan
        .contents
        .iter()
        .find(|content| content.title == "Backlog")
        .unwrap();
    assert_eq!(backlog.title, "Backlog");
    assert!(backlog.primary_session.is_none());
    assert!(backlog.location.is_none());
    assert!(backlog.argv.is_none());
    assert_eq!(backlog.unavailable.len(), 2);
    assert_eq!(store.revisions().unwrap(), before);
    let default = view_plan(&store, &repo, None).unwrap();
    assert_eq!(
        default.resolution.members[0].work.kind,
        WorkWorkspaceKind::Task
    );
    assert_eq!(
        default.resolution.members.last().unwrap().work.kind,
        WorkWorkspaceKind::Wave
    );
    let id = DashboardId("picked".into());
    let first =
        selections::edit_dashboard(&store, &repo, &id, "task:FIX-1 or task:FIX-2", None).unwrap();
    let original = first.arrangement.order.clone();
    let mut arrangement = first.arrangement.clone();
    arrangement.hide(&original[0], true).unwrap();
    arrangement.order.reverse();
    let touched = selections::save_dashboard(
        &store,
        &repo,
        &id,
        Some(first.revision),
        &first.selection,
        &arrangement.to_edit(),
    )
    .unwrap();
    let second = selections::edit_dashboard(
        &store,
        &repo,
        &id,
        "task:FIX-3 or task:FIX-1 or task:FIX-2",
        Some(touched.revision),
    )
    .unwrap();
    assert_eq!(&second.arrangement.order[..2], arrangement.order);
    assert!(second.arrangement.hidden.contains(&original[0]));
    assert!(
        selections::edit_dashboard(&store, &repo, &id, "state:open", Some(first.revision)).is_err()
    );
    assert!(
        selections::edit_dashboard(&store, &repo, &id, "query:picked", Some(second.revision))
            .is_err()
    );
    let plan = view_plan(&store, &repo, Some("query:picked")).unwrap();
    assert_eq!(plan.saved_revision, Some(second.revision));
    assert_eq!(plan.arrangement, second.arrangement);
    let plans = selections::selection_plans(&store, &repo, &mut ships_before_review).unwrap();
    let observed = plans
        .iter()
        .find(|plan| plan.dashboard_id.as_ref() == Some(&id))
        .unwrap();
    assert_eq!(observed.selection, plan.selection);
    assert_eq!(observed.resolution, plan.resolution);
    assert_eq!(observed.arrangement, plan.arrangement);
    assert_eq!(observed.contents, plan.contents);
    assert_eq!(store.dashboards(&repo).unwrap(), [second]);
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        conn.query_row("SELECT count(*) FROM agent_sessions", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM task_ownership WHERE kind!='recorded'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn released_work_upgrades_to_saved_intent_without_rewriting_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    crate::store::migrations::apply_before_migration(&conn, "saved_selections");
    conn.execute_batch(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../loopflow-storage/tests/fixtures/migrations/saved_selection_work.sql"
    )))
    .unwrap();
    crate::store::migrations::upgrade_from_migration(&conn, "saved_selections");
    // The idle Task has a checkout; the deferred one isn't Working.
    conn.execute(
        "INSERT INTO task_checkouts(task_id,worktree) SELECT id,'/repo/fix-2' FROM tasks WHERE issue_identifier='FIX-2' ON CONFLICT(task_id) DO UPDATE SET worktree=excluded.worktree",
        [],
    )
    .unwrap();
    drop(conn);
    let store = SqliteStore::from_fixture_connection(
        rusqlite::Connection::open(&path).unwrap(),
        crate::store::token_key::encryption_key,
    );
    let repo = RepositoryId::parse("repo_11111111111111111111111111111111").unwrap();
    let selection = selection();
    let result = selections::resolve_selection(&store, &repo, &selection).unwrap();
    assert_eq!(result.members.len(), 3);
    let id = DashboardId::new();
    let saved = selections::save_dashboard(
        &store,
        &repo,
        &id,
        None,
        &selection,
        &Arrangement::automatic(&result.members).to_edit(),
    )
    .unwrap();
    let reader =
        SqliteStore::open_read_only(&path, crate::store::token_key::encryption_key).unwrap();
    assert_eq!(
        selections::read_dashboard(&reader, &repo, &id)
            .unwrap()
            .unwrap()
            .saved,
        saved
    );
    let conn = store.conn.lock().unwrap();
    assert_eq!(
        conn.query_row("SELECT count(*) FROM tasks", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        4
    );
    loopflow_storage::test_fixture::validate_persisted_json(&conn).unwrap();
}

#[test]
fn saved_selections_upgrade_reload_and_edit_atomically_without_execution() {
    let (_dir, store, repo) = fixture();
    let selection = selection();
    let id = DashboardId::new();
    let result = selections::resolve_selection(&store, &repo, &selection).unwrap();
    assert!(result.unavailable.is_empty());
    assert_eq!(result.members.len(), 3); // Two Waves and the idle Task; FIX-1 has no checkout.
    assert_eq!(
        result
            .members
            .iter()
            .filter(|member| member.work.kind == WorkWorkspaceKind::Task)
            .map(|member| member.work.id.as_str())
            .collect::<Vec<_>>(),
        ["task_22222222222222222222222222222222"]
    );
    let hidden = result.members[0].clone();
    let mut arrangement = Arrangement::automatic(&result.members);
    arrangement.hide(&hidden, true).unwrap();
    let first =
        selections::save_dashboard(&store, &repo, &id, None, &selection, &arrangement.to_edit())
            .unwrap();
    // A second connection reads real lfdb bytes, without opening a native surface.
    let reader = SqliteStore::open_read_only(
        &_dir.path().join("db"),
        crate::store::token_key::encryption_key,
    )
    .unwrap();
    let loaded = selections::read_dashboard(&reader, &repo, &id)
        .unwrap()
        .unwrap();
    assert_eq!(loaded.saved, first);
    assert_eq!(loaded.arrangement, arrangement);
    arrangement.order.reverse();
    let saved = selections::save_dashboard(
        &store,
        &repo,
        &id,
        Some(first.revision),
        &selection,
        &arrangement.to_edit(),
    )
    .unwrap();
    assert!(selections::save_dashboard(
        &store,
        &repo,
        &id,
        Some(first.revision),
        &selection,
        &arrangement.to_edit()
    )
    .is_err());
    let incomplete = Arrangement::automatic(&result.members[1..]);
    assert!(selections::save_dashboard(
        &store,
        &repo,
        &id,
        Some(saved.revision),
        &selection,
        &incomplete.to_edit()
    )
    .is_err());
    assert_eq!(reader.dashboards(&repo).unwrap(), [saved]);
    let mut restored = selections::read_dashboard(&reader, &repo, &id)
        .unwrap()
        .unwrap()
        .arrangement;
    restored.hide(&hidden, false).unwrap();
    assert_eq!(restored.order, arrangement.order);
    let conn = store.conn.lock().unwrap();
    for table in ["agent_sessions", "processes", "graph_processes"] {
        assert_eq!(
            conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    loopflow_storage::test_fixture::validate_persisted_json(&conn).unwrap();
}

#[test]
fn nested_query_failure_retains_prior_members_but_confirmed_departure_prunes() {
    let (_dir, store, repo) = fixture();
    let base = selection();
    let base_id = DashboardId::new();
    let resolution = selections::resolve_selection(&store, &repo, &base).unwrap();
    selections::save_dashboard(
        &store,
        &repo,
        &base_id,
        None,
        &base,
        &Arrangement::automatic(&resolution.members).to_edit(),
    )
    .unwrap();
    let nested = Selection {
        id: SelectionId::new(),
        expression: SelectionExpression::Query {
            query: base_id.clone(),
        },
    };
    let nested_id = DashboardId::new();
    let result = selections::resolve_selection(&store, &repo, &nested).unwrap();
    let arrangement = Arrangement::automatic(&result.members);
    selections::save_dashboard(
        &store,
        &repo,
        &nested_id,
        None,
        &nested,
        &arrangement.to_edit(),
    )
    .unwrap();
    // The named query edit would introduce a cycle through the dependent.
    let cycle = Selection {
        id: SelectionId::new(),
        expression: SelectionExpression::Query {
            query: nested_id.clone(),
        },
    };
    assert!(selections::save_dashboard(
        &store,
        &repo,
        &base_id,
        Some(1),
        &cycle,
        &arrangement.to_edit()
    )
    .is_err());
    assert!(store.delete_dashboard(&repo, &base_id, 2).is_err());
    assert_eq!(
        selections::read_dashboard(&store, &repo, &nested_id)
            .unwrap()
            .unwrap()
            .arrangement,
        arrangement
    );
    assert!(selections::read_dashboard(&store, &repo, &base_id)
        .unwrap()
        .is_some());
    store.delete_dashboard(&repo, &base_id, 1).unwrap();
    assert!(selections::read_dashboard(&store, &repo, &base_id)
        .unwrap()
        .is_none());
    assert!(store.delete_dashboard(&repo, &base_id, 1).is_err());
    let unavailable = selections::read_dashboard(&store, &repo, &nested_id)
        .unwrap()
        .unwrap();
    assert!(!unavailable.resolution.unavailable.is_empty());
    assert_eq!(unavailable.arrangement, arrangement);
    selections::save_dashboard(
        &store,
        &repo,
        &base_id,
        None,
        &base,
        &Arrangement::automatic(&resolution.members).to_edit(),
    )
    .unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE tasks SET planning_state='completed' WHERE issue_identifier='FIX-2'",
            [],
        )
        .unwrap();
    let current = selections::read_dashboard(&store, &repo, &nested_id)
        .unwrap()
        .unwrap();
    assert!(current.resolution.unavailable.is_empty());
    assert_eq!(current.arrangement.order.len(), 2);
    assert_eq!(current.saved.arrangement.order.len(), 3); // Reading does not write.
}

#[test]
fn selection_reads_compose_with_the_work_snapshot_and_report_failed_reads() {
    let (_dir, store, repo) = fixture();
    let _snapshot = store.read_snapshot().unwrap();
    let result = selections::resolve_selection(&store, &repo, &selection()).unwrap();
    assert_eq!(result.members.len(), 3);
    let missing = RepositoryId::new();
    assert!(selections::resolve_selection(&store, &missing, &selection()).is_err());
}

#[test]
fn a_dashboard_changed_after_preparation_rejects_the_prepared_write() {
    let (_dir, store, repo) = fixture();
    let id = DashboardId("prepared".into());
    let mut prepared = selections::edit_dashboard(&store, &repo, &id, "task:#1", None).unwrap();
    // Another connection changes the Dashboard while the application owns its prepared value.
    let writer = rusqlite::Connection::open(_dir.path().join("db")).unwrap();
    writer.execute(
        "UPDATE dashboards SET name='later',revision=revision+1 WHERE repository_id=?1 AND id=?2",
        rusqlite::params![repo.as_str(), id.0],
    ).unwrap();
    prepared.revision += 1;
    assert!(matches!(
        store.commit_dashboard(&repo, Some(1), prepared),
        Err(crate::store::StoreError::Stale(_))
    ));
    let saved = selections::read_dashboard(&store, &repo, &id)
        .unwrap()
        .unwrap()
        .saved;
    assert_eq!(saved.name.as_deref(), Some("later"));
    assert_eq!(saved.revision, 2);
}

#[test]
fn a_view_plan_names_who_a_claim_here_would_make_owner() {
    let (_dir, store, repo) = fixture();
    let plan = view_plan(&store, &repo, None).unwrap();
    let viewer = plan.viewer.expect("the test home has a Git identity");
    assert_eq!(
        viewer.person_key,
        crate::config::participant_email(std::path::Path::new(&plan.repo_path))
    );
    assert_eq!(
        viewer.machine.id,
        Some(store.saved_local_machine().unwrap().id)
    );
}
