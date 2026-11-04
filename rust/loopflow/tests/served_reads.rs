//! Native client operations through the real server match the CLI.

mod isolated_lf;
mod support;

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Duration;

use isolated_lf::{isolated_lf, isolated_read as read};
use loopflow_test_support::TestRepo;
use serde_json::Value;
use support::{register_task_with_pr, EnvGuard};

#[test]
fn server_operations_match_their_lf_commands() {
    use loopflow::lf::commands::activity::HistoryWindow;
    use loopflow::lf::commands::read::Read;
    use loopflow::lf::commands::write::Write;

    let home = tempfile::tempdir().unwrap();
    // Same HOME as `read`'s children: `portfolio` scans `$HOME/src`.
    let _env = EnvGuard::with_home(&[], Some(home.path()));
    // As the children resolve it: `doctor` prints the store's path.
    std::env::set_var("LF_HOME", home.path().canonicalize().unwrap());
    let repo = TestRepo::new();
    repo.create_file("edit.txt", "before\n");
    repo.stage_all();
    repo.commit("Comparison base");
    let base = repo.head_sha();
    repo.create_branch("served-reads");
    register_task_with_pr(home.path(), repo.path(), "served-reads", &base);
    repo.create_file("edit.txt", "working\n");
    let graph = support::record_graph(home.path(), repo.path(), "code", "implement", "succeeded");
    let task_diff = |path: Option<&str>, draft: Option<&str>| Read::TaskDiff {
        issue: "INF-123".into(),
        path: path.map(str::to_owned),
        base: "parent".into(),
        draft: draft.map(str::to_owned),
    };
    // Registration is a write: `lf` owns it, as at Desktop's workspace opening.
    let repository: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &["config", "repository", "--json"],
        None,
    ))
    .unwrap();
    // The Wave's authored files, seeded and committed once by `lf`: the add
    // compared below then changes nothing, in the app and again through `lf`.
    read(
        repo.path(),
        home.path(),
        &["wave", "add", "task-pr-tests", "--json"],
        None,
    );
    // Prepared, not started: a Session a bind preview can name.
    let session: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &["session", "resume", "--prepare", "--json"],
        None,
    ))
    .unwrap();
    // Unpublished, so it can complete: `lf task complete` waits on a PR's merge.
    let unpublished: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--no-wave",
            "--title",
            "Served writes",
            "--json",
        ],
        None,
    ))
    .unwrap();
    let task_id = unpublished["id"].as_str().unwrap().to_owned();
    let unpublished = unpublished["identifier"].as_str().unwrap().to_owned();
    // Unpublished too: abandoning it touches no PR, so needs no GitHub.
    let abandoned: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--no-wave",
            "--title",
            "Abandoned in the app",
            "--json",
        ],
        None,
    ))
    .unwrap();
    let abandoned = abandoned["identifier"].as_str().unwrap().to_owned();
    let folder = repo.path().file_name().unwrap().to_str().unwrap();
    // Planning status also serves a Work folder without Git.
    let work_folder = home.path().join("folder");
    fs::create_dir_all(&work_folder).unwrap();
    let history = || {
        Read::History(HistoryWindow {
            since: "7d".into(),
            limit: 50,
            wave: None,
            project: None,
            task: Some("INF-123".into()),
        })
    };
    let all_plans = || Read::Roadmap {
        directory: "/".into(),
        wave: None,
        task: None,
        all: true,
    };
    let sessions = |headless| Read::DesktopSessions {
        directory: repo.path().to_owned(),
        headless,
    };
    let task_pane = || Read::DesktopTask {
        directory: repo.path().to_owned(),
        issue: "INF-123".into(),
    };
    let plans = || Read::DesktopPlans {
        directory: repo.path().to_owned(),
    };
    // A new Read fails to compile here until it is compared too.
    let (reads, watches) = match task_diff(None, None) {
        Read::MonitorPs
        | Read::DesktopPlans { .. }
        | Read::DesktopSessions { .. }
        | Read::DesktopTask { .. }
        | Read::TaskChanges { .. }
        | Read::TaskDiff { .. }
        | Read::TaskFile { .. }
        | Read::Waves
        | Read::MachineId
        | Read::Portfolio
        | Read::PaneLayout { .. }
        | Read::People { .. }
        | Read::Graphs { .. }
        | Read::ViewPlan { .. }
        | Read::GraphProcesses { .. }
        | Read::GraphProcess { .. }
        | Read::Usage { .. }
        | Read::TaskContext { .. }
        | Read::WaveDetail { .. }
        | Read::ClosedTasks { .. }
        | Read::Machines
        | Read::History(_)
        | Read::Roadmap { .. }
        | Read::PlanningStatus { .. }
        | Read::WorkLink { .. }
        | Read::RepositoryPath { .. }
        | Read::WaveGraph { .. }
        | Read::SessionPage { .. }
        | Read::Tokens { .. }
        | Read::Retention
        | Read::Doctor
        | Read::BindPreview { .. } => (
            [
                Read::MonitorPs,
                plans(),
                sessions(true),
                task_pane(),
                Read::TaskChanges {
                    issue: "INF-123".into(),
                    base: "parent".into(),
                },
                task_diff(None, None),
                task_diff(Some("edit.txt"), None),
                task_diff(Some("edit.txt"), Some("draft\n")),
                Read::TaskFile {
                    issue: "INF-123".into(),
                    path: "edit.txt".into(),
                },
                Read::Waves,
                Read::MachineId,
                Read::Portfolio,
                Read::PaneLayout {
                    directory: repo.path().to_owned(),
                    work: "repository".into(),
                },
                Read::People {
                    directory: repo.path().to_owned(),
                },
                Read::Graphs {
                    directory: repo.path().to_owned(),
                },
                Read::ViewPlan {
                    directory: repo.path().to_owned(),
                    expression: None,
                },
                Read::ViewPlan {
                    directory: repo.path().to_owned(),
                    expression: Some("state:backlog".into()),
                },
                Read::GraphProcesses {
                    directory: repo.path().to_owned(),
                    task: "INF-123".into(),
                    after: None,
                },
                Read::GraphProcess { id: graph },
                Read::Usage {
                    days: 0,
                    wave: None,
                    project: None,
                    task: Some("INF-123".into()),
                },
                Read::Usage {
                    days: 30,
                    wave: Some("task-pr-tests".into()),
                    project: None,
                    task: None,
                },
                Read::TaskContext {
                    task: "INF-123".into(),
                },
                Read::WaveDetail {
                    directory: repo.path().to_owned(),
                    wave: "task-pr-tests".into(),
                },
                Read::ClosedTasks {
                    directory: repo.path().to_owned(),
                    wave: "task-pr-tests".into(),
                    after: None,
                },
                Read::Machines,
                history(),
                all_plans(),
                Read::Roadmap {
                    directory: repo.path().to_owned(),
                    wave: None,
                    task: Some("INF-123".into()),
                    all: false,
                },
                Read::PlanningStatus {
                    directory: repo.path().to_owned(),
                },
                Read::WorkLink {
                    directory: "/".into(),
                    url: format!("loopflow://{folder}/task/INF-123"),
                },
                Read::RepositoryPath {
                    id: serde_json::from_value(repository["id"].clone()).unwrap(),
                },
                Read::WaveGraph {
                    directory: repo.path().to_owned(),
                    wave: Some("task-pr-tests".into()),
                },
                Read::WaveGraph {
                    directory: repo.path().to_owned(),
                    wave: None,
                },
                Read::SessionPage {
                    directory: repo.path().to_owned(),
                    headless: true,
                    after: None,
                },
                Read::Tokens {
                    directory: repo.path().to_owned(),
                    days: None,
                },
                Read::Tokens {
                    directory: repo.path().to_owned(),
                    days: Some(1),
                },
                Read::Retention,
                Read::Doctor,
                Read::BindPreview {
                    session: session["id"].as_str().unwrap().into(),
                    task: "INF-123".into(),
                },
            ],
            [
                history(),
                Read::MonitorPs,
                all_plans(),
                plans(),
                sessions(false),
                sessions(true),
                Read::PlanningStatus {
                    directory: work_folder,
                },
                task_pane(),
            ],
        ),
    };
    let server = Server(isolated_lf(repo.path(), home.path(), &["server", "run"]));
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while loopflow::server::client::connect(loopflow::server::wire::Operation::Bootstrap).is_err() {
        assert!(std::time::Instant::now() < deadline, "server did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    let database = home.path().join("loopflow.db");
    // A server read changes no Work and records no Process.
    let unchanged = |argv: &[&str], serve: &mut dyn FnMut()| {
        let before = changes(&database);
        serve();
        assert_eq!(changes(&database), before, "{argv:?} wrote");
    };
    for served in reads {
        let owned = served.argv();
        let argv: Vec<&str> = owned.iter().map(String::as_str).collect();
        let printed = read(repo.path(), home.path(), &argv, served.input());
        let mut text = String::new();
        unchanged(&argv, &mut || text = server_read(served.clone()));
        let served = text;
        let Ok(printed) = serde_json::from_str::<Value>(&printed) else {
            // `config repository --path` prints a path, not JSON.
            assert_eq!(served.trim(), printed.trim(), "{argv:?}");
            continue;
        };
        let served: Value = serde_json::from_str(&served).unwrap();
        assert_eq!(without_clock(served), without_clock(printed), "{argv:?}");
    }
    for served in watches {
        let mut argv = served.argv();
        argv.push("--watch".into());
        let mut frame = Vec::new();
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        unchanged(&words, &mut || {
            let (_, response) =
                loopflow::server::client::connect(loopflow::server::wire::Operation::Subscribe {
                    query: served.clone(),
                    generation: 1,
                })
                .unwrap();
            let loopflow::server::wire::Reply::Reading { frame: reading, .. } = response.result
            else {
                panic!("unexpected subscription: {:?}", response.result);
            };
            frame = serde_json::to_vec(&reading).unwrap();
        });
        let served: Value = serde_json::from_slice(&frame).unwrap();
        assert!(served["unavailable"].is_null(), "{argv:?}: {served}");
        let printed = first_frame(repo.path(), home.path(), &argv);
        assert_eq!(without_clock(served), without_clock(printed), "{argv:?}");
    }

    let directory = repo.path().to_owned();
    let file_revision = || {
        let file: Value = serde_json::from_str(
            &Read::TaskFile {
                issue: "INF-123".into(),
                path: "edit.txt".into(),
            }
            .read()
            .unwrap(),
        )
        .unwrap();
        file["revision"].as_str().unwrap().to_owned()
    };
    let save = |revision: String| Write::TaskSave {
        directory: directory.clone(),
        issue: "INF-123".into(),
        path: "edit.txt".into(),
        revision,
        content: "saved\n".repeat(12_000),
    };
    let session = session["id"].as_str().unwrap().to_owned();
    // What a write changed, as the app reads it.
    let effect = |issue: &str| {
        without_clock(
            serde_json::from_str(
                &Read::Roadmap {
                    directory: directory.clone(),
                    wave: None,
                    task: Some(issue.into()),
                    all: false,
                }
                .read()
                .unwrap(),
            )
            .unwrap(),
        )
    };
    let repository_id = repository["id"].as_str().unwrap().to_owned();
    // A saved record as the app reads it, and the revision a write names.
    let layout = |work: &str| -> Value {
        serde_json::from_str(
            &Read::PaneLayout {
                directory: directory.clone(),
                work: work.into(),
            }
            .read()
            .unwrap(),
        )
        .unwrap()
    };
    let revision = |work: &str| layout(work)["revision"].as_i64().unwrap();
    let dashboard = || without_revision(layout("query:served"));
    let pane = |revision: i64| Write::PaneLayout {
        directory: directory.clone(),
        work: "repository".into(),
        layout: serde_json::json!({
            "work": {"repository": repository_id, "kind": "repository", "id": repository_id},
            "revision": revision,
            "tree": {"kind": "pane", "id": "session",
                     "content": {"kind": "session", "session_id": null, "location": null}},
            "focused": null,
            "zoomed": null,
            "hidden": [],
            "drafts": []
        })
        .to_string(),
    };
    let pane_revision = || {
        let saved = layout("repository");
        saved["revision"].as_i64().unwrap_or(0)
    };
    // A new Write fails to compile here until it is compared too.
    let (writes, arrangement) = match (Write::TaskReopen {
        directory: directory.clone(),
        issue: "INF-123".into(),
    }) {
        Write::TaskComplete { .. }
        | Write::TaskReopen { .. }
        | Write::TaskAbandon { .. }
        | Write::TaskNotes { .. }
        | Write::TaskOwnership { .. }
        | Write::ShipStop { .. }
        | Write::TaskMove { .. }
        | Write::TaskSave { .. }
        | Write::SessionRename { .. }
        | Write::SessionBind { .. }
        | Write::SessionResume { .. }
        | Write::SessionNew { .. }
        | Write::WaveAdd { .. }
        | Write::PlanningRefresh { .. }
        | Write::NewChapter { .. }
        | Write::GraphCustomize { .. }
        | Write::Portfolio { .. }
        | Write::DashboardQuery { .. }
        | Write::DashboardPin { .. }
        | Write::DashboardRename { .. }
        | Write::DashboardTile { .. }
        | Write::DashboardSlots { .. }
        | Write::TileMove { .. }
        | Write::PaneLayout { .. } => (
            [
                Write::TaskNotes {
                    directory: directory.clone(),
                    issue: "INF-123".into(),
                    notes: "Served from the app".into(),
                },
                Write::TaskMove {
                    directory: directory.clone(),
                    issue: "INF-123".into(),
                    wave: None,
                },
                Write::SessionRename {
                    directory: directory.clone(),
                    id: session.clone(),
                    name: "Served name".into(),
                },
                Write::SessionBind {
                    directory: directory.clone(),
                    id: session.clone(),
                    task: "INF-123".into(),
                },
                // The repository's conversation, one Session and its fresh
                // thread: prepared, never started.
                Write::SessionResume {
                    directory: directory.clone(),
                    id: None,
                    task: None,
                    wave: None,
                    agent: None,
                },
                Write::SessionResume {
                    directory: directory.clone(),
                    id: Some(session.clone()),
                    task: None,
                    wave: None,
                    agent: None,
                },
                Write::SessionNew {
                    directory: directory.clone(),
                    id: session.clone(),
                    agent: None,
                },
                Write::TaskComplete {
                    directory: directory.clone(),
                    issue: unpublished.clone(),
                },
                Write::TaskReopen {
                    directory: directory.clone(),
                    issue: unpublished.clone(),
                },
                Write::WaveAdd {
                    directory: directory.clone(),
                    wave: "task-pr-tests".into(),
                    objective: None,
                },
                Write::PlanningRefresh {
                    directory: directory.clone(),
                    wave: "task-pr-tests".into(),
                },
                Write::TaskAbandon {
                    directory: directory.clone(),
                    issue: abandoned.clone(),
                },
            ],
            // Each names the revision its last write left, so it is built again
            // before `lf` repeats it.
            [
                // The fixture's Working Tasks hold no Task; the shared order
                // moves through the same tile writer as a Dashboard's.
                Box::new(|| Write::TileMove {
                    directory: directory.clone(),
                    board: "query:served".into(),
                    member: format!("task:{task_id}"),
                    destination: format!("task:{task_id}"),
                }) as Box<dyn Fn() -> Write>,
                Box::new(|| Write::DashboardPin {
                    directory: directory.clone(),
                    id: "served".into(),
                    pinned: true,
                    revision: revision("query:served"),
                }),
                Box::new(|| Write::DashboardRename {
                    directory: directory.clone(),
                    id: "served".into(),
                    name: "Served board".into(),
                    revision: revision("query:served"),
                }),
                // A hidden tile is no longer one to hide: show it before each hide.
                Box::new(|| {
                    let tile = |hidden| Write::DashboardTile {
                        directory: directory.clone(),
                        id: "served".into(),
                        member: format!("task:{task_id}"),
                        hidden,
                    };
                    let _ = loopflow::server::client::connect(
                        loopflow::server::wire::Operation::Command {
                            change: tile(false),
                        },
                    );
                    tile(true)
                }),
                Box::new(|| Write::DashboardSlots {
                    directory: directory.clone(),
                    id: "served".into(),
                    revision: revision("query:served"),
                    slots: "null".into(),
                }),
                Box::new(|| Write::DashboardQuery {
                    directory: directory.clone(),
                    id: "served".into(),
                    expression: "state:backlog".into(),
                    revision: Some(revision("query:served")),
                }),
                Box::new(|| pane(pane_revision())),
            ],
        ),
    };
    // The same write through the server, then through `lf`: each prints the same,
    // leaves the same Work and records a Process the same way. A finished
    // Process nothing references is forgotten as it exits; its revision stays.
    let compare = |served: Write, again: &dyn Fn() -> Write, effect: &dyn Fn() -> Value| {
        let argv = served.argv();
        let before = processes(&database);
        let written = server_write(served);
        let recorded = processes(&database) - before;
        // Nothing it started outlives it: no provider, no child of the app.
        assert_eq!(children().trim(), server.0.id().to_string(), "{argv:?}");
        let effected = effect();
        let again = again();
        let args = again.argv();
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let before = processes(&database);
        let printed = read(repo.path(), home.path(), &args, again.input());
        assert_eq!(processes(&database) - before, recorded, "{argv:?}");
        assert_eq!(effect(), effected, "{argv:?}");
        match (
            serde_json::from_str::<Value>(&written),
            serde_json::from_str::<Value>(&printed),
        ) {
            (Ok(written), Ok(printed)) => assert_eq!(
                without_revision(without_save_time(without_clock(written))),
                without_revision(without_save_time(without_clock(printed))),
                "{argv:?}"
            ),
            _ => assert_eq!(written, printed, "{argv:?}"),
        }
        recorded
    };
    for write in writes {
        let issue = match &write {
            Write::TaskComplete { issue, .. }
            | Write::TaskReopen { issue, .. }
            | Write::TaskAbandon { issue, .. } => issue.clone(),
            _ => "INF-123".to_owned(),
        };
        let recorded = compare(write.clone(), &|| write.clone(), &|| effect(&issue));
        assert!(recorded > 0, "{:?} recorded no Process", write.argv());
    }
    // A save names the revision it replaces, which the first save changed.
    compare(save(file_revision()), &|| save(file_revision()), &|| {
        effect("INF-123")
    });
    // Claiming and unassigning leave the same owner, which each prints from the
    // store, and record a Process as `lf` does. Every claim appends to the
    // ownership log, so the log itself differs.
    for claimed in [true, false] {
        let ownership = Write::TaskOwnership {
            directory: directory.clone(),
            issue: "INF-123".into(),
            claimed,
        };
        let owner = |text: &str| serde_json::from_str::<Value>(text).unwrap()["owner"].clone();
        let before = processes(&database);
        let written = server_write(ownership.clone());
        let recorded = processes(&database) - before;
        let args = ownership.argv();
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let before = processes(&database);
        let printed = read(repo.path(), home.path(), &args, None);
        assert_eq!(processes(&database) - before, recorded, "{args:?}");
        assert!(recorded > 0, "{args:?} recorded no Process");
        assert_eq!(owner(&written), owner(&printed), "{args:?}");
        assert_eq!(owner(&written).is_null(), !claimed, "{written}");
    }
    // Stopping a Task that is not shipping saves the stop, which keeps a run
    // from approving it until `lf ship`, under the Process that made it. The
    // ship process it kills and the merge request it takes back are the ship
    // tests'.
    let stop = Write::ShipStop {
        directory: directory.clone(),
        issue: "INF-123".into(),
    };
    let stopped = || {
        let task = Read::DesktopTask {
            directory: directory.clone(),
            issue: "INF-123".into(),
        }
        .read()
        .unwrap();
        serde_json::from_str::<Value>(&task).unwrap()["execution"]["ship_stop"]["by"].clone()
    };
    assert!(compare(stop.clone(), &|| stop.clone(), &stopped) > 0);
    assert_eq!(stopped()["kind"], "person", "no stop saved");
    // Which repositories the app shows, as the app reads them.
    let portfolio =
        || without_clock(serde_json::from_str(&Read::Portfolio.read().unwrap()).unwrap());
    let manual = Write::Portfolio {
        change: vec!["manual".into()],
    };
    assert!(compare(manual.clone(), &|| manual.clone(), &portfolio) > 0);
    // A customized Graph is the builtin's copy in the repository.
    let customized = || {
        Value::from(
            fs::read_to_string(repo.path().join(".lf/graphs/code.yaml")).unwrap_or_default(),
        )
    };
    let customize = Write::GraphCustomize {
        directory: directory.clone(),
        name: "code".into(),
    };
    // `lf` writes its own copy, not the app's.
    let recustomize = || {
        fs::remove_file(repo.path().join(".lf/graphs/code.yaml")).unwrap();
        customize.clone()
    };
    assert_eq!(compare(customize.clone(), &recustomize, &customized), 0);
    // A chapter preview reads the rotation it would make and records nothing.
    let waves: Value = serde_json::from_str(&Read::Waves.read().unwrap()).unwrap();
    let wave_id = waves
        .as_array()
        .unwrap()
        .iter()
        .find(|wave| wave.to_string().contains("\"task-pr-tests\""))
        .and_then(|wave| wave["id"].as_str())
        .unwrap_or_else(|| panic!("no task-pr-tests Wave in {waves}"))
        .to_owned();
    let preview = Write::NewChapter {
        directory: directory.clone(),
        name: "October".into(),
        plan: serde_json::json!({
            "name": "October",
            "waves": [{
                "wave_id": wave_id,
                "successor_id": "proj_4f6c2b8e-3d5a-4c1e-9b7a-2e8d6f0a1c3b",
                "create": true,
                "project_name": "October",
                "krs": [{"text": "Desktop writes through the server", "holds": false}],
                "metric_targets": []
            }]
        })
        .to_string(),
        preview: true,
    };
    assert_eq!(
        compare(preview.clone(), &|| preview.clone(), &|| effect("INF-123")),
        0
    );
    // Arrangement persists without a Process, in the app as through `lf`.
    let create = Write::DashboardQuery {
        directory: directory.clone(),
        id: "served".into(),
        expression: format!("task:{task_id}"),
        revision: None,
    };
    let created = server_write(create);
    assert_eq!(layout("query:served")["id"], "served", "{created}");
    for write in arrangement {
        let recorded = compare(write(), &*write, &dashboard);
        assert_eq!(recorded, 0, "{:?} recorded a Process", write().argv());
    }
}

/// The processes this test started that are still running.
fn children() -> String {
    let output = std::process::Command::new("pgrep")
        .args(["-P", &std::process::id().to_string()])
        .output()
        .unwrap();
    String::from_utf8(output.stdout).unwrap()
}

/// A saved record without its revision, which each save advances.
fn without_revision(mut value: Value) -> Value {
    if let Some(fields) = value.as_object_mut() {
        fields.remove("revision");
    }
    value
}

/// Each Work domain's revision and the Processes recorded.
fn changes(database: &Path) -> (Vec<(String, i64)>, i64) {
    let conn = rusqlite::Connection::open(database).unwrap();
    let revisions = conn
        .prepare("SELECT domain, revision FROM store_revisions ORDER BY domain")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let processes = conn
        .query_row("SELECT count(*) FROM processes", [], |row| row.get(0))
        .unwrap();
    (revisions, processes)
}

/// A save without its recoveries' directories, which each save names by its time.
fn without_save_time(mut value: Value) -> Value {
    if let Some(recoveries) = value
        .pointer_mut("/file/recoveries")
        .and_then(Value::as_array_mut)
    {
        for recovery in recoveries {
            recovery.as_object_mut().unwrap().remove("directory");
        }
    }
    value
}

/// The `processes` revision: every Process recorded or forgotten moves it.
fn processes(database: &Path) -> i64 {
    let (revisions, _) = changes(database);
    revisions
        .into_iter()
        .find_map(|(domain, revision)| (domain == "processes").then_some(revision))
        .unwrap()
}

/// The first frame `lf <args>` prints, with its stdin held open as a watch's
/// parent holds it.
fn first_frame(repo: &Path, home: &Path, args: &[String]) -> Value {
    let mut child = isolated_lf(repo, home, args);
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    serde_json::from_str(&line).unwrap_or_else(|_| panic!("{args:?}: no frame"))
}

/// `value` without the times a reading stamps when it runs.
fn without_clock(mut value: Value) -> Value {
    match &mut value {
        Value::Object(fields) => {
            fields.retain(|key, field| {
                !(key == "evidence_age_secs"
                    || key == "generated_at"
                    || (key == "observed_at" && field.is_string()))
            });
            for field in fields.values_mut() {
                *field = without_clock(field.take());
            }
        }
        Value::Array(items) => {
            for item in items {
                *item = without_clock(item.take());
            }
        }
        _ => {}
    }
    value
}

struct Server(std::process::Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn server_read(query: loopflow::lf::commands::read::Read) -> String {
    let (_, response) =
        loopflow::server::client::connect(loopflow::server::wire::Operation::Read { query })
            .unwrap();
    match response.result {
        loopflow::server::wire::Reply::Read { output } => output,
        result => panic!("unexpected read: {result:?}"),
    }
}

fn server_write(change: loopflow::lf::commands::write::Write) -> String {
    let (_, response) =
        loopflow::server::client::connect(loopflow::server::wire::Operation::Command { change })
            .unwrap();
    match response.result {
        loopflow::server::wire::Reply::Command {
            output,
            stderr,
            code,
            ..
        } => {
            assert_eq!(code, 0, "{stderr}");
            output
        }
        result => panic!("unexpected command: {result:?}"),
    }
}
