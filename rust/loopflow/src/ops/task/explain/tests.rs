//! Task run and checkout previews as table rows.
//!
//! `lf task run --explain` and `lf task checkout --explain` are dispatched in
//! `bin/lf.rs`: it maps the Clap flags to `TaskProcessOptions` (`-b`/`-i` to
//! `through`) and calls `lf::commands::context::explain_task_run` or
//! `explain_task_checkout`. Those open the existing registry read-only and call
//! `ops::task::explain_task_run` (→ `select_task_run` → `CapturedGraph`) or
//! `explain_task_checkout` at the process's cwd. Rows call the ops functions
//! with the checkout as cwd, and the `lf::commands::context` wrappers for an
//! absent or unreadable registry.
//!
//! Clap mapping, `--task` scoping, unchanged store bytes and launch refusing
//! what the preview names stay with the full-Machine tests in `tests/README.md`.

use std::fs;
use std::path::Path;

use crate::ops::context::ContextFact;
use crate::ops::human_session::tests::SessionHome;
use crate::ops::run::WorkSelection;
use crate::ops::task::{
    explain_task_checkout, explain_task_run, TaskCheckoutBehavior, TaskCheckoutExplanation,
    TaskProcessOptions, TaskRunAction, TaskRunExplanation,
};
use crate::store::SharedStore;
use crate::task_graph::CapturedGraph;
use crate::work::task::Task;

const GRAPHS: [(&str, &str); 5] = [
    ("proof", "- cmd: task sync --plan\n"),
    ("feature", "- graph: proof\n- interactive: {name: demo, id: review}\n"),
    ("findings", "- graph: proof\n- interactive: {name: research, id: findings}\n"),
    ("captured", "- implement\n- interactive: {name: demo, id: review}\n"),
    (
        "conditional",
        "- xor:\n    paths:\n      review:\n        description: Review it\n        steps:\n          - interactive: demo\n      skip:\n        description: Nothing to review\n",
    ),
];

/// One registered Task (`INF-123`) whose checkout is the fixture repository.
struct Table {
    repo: loopflow_test_support::TestRepo,
    store: SharedStore,
    task: Task,
    runtime: tokio::runtime::Runtime,
    home: SessionHome,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl Table {
    /// `branch` exists locally when `created`; restoration reads it.
    fn new(branch: &str, created: bool) -> Self {
        let lock = crate::journal::test_env_lock();
        let home = SessionHome::new();
        let repo = loopflow_test_support::TestRepo::new();
        crate::test_support::bind_task_planning(&repo);
        for (name, content) in GRAPHS {
            repo.create_file(&format!(".lf/graphs/{name}.yaml"), content);
        }
        repo.stage_all();
        repo.commit("Define the fixture Graphs");
        if created {
            repo.create_branch(branch);
        }
        let registered = crate::test_support::register_task_with_pr(
            home.path(),
            &repo.path().canonicalize().unwrap(),
            branch,
            &repo.head_sha(),
        );
        Self {
            repo,
            store: std::sync::Arc::new(registered.store),
            task: registered.task,
            runtime: tokio::runtime::Runtime::new().unwrap(),
            home,
            _lock: lock,
        }
    }

    fn wave_config(&self, graph: Option<&str>) {
        let mut config = "linear:\n  initiative: initiative-task-pr-tests\n".to_string();
        if let Some(graph) = graph {
            config.push_str(&format!("graph: {graph}\n"));
        }
        self.repo
            .create_file(".lf/wave/task-pr-tests/config.yaml", &config);
    }

    fn repository_graph(&self, graph: Option<&str>) {
        let mut config = "planning:\n  provider: linear\n  team: team-task-pr-tests\n".to_string();
        if let Some(graph) = graph {
            config.push_str(&format!("graph: {graph}\n"));
        }
        self.repo.create_file(".lf/config.yaml", &config);
    }

    /// The Task stands at `step` of the captured fixture Graph.
    fn capture(&self, step: &str) {
        let graph = CapturedGraph::load("captured", self.repo.path())
            .unwrap()
            .unwrap();
        let sqlite = &self.store.sqlite;
        sqlite.take_up_task_graph(&self.task.id, &graph).unwrap();
        assert!(sqlite.set_task_graph_step(&self.task.id, step).unwrap());
    }

    /// A Task with no checkout yet, as `lf task create` files it.
    fn unplaced(&self) -> Task {
        let mut task = self.task.clone();
        task.id = crate::work::task::TaskId::new();
        task.identifier = "INF-124".into();
        task.linear_id = None;
        task.worktree = None;
        crate::test_support::planning::seed_unplaced_task(
            &self.home.path().join("loopflow.db"),
            &task,
        );
        task
    }

    fn run(
        &self,
        task: Option<&str>,
        options: &TaskProcessOptions,
        through: bool,
    ) -> TaskRunExplanation {
        self.runtime.block_on(explain_task_run(
            &self.store,
            self.repo.path(),
            WorkSelection { task, wave: None },
            options,
            through,
        ))
    }

    fn checkout(
        &self,
        task: Option<&str>,
        options: &TaskProcessOptions,
    ) -> TaskCheckoutExplanation {
        self.runtime.block_on(explain_task_checkout(
            &self.store,
            self.repo.path(),
            WorkSelection { task, wave: None },
            options,
        ))
    }
}

fn none_options() -> TaskProcessOptions {
    TaskProcessOptions::default()
}

fn graph(name: &str) -> TaskProcessOptions {
    TaskProcessOptions {
        graph: Some(name.into()),
        ..Default::default()
    }
}

fn from(step: &str) -> TaskProcessOptions {
    TaskProcessOptions {
        from: Some(step.into()),
        ..Default::default()
    }
}

fn run(graph: &str, take_up: bool, from: &str, stops_at: Option<&str>) -> TaskRunAction {
    TaskRunAction::Run {
        graph: graph.into(),
        take_up,
        from: from.into(),
        stops_at: stops_at.map(Into::into),
    }
}

/// The action, or text of the impediment that refuses it.
type Expected = Result<TaskRunAction, &'static str>;

fn check(row: &str, report: &TaskRunExplanation, expected: Expected) {
    match (&report.action, expected) {
        (Some(action), Ok(expected)) => assert_eq!(action, &expected, "{row}"),
        (None, Err(reason)) => assert!(mentions(&report.impediments, reason), "{row}: {report:?}"),
        _ => panic!("{row}: {report:?}"),
    }
}

fn source(fact: &ContextFact) -> &str {
    match fact {
        ContextFact::Bound { source, .. } => source,
        other => panic!("unbound: {other:?}"),
    }
}

fn mentions(reasons: &[String], text: &str) -> bool {
    reasons.iter().any(|reason| reason.contains(text))
}

// ---- Task run: which Graph, from where, until where ----

#[test]
fn run_takes_up_the_named_else_waves_else_repositorys_else_code_graph() {
    let table = Table::new("explain-proof", true);
    table.wave_config(Some("feature"));
    let none = none_options();
    let rows: [(&str, &TaskProcessOptions, bool, Expected); 4] = [
        (
            "Wave default stops at its interactive step",
            &none,
            false,
            Ok(run("feature", true, "sync --plan", Some("review"))),
        ),
        (
            "-b runs through to the end",
            &none,
            true,
            Ok(run("feature", true, "sync --plan", None)),
        ),
        (
            "a named Graph stops inside its first xor",
            &graph("conditional"),
            false,
            Ok(run("conditional", true, "xor-route", Some("demo"))),
        ),
        (
            "an unknown Graph is an impediment",
            &graph("nonexistent"),
            false,
            Err("graph \"nonexistent\" is unavailable"),
        ),
    ];
    for (row, options, through, expected) in rows {
        check(row, &table.run(Some("INF-123"), options, through), expected);
    }
    // Without a Wave default, the repository's `graph:`, else the builtin.
    table.wave_config(None);
    for (repository, expected) in [(Some("findings"), "findings"), (None, "code")] {
        table.repository_graph(repository);
        let report = table.run(Some("INF-123"), &none, false);
        assert!(
            matches!(&report.action, Some(TaskRunAction::Run { graph, take_up: true, .. }) if graph == expected),
            "{expected}: {report:?}"
        );
    }
}

#[test]
fn run_on_a_captured_graph_waits_runs_or_ends_from_its_position() {
    let table = Table::new("explain-proof", true);
    let rows: [(&str, &str, TaskProcessOptions, bool, Expected); 6] = [
        (
            "nothing after the interactive step it waits at",
            "review",
            none_options(),
            false,
            Ok(TaskRunAction::End {
                graph: "captured".into(),
                take_up: false,
            }),
        ),
        (
            "at end a bare run points to --from",
            "end",
            none_options(),
            false,
            Err("--from <step>"),
        ),
        (
            "--from an interactive step waits there",
            "end",
            from("review"),
            false,
            Ok(TaskRunAction::Wait {
                graph: "captured".into(),
                take_up: false,
                step: "review".into(),
            }),
        ),
        (
            "--from a step that runs stops before the next interactive one",
            "end",
            from("implement"),
            false,
            Ok(run("captured", false, "implement", Some("review"))),
        ),
        (
            "-b --from an interactive step runs through it",
            "end",
            from("review"),
            true,
            Ok(run("captured", false, "review", None)),
        ),
        (
            "--from an unknown step",
            "end",
            from("missing"),
            false,
            Err("has no step \"missing\""),
        ),
    ];
    for (row, at, options, through, expected) in rows {
        table.capture(at);
        check(
            row,
            &table.run(Some("INF-123"), &options, through),
            expected,
        );
    }
    table.capture("end");
    let text = table.run(Some("INF-123"), &from("review"), false).render();
    assert!(
        text.contains("graph captured: wait at review; run nothing"),
        "{text}"
    );
    table.capture("review");
    let text = table.run(None, &none_options(), false).render();
    assert!(
        text.contains("graph captured: nothing remains; reach end"),
        "{text}"
    );
    assert!(text.contains("Nothing was executed"), "{text}");
}

#[test]
fn run_names_option_impediments_selection_and_missing_evidence() {
    let table = Table::new("explain-proof", true);
    let option = |directive: Option<&str>, name: Option<&str>| TaskProcessOptions {
        directive: directive.map(Into::into),
        name: name.map(Into::into),
        ..Default::default()
    };
    for (options, expected) in [
        (option(Some(" "), None), "directive cannot be empty"),
        (option(Some("new direction"), None), "already exists"),
        (
            option(None, Some("other-workspace")),
            "already uses workspace name",
        ),
        (option(None, Some("invalid/name")), "workspace name"),
    ] {
        let report = table.run(Some("INF-123"), &options, false);
        assert!(
            mentions(&report.impediments, expected),
            "{expected}: {report:?}"
        );
    }
    // Named or inferred from the checkout, the same Task and action.
    let explicit = table.run(Some("INF-123"), &none_options(), false);
    let inferred = table.run(None, &none_options(), false);
    assert_eq!(source(&explicit.resolution.task), "explicit");
    assert_eq!(source(&inferred.resolution.task), "checkout");
    assert_eq!(source(&explicit.resolution.execution_machine), "owner");
    assert_eq!(
        explicit.resolution.task,
        ContextFact::bound(&table.task.id, "explicit")
    );
    assert_eq!(
        (&explicit.action, &explicit.impediments),
        (&inferred.action, &inferred.impediments)
    );
    // A Task with no checkout reads definitions before preparation.
    let unplaced = table.unplaced();
    let report = table.run(Some(unplaced.id.as_str()), &none_options(), false);
    assert_eq!(
        source(&report.resolution.execution_machine),
        "owning_machine:this_machine"
    );
    assert!(
        mentions(&report.unavailable, "Future checkout"),
        "{report:?}"
    );
}

#[test]
fn previews_without_a_readable_registry_are_unavailable_and_create_nothing() {
    let _lock = crate::journal::test_env_lock();
    let home = SessionHome::new();
    let database = home.path().join("loopflow.db");
    let options = TaskProcessOptions::default();
    for corrupt in [false, true] {
        if corrupt {
            fs::write(&database, "not a database").unwrap();
        }
        let run =
            crate::lf::commands::context::explain_task_run(Some("UNKNOWN-1"), &options, false)
                .unwrap();
        assert!(run.action.is_none());
        assert!(matches!(
            run.resolution.task,
            ContextFact::Unavailable { .. }
        ));
        assert!(!run.unavailable.is_empty());
        let checkout =
            crate::lf::commands::context::explain_task_checkout(Some("UNKNOWN-1"), &options)
                .unwrap();
        assert!(checkout.action.is_none());
        match corrupt {
            true => assert_eq!(fs::read(&database).unwrap(), b"not a database"),
            false => assert!(!database.exists()),
        }
    }
}

// ---- Task checkout: reuse, restore or prepare ----

#[test]
fn checkout_reuses_a_present_checkout_and_names_option_impediments() {
    let table = Table::new("checkout-proof", true);
    let explicit = table.checkout(Some("INF-123"), &none_options());
    let action = explicit.action.clone().unwrap();
    assert_eq!(action.behavior, TaskCheckoutBehavior::Reuse);
    assert!(explicit.impediments.is_empty(), "{explicit:?}");
    assert!(explicit.render().contains("reuse recorded"));
    let inferred = table.checkout(None, &none_options());
    assert_eq!(source(&inferred.resolution.task), "checkout");
    assert_eq!(
        (&inferred.action, &inferred.impediments),
        (&explicit.action, &explicit.impediments)
    );
    for (options, expected) in [
        (
            TaskProcessOptions {
                name: Some("other-workspace".into()),
                ..Default::default()
            },
            "already uses workspace name",
        ),
        (
            TaskProcessOptions {
                directive: Some("new direction".into()),
                ..Default::default()
            },
            "already exists",
        ),
        (
            TaskProcessOptions {
                stack_on: Some("UNKNOWN-1".into()),
                ..Default::default()
            },
            "has no Task",
        ),
    ] {
        let report = table.checkout(Some("INF-123"), &options);
        assert!(
            mentions(&report.impediments, expected),
            "{expected}: {report:?}"
        );
    }
}

#[test]
fn checkout_restores_a_missing_checkout_unless_occupied_terminal_or_remote() {
    let table = Table::new("restore-proof", false);
    let checkout = table.repo.path().join("missing-checkout");
    let db = rusqlite::Connection::open(table.home.path().join("loopflow.db")).unwrap();
    db.execute(
        "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2)
         ON CONFLICT(task_id) DO UPDATE SET worktree=excluded.worktree",
        [table.task.id.as_str(), checkout.to_str().unwrap()],
    )
    .unwrap();
    let sqlite = &table.store.sqlite;
    let reset = || {
        db.execute("UPDATE tasks SET planning_state=NULL", [])
            .unwrap()
    };
    let explain = || table.checkout(Some(table.task.id.as_str()), &none_options());

    reset();
    let report = explain();
    assert_eq!(
        report.action.as_ref().unwrap().behavior,
        TaskCheckoutBehavior::Restore
    );
    assert!(report.impediments.is_empty(), "{report:?}");
    assert!(mentions(&report.unavailable, "must fetch"), "{report:?}");
    assert!(!checkout.exists());

    fs::create_dir(&checkout).unwrap();
    let report = explain();
    assert!(mentions(&report.impediments, "occupied"), "{report:?}");

    db.execute("UPDATE tasks SET planning_state='canceled'", [])
        .unwrap();
    let report = explain();
    assert!(
        mentions(&report.impediments, "terminal planning"),
        "{report:?}"
    );

    // A handoff is append-only, so the remote owner comes last.
    reset();
    let peer = crate::durable::MachineId::new();
    sqlite
        .add_machine(&peer, "must-not-contact.invalid", "peer")
        .unwrap();
    sqlite.move_claim(&table.task.id, &peer).unwrap();
    let report = explain();
    assert!(
        matches!(&report.resolution.execution_machine, ContextFact::Bound { value, .. } if value == peer.as_str()),
        "{report:?}"
    );
    assert!(
        mentions(&report.unavailable, "no remote preparation was performed"),
        "{report:?}"
    );
}

#[test]
fn checkout_proposes_a_path_for_a_task_without_one() {
    let table = Table::new("checkout-proof", true);
    let unplaced = table.unplaced();
    let report = table.checkout(Some(unplaced.id.as_str()), &none_options());
    let action = report.action.clone().unwrap();
    assert_eq!(action.behavior, TaskCheckoutBehavior::Prepare, "{report:?}");
    assert!(report.impediments.is_empty(), "{report:?}");
    assert!(!Path::new(&action.path).exists());
    assert!(
        mentions(&report.unavailable, "local Git facts"),
        "{report:?}"
    );
}
