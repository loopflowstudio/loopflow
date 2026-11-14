//! Command observation is durable even when no agent work starts.
#![cfg(unix)]

#[allow(dead_code)] // Linked for its inherited provider-home scrub.
mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command};

use loopflow::id::LfProcessId;
use loopflow::session::{LfSession, TitleSource};
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::{open_ephemeral_store, StorageConfig};
use loopflow_test_support::{RunningParent, TestRepo};
use rusqlite::OptionalExtension;

#[tokio::test]
async fn agent_settlement_on_read_reports_failed_observation_and_recovers_without_losing_history() {
    use std::os::unix::process::CommandExt;

    use loopflow::lf::commands::top::ActivitySnapshot;

    struct Agent(Child);
    impl Drop for Agent {
        fn drop(&mut self) {
            if matches!(self.0.try_wait(), Ok(None)) {
                // SAFETY: this unreaped fixture child created its own process
                // group; only that owned group is signaled during cleanup.
                unsafe { libc::kill(-(self.0.id() as i32), libc::SIGKILL) };
            }
            let _ = self.0.wait();
        }
    }

    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let store = SqliteStore::new(&database).unwrap();
    reserve_session(&store, "orphan", repo.path());
    let sql = rusqlite::Connection::open(&database).unwrap();
    sql.execute(
        "UPDATE agent_sessions SET interactive=0 WHERE id='orphan'",
        [],
    )
    .unwrap();
    let parent = recorded_lf_process(&database);
    let attachment = store
        .claim_session_attachment("orphan", None, &parent, true)
        .unwrap();
    // Only this throwaway group can be signaled. The shell supplies the server
    // argv shape, not a configured provider, account or conversation.
    let started = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut agent = Agent(
        Command::new("/bin/sh")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .args([
                "-c",
                "sleep 60 & wait; :",
                "codex",
                "app-server",
                "--listen",
                "fixture",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap(),
    );
    sql.execute(
        "UPDATE processes SET pid=?2,os_started_at=?3,spawn_state='spawn_requested' WHERE id=?1",
        rusqlite::params![attachment.agent_process_id, agent.0.id(), started],
    )
    .unwrap();
    let _detached = store
        .release_session_attachment("orphan", &attachment)
        .unwrap();
    let before = store
        .agent_process(&attachment.agent_process_id)
        .unwrap()
        .unwrap();
    let history = store.session_history("orphan", 0, 0).unwrap();
    let snapshot = || {
        let output = command(home.path(), repo.path(), &["monitor", "ps", "--json"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        serde_json::from_slice::<ActivitySnapshot>(&output.stdout).unwrap()
    };
    let id = attachment.agent_process_id.to_string();
    // Every read of this Machine's processes recovers first, so the orphan is
    // checked in the store here rather than in a `monitor ps` row.
    assert!(before.completed_at.is_none());
    assert!(agent.0.try_wait().unwrap().is_none());

    // Only the portable reader invokes ps; macOS observes the kernel directly.
    #[cfg(not(target_os = "macos"))]
    {
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let ps = bin.join("ps");
        for (args, single_pid_reads) in [
            (["monitor", "prune", "--json"], false),
            // Exact leader observation succeeds, but descendant inventory fails.
            // The reaper must refuse before signaling, not assume no descendants.
            (["monitor", "prune", "--json"], true),
        ] {
            let forward = if single_pid_reads {
                "if [ \"$1\" = -p ]; then exec /bin/ps \"$@\"; fi\n"
            } else {
                ""
            };
            std::fs::write(
                &ps,
                format!("#!/bin/sh\n{forward}printf 'fixture sampling unavailable' >&2\nexit 2\n"),
            )
            .unwrap();
            std::fs::set_permissions(&ps, std::fs::Permissions::from_mode(0o755)).unwrap();
            let output = command(home.path(), repo.path(), &args)
                .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
                .output()
                .unwrap();
            assert!(
                !output.status.success(),
                "failed observation was reported successful: {output:?}"
            );
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("fixture sampling unavailable"),
                "{output:?}"
            );
            assert!(agent.0.try_wait().unwrap().is_none());
            assert_eq!(
                store.agent_process(&attachment.agent_process_id).unwrap(),
                Some(before.clone())
            );
            assert_eq!(
                store.session_attachment("orphan").unwrap(),
                Some(_detached.clone())
            );
            assert_eq!(store.session_history("orphan", 0, 0).unwrap(), history);
        }
    }

    // An ordinary read of this Machine's Tasks settles a detached row without receipts.
    let output = command(home.path(), repo.path(), &["task", "list"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(!agent
        .0
        .try_wait()
        .unwrap()
        .expect("reconciled provider exited")
        .success());
    assert!(snapshot().nodes.iter().all(|row| row.id != id));
    let ended = store
        .agent_process(&attachment.agent_process_id)
        .unwrap()
        .unwrap();
    assert!(ended.completed_at.is_some());
    assert!(
        ended.outcome.is_none(),
        "observed death is not successful work"
    );
    assert_eq!(ended.pid, before.pid);
    assert_eq!(ended.parent_lf_process_id, Some(parent));
    assert_eq!(store.session_history("orphan", 0, 0).unwrap(), history);
}

/// The Task gate and the activity list read one inventory: a held checkout
/// never names a Process the list omits, including an agent with no OS identity.
#[test]
fn task_checkout_blockers_are_rows_in_the_activity_list() {
    use loopflow::lf::commands::top::{ActivityNodeKind, ActivitySnapshot};

    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let lf = |cwd: &Path, args: &[&str]| {
        let mut command = command(home.path(), cwd, args);
        command
            .env("HOME", home.path())
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("LF_USER_NAME", "Fixture Person")
            .stdin(std::process::Stdio::null());
        command.output().unwrap()
    };
    let json = |cwd: &Path, args: &[&str]| {
        let output = lf(cwd, args);
        assert!(output.status.success(), "{args:?}: {output:?}");
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    let created = json(
        repo.path(),
        &["task", "create", "--title", "Hold a Task", "--json"],
    )["task"]
        .clone();
    let id = created["id"].as_str().unwrap();
    let placed = json(repo.path(), &["checkout", id, "--json"]);
    let worktree = Path::new(placed["worktree"].as_str().unwrap());

    let database = home.path().join("loopflow.db");
    let store = SqliteStore::new(&database).unwrap();
    reserve_session(&store, "held", worktree);
    let attachment = store
        .claim_session_attachment("held", None, &recorded_lf_process(&database), true)
        .unwrap();
    let agent = attachment.agent_process_id.as_str();

    let output = lf(repo.path(), &["monitor", "ps", "--json"]);
    assert!(output.status.success(), "{output:?}");
    let listed: ActivitySnapshot = serde_json::from_slice(&output.stdout).unwrap();
    let row = listed
        .nodes
        .iter()
        .find(|row| row.id == agent)
        .expect("reserved AgentProcess is listed");
    assert_eq!(row.kind, ActivityNodeKind::AgentProcess);

    let status = json(repo.path(), &["desktop", "task", id, "--json"]);
    assert!(status.to_string().contains(agent), "{status}");

    // Cancellation is the Task's decision; only checkout cleanup is held.
    let abandoned = lf(repo.path(), &["task", "abandon", id]);
    assert!(abandoned.status.success(), "{abandoned:?}");
    let reason = String::from_utf8_lossy(&abandoned.stderr);
    let named = reason
        .split("Process ")
        .skip(1)
        .filter_map(|rest| rest.split_whitespace().next())
        .collect::<Vec<_>>();
    assert!(named.contains(&agent), "{reason}");
    for id in named {
        assert!(
            listed.nodes.iter().any(|row| row.id == id),
            "blocker {id} is not a listed row: {reason}"
        );
    }
    assert!(worktree.exists());
}

#[cfg(not(target_os = "macos"))] // Failure injection replaces the portable ps reader.
#[tokio::test]
async fn activity_lists_unresolved_records_without_receipts_or_os_sampling() {
    use loopflow::lf::commands::top::{ActivitySnapshot, ActivityState};

    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let sql = rusqlite::Connection::open(database).unwrap();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let parent = LfProcessId::new();
    let agent = LfProcessId::new();
    sql.execute(
        "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,?2)",
        rusqlite::params![parent, now],
    )
    .unwrap();
    sql.execute(
        "INSERT INTO processes(id,trace_id,kind,parent_lf_process_id,started_at,pid,os_started_at)
         VALUES(?1,?1,'agent',?2,?3,4242,?3)",
        rusqlite::params![agent, parent, now],
    )
    .unwrap();
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let ps = bin.join("ps");
    std::fs::write(
        &ps,
        "#!/bin/sh\nprintf 'sampling unavailable' >&2\nexit 2\n",
    )
    .unwrap();
    std::fs::set_permissions(&ps, std::fs::Permissions::from_mode(0o755)).unwrap();

    let output = command(home.path(), home.path(), &["monitor", "ps", "--json"])
        .env("PATH", &bin)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let snapshot: ActivitySnapshot = serde_json::from_slice(&output.stdout).unwrap();
    for id in [&parent, &agent] {
        let node = snapshot
            .nodes
            .iter()
            .find(|node| node.id == id.as_str())
            .unwrap();
        assert_eq!(node.state, ActivityState::Unknown);
    }
    assert_eq!(
        snapshot
            .nodes
            .iter()
            .find(|node| node.id == agent.as_str())
            .unwrap()
            .parent_id,
        Some(parent.to_string())
    );
    let retained: i64 = sql
        .query_row(
            "SELECT count(*) FROM processes WHERE id IN (?1,?2) AND completed_at IS NULL",
            rusqlite::params![parent, agent],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(retained, 2, "observation grants no settlement authority");
}

#[tokio::test]
async fn process_discovery_pages_real_commands_and_preserves_unknown_history() {
    use loopflow::process::{ProcessEntry, ProcessPage};
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let connection = rusqlite::Connection::open(&database).unwrap();
    let parent = LfProcessId::new();
    let raw = serde_json::to_string(&["lf", "pr", "land", "--strict", "%_ literal"]).unwrap();
    connection
        .execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
            [&parent],
        )
        .unwrap();
    let ids = [LfProcessId::new(), LfProcessId::new()];
    for (index, id) in ids.iter().enumerate() {
        connection.execute("INSERT INTO processes(id,trace_id,parent_lf_process_id,via_agent,caller_session_id,command,started_at)
            VALUES(?1,?1,?2,?3,?4,?5,?6)", rusqlite::params![id,parent,index != 0,
                if index == 0 { None } else { Some("caller-session") },raw,10-index as i64]).unwrap();
    }
    let invoke = |args: &[&str]| {
        let result = command(home.path(), home.path(), args).output().unwrap();
        assert!(result.status.success(), "{result:?}");
        result.stdout
    };
    let first: ProcessPage = serde_json::from_slice(&invoke(&[
        "history",
        "list",
        "--all",
        "--parent",
        parent.as_str(),
        "--search",
        "PR LAND --strict %_",
        "--outcome",
        "unknown",
        "--limit",
        "1",
        "--json",
    ]))
    .unwrap();
    let entry = first.entries[0].as_lf().unwrap();
    assert_eq!(entry.id, ids[0]);
    assert_eq!(entry.via_agent, Some(false));
    assert_eq!(entry.command.as_ref(), Some(&raw));
    assert_eq!(entry.outcome, None);
    let cursor = serde_json::to_string(first.next.as_ref().unwrap()).unwrap();
    let second: ProcessPage = serde_json::from_slice(&invoke(&[
        "history",
        "list",
        "--all",
        "--parent",
        parent.as_str(),
        "--search",
        "PR LAND --strict %_",
        "--outcome",
        "unknown",
        "--limit",
        "1",
        "--after",
        &cursor,
        "--json",
    ]))
    .unwrap();
    assert_eq!(second.entries[0].id(), ids[1].as_str());
    assert_eq!(
        second.entries[0]
            .as_lf()
            .unwrap()
            .caller_session_id
            .as_deref(),
        Some("caller-session")
    );
    assert_eq!(second.next, None);
    let detail: ProcessEntry = serde_json::from_slice(&invoke(&[
        "history",
        "show",
        &ids[1].as_str()[..20],
        "--json",
    ]))
    .unwrap();
    assert_eq!(detail, second.entries[0]);
    // Both observations use the same text presentation, including unknown outcomes
    // and older records that have no command. JSON above retains the raw argv.
    let expected = format!("{}  9  unknown  lf pr land --strict %_ literal", ids[1]);
    for args in [
        vec!["history", "list", "--all", "--parent", parent.as_str()],
        vec!["history", "show", ids[1].as_str()],
        vec!["history", "list", "--all"],
    ] {
        let text = String::from_utf8(invoke(&args)).unwrap();
        assert!(text.contains(&expected), "{text}");
        if args == ["history", "list", "--all"] {
            assert!(
                text.contains(&format!("{parent}  1  unknown  unknown command")),
                "{text}"
            );
        }
    }
    let caller: ProcessPage = serde_json::from_slice(&invoke(&[
        "history",
        "list",
        "--all",
        "--caller",
        "caller-session",
        "--json",
    ]))
    .unwrap();
    assert_eq!(caller.entries, vec![detail]);
    let wave = loopflow::id::WaveId::new();
    let project = loopflow::durable::ProjectId::new();
    let task = loopflow::durable::TaskId::new();
    connection
        .execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'historical','/missing',1)",
            [&wave],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO repository_plans(id,repo,selected) VALUES(?1,'/missing',1)",
            [loopflow::durable::RepositoryId::new().as_str()],
        )
        .unwrap();
    connection.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project-proof',1)",
        rusqlite::params![project.as_str(),wave]).unwrap();
    connection
        .execute(
            "INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,created_at)
        VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue-proof','PROOF-1',1)",
            rusqlite::params![task.as_str(), project.as_str()],
        )
        .unwrap();
    connection.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id)
        VALUES('caller-session','Historical','human',1,1,'/missing',?1,?2)",
        rusqlite::params![task.as_str(),wave]).unwrap();
    connection.execute("INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,lf_process_id,task_id,wave_id,observed_at,payload)
        VALUES('caller-session','thread','turn','started','',?1,?2,?3,1,'unreadable history')",
        rusqlite::params![ids[1],task.as_str(),wave]).unwrap();
    for args in [
        vec!["history", "list", "--all", "--task", "PROOF-1", "--json"],
        vec![
            "history",
            "list",
            "--all",
            "--task",
            task.as_str(),
            "--json",
        ],
        vec!["history", "list", "--all", "--wave", "historical", "--json"],
    ] {
        let page: ProcessPage = serde_json::from_slice(&invoke(&args)).unwrap();
        assert_eq!(
            page.entries
                .iter()
                .map(ProcessEntry::id)
                .collect::<Vec<_>>(),
            vec![ids[1].as_str()]
        );
    }

    let repos = [TestRepo::new(), TestRepo::new()];
    let other_wave = loopflow::id::WaveId::new();
    let paths = repos
        .iter()
        .map(|repo| {
            loopflow::repository::CanonicalRepo::discover(repo.path())
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    connection
        .execute(
            "UPDATE waves SET repo=?2 WHERE id=?1",
            rusqlite::params![wave, paths[0]],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'historical',?2,1)",
            rusqlite::params![other_wave, paths[1]],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE processes SET repo=?2 WHERE id=?1",
            rusqlite::params![ids[1], paths[0]],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE processes SET repo=?2 WHERE id=?1",
            rusqlite::params![ids[0], paths[1]],
        )
        .unwrap();
    connection.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,wave_id)
        VALUES('other-session','Other','human',1,1,'/missing',?1)", [&other_wave]).unwrap();
    connection.execute("INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,lf_process_id,wave_id,observed_at,payload)
        VALUES('other-session','thread','turn','started','',?1,?2,1,'unreadable history')", rusqlite::params![ids[0],other_wave]).unwrap();
    for (repo, expected) in repos.iter().zip([&ids[1], &ids[0]]) {
        let result = command(
            home.path(),
            repo.path(),
            &["history", "list", "--wave", "historical", "--json"],
        )
        .output()
        .unwrap();
        assert!(result.status.success(), "{result:?}");
        let page: ProcessPage = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(
            page.entries
                .iter()
                .map(ProcessEntry::id)
                .collect::<Vec<_>>(),
            vec![expected.as_str()]
        );
    }
    let ambiguous = command(
        home.path(),
        repos[0].path(),
        &["history", "list", "--all", "--wave", "historical", "--json"],
    )
    .output()
    .unwrap();
    assert!(!ambiguous.status.success());
    assert!(String::from_utf8_lossy(&ambiguous.stderr).contains("Ambiguous Work selector"));
    let explicit: ProcessPage = serde_json::from_slice(&invoke(&[
        "history",
        "list",
        "--all",
        "--wave",
        other_wave.as_str(),
        "--json",
    ]))
    .unwrap();
    assert_eq!(explicit.entries[0].id(), ids[0].as_str());
    let zero = command(
        home.path(),
        home.path(),
        &["history", "list", "--limit", "0"],
    )
    .output()
    .unwrap();
    assert_eq!(zero.status.code(), Some(2));
}

#[test]
fn parser_returns_exact_status_without_admitting_an_early_store() {
    for (args, code) in [
        (vec!["--help"], 0),
        (vec!["--version"], 0),
        (vec!["session", "list", "--definitely-not-a-flag"], 2),
    ] {
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("loopflow.db");
        // An incompatible existing target must not be opened or repaired for help.
        std::fs::write(&database, b"retained incompatible store").unwrap();
        let output = command(home.path(), home.path(), &args)
            .env("PATH", "")
            .env("RUST_LOG", "off")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code), "{output:?}");
        assert!(!String::from_utf8_lossy(&output.stderr)
            .contains("Process history unavailable: no compatible process ledger"));
        assert_eq!(
            std::fs::read(&database).unwrap(),
            b"retained incompatible store"
        );
        assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 1);
        if code == 0 {
            assert!(!output.stdout.is_empty(), "{output:?}");
            assert!(output.stderr.is_empty(), "{output:?}");
        } else {
            assert!(String::from_utf8_lossy(&output.stderr).contains("unexpected argument"));
        }
    }
}

#[tokio::test]
async fn early_commands_record_exact_exits_without_initializing_or_migrating() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    drop(store);
    let parent = RunningParent::insert(&database);
    let conn = rusqlite::Connection::open(&database).unwrap();
    let schema: String = conn
        .query_row(
            "SELECT group_concat(sql) FROM sqlite_master ORDER BY name",
            [],
            |row| row.get(0),
        )
        .unwrap();
    for (args, code) in [
        (vec!["--help"], 0),
        (vec!["--version"], 0),
        (vec!["session", "list", "--definitely-not-a-flag"], 2),
    ] {
        let output = child(home.path(), home.path(), &args, &parent)
            .env("PATH", "")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code), "{output:?}");
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("Process history unavailable"),
            "{output:?}"
        );
    }
    let rows: Vec<(i32, String)> = conn
        .prepare(
            "SELECT exit_code,outcome FROM processes WHERE parent_lf_process_id=?1
             ORDER BY started_at,rowid",
        )
        .unwrap()
        .query_map([parent.id.as_str()], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        rows,
        // `--help` is the help read and records none.
        vec![(0, "succeeded".into()), (2, "failed".into())]
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM agent_sessions", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT group_concat(sql) FROM sqlite_master ORDER BY name",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        schema
    );
}

#[tokio::test]
#[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
async fn install_preflight_records_no_process() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    drop(store);
    let parent = RunningParent::insert(&database);
    let output = child(
        home.path(),
        home.path(),
        &["self", "install", "preflight", "--json"],
        &parent,
    )
    .output()
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    let conn = rusqlite::Connection::open(&database).unwrap();
    // Preflight only reads, so it records no Process.
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM processes WHERE parent_lf_process_id=?1",
            [parent.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn exit_observation_writes_only_agent_issued_processes() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let caller = serde_json::json!({
        "session_id": "session",
        "agent_process_id": null,
        "origin_exec_id": LfProcessId::new(),
    })
    .to_string();
    let rows = || -> Vec<bool> {
        rusqlite::Connection::open(&database)
            .unwrap()
            .prepare("SELECT via_agent FROM processes")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let rejected = ["task", "--no-such-flag"];

    let direct = command(home.path(), home.path(), &rejected)
        .output()
        .unwrap();
    assert!(!direct.status.success(), "{direct:?}");
    assert_eq!(rows(), Vec::<bool>::new(), "nothing saw it running");

    let hook = command(
        home.path(),
        home.path(),
        &["__context-block", "--moment", "start"],
    )
    .env("LF_AGENT_CALLER", &caller)
    .stdin(std::process::Stdio::null())
    .output()
    .unwrap();
    assert_eq!(rows(), Vec::<bool>::new(), "the harness ran it: {hook:?}");

    let issued = command(home.path(), home.path(), &rejected)
        .env("LF_AGENT_CALLER", &caller)
        .output()
        .unwrap();
    assert!(!issued.status.success(), "{issued:?}");
    assert_eq!(rows(), [true]);
}

#[test]
fn empty_release_check_returns_through_process_completion() {
    let repo = TestRepo::new();
    let tagged = Command::new("git")
        .args(["tag", "v0.9.0"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(tagged.status.success(), "{tagged:?}");
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let gh = bin.join("gh");
    std::fs::write(&gh, "#!/bin/sh\ncase \"$1 $2\" in '--version ') exit 0;; 'pr list') echo '[]'; exit 0;; esac\nexit 1\n").unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let database = home.path().join("loopflow.db");
    drop(SqliteStore::new(&database).unwrap());
    let parent = RunningParent::insert(&database);
    let output = child(home.path(), repo.path(), &["release", "check"], &parent)
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", bin.display()),
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No commits in the target area since the last tag."),
        "{output:?}"
    );
    assert!(!stderr.contains("Error:"), "{output:?}");
    // A release check is a read: it records no Process.
    let conn = rusqlite::Connection::open(&database).unwrap();
    let rows: i64 = conn
        .query_row(
            "SELECT count(*) FROM processes WHERE parent_lf_process_id=?1",
            [parent.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(rows, 0);
}

#[tokio::test]
async fn command_failure_records_the_process_result() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let repo = TestRepo::new();
    let parent = RunningParent::insert(&database);
    for args in [
        vec!["session", "rename", "missing", "Name"],
        vec![
            "--wave",
            "fixture-wave-does-not-exist",
            "session",
            "resume",
            "--prepare",
            "--json",
        ],
    ] {
        let result = child(home.path(), repo.path(), &args, &parent)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1), "{result:?}");
    }
    std::fs::write(repo.path().join("change.txt"), "retained work").unwrap();
    let lock = repo.path().join(".git/index.lock");
    std::fs::write(&lock, "retained lock").unwrap();
    let result = child(
        home.path(),
        repo.path(),
        &["commit", "-m", "fixture"],
        &parent,
    )
    .output()
    .unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert!(String::from_utf8_lossy(&result.stderr).contains("index.lock"));
    assert_eq!(std::fs::read_to_string(lock).unwrap(), "retained lock");
    let conn = rusqlite::Connection::open(database).unwrap();
    let failed: i64 = conn
        .query_row(
            "SELECT count(*) FROM processes
             WHERE parent_lf_process_id=?1 AND outcome='failed' AND exit_code=1",
            [parent.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        failed, 3,
        "resolution, dispatch and delivery failures retain their command outcomes"
    );
}

fn command(home: &Path, cwd: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LF_")
            || key.to_string_lossy().starts_with("LOOPFLOW_")
        {
            command.env_remove(key);
        }
    }
    command.args(args).current_dir(cwd).env("LF_HOME", home);
    command
}

fn child(home: &Path, cwd: &Path, args: &[&str], parent: &RunningParent) -> Command {
    let mut command = command(home, cwd, args);
    parent.adopt(&mut command);
    command
}

/// An attaching lf records its own row; fixtures attaching from outside one record it here.
fn recorded_lf_process(database: &Path) -> LfProcessId {
    let id = LfProcessId::new();
    rusqlite::Connection::open(database)
        .unwrap()
        .execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,unixepoch())",
            [&id],
        )
        .unwrap();
    id
}

fn reserve_session(store: &SqliteStore, session_id: &str, repo: &Path) {
    let run_id = uuid::Uuid::new_v4().simple().to_string();
    store
        .create_session(
            LfSession {
                captured: None,
                task_id: None,
                wave_id: None,
                graph_lf_process_id: None,
                work_source: None,
                bound_at: None,
                id: session_id.into(),
                artifact_key: Some(run_id),
                caller_artifact_key: None,
                input_published: false,
                cwd: repo.into(),
                skill: None,
                provider: Some("codex".into()),
                model: None,
                node: None,
                iterations: None,
                interactive: true,
                repo: None,
                title: "AgentProcess ownership".into(),
                title_source: TitleSource::Generated,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: 1,
            },
            None,
        )
        .unwrap();
}

#[tokio::test]
async fn monitor_prune_settles_proven_dead_processes_and_keeps_unknown_ones() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let conn = rusqlite::Connection::open(database).unwrap();
    let mut child = Command::new("sleep").arg("30").spawn().unwrap();
    let pid = child.id();
    child.kill().unwrap();
    child.wait().unwrap();
    let directory = home.path().join("runtime/exec-processes");
    std::fs::create_dir_all(&directory).unwrap();
    // Started since boot, so only the missing receipt link leaves it unknown.
    let now = time::OffsetDateTime::now_utc().unix_timestamp();

    for terminal in [false, true] {
        let id = LfProcessId::new();
        conn.execute(
            "INSERT INTO processes(id,trace_id,started_at,completed_at,outcome) VALUES(?1,?1,?2,?3,?4)",
            rusqlite::params![id, now, terminal.then_some(now), terminal.then_some("failed")],
        )
        .unwrap();
        // A PID-named receipt never linked to its row leaves the row's exit
        // unknown; the Process-named one proves it dead, so retention closes
        // and forgets the row and its receipt goes with it.
        for name in [pid.to_string(), id.to_string()] {
            let settled = terminal || name == id.to_string();
            let receipt = directory.join(format!("{name}.json"));
            let bytes = serde_json::to_vec(&serde_json::json!({
                "schema_version": 1, "trace_id": id, "exec_id": id,
                "pid": pid, "started_at": 1,
            }))
            .unwrap();
            for json in [false, true] {
                std::fs::write(&receipt, &bytes).unwrap();
                for dry_run in [true, false] {
                    let mut args = vec!["monitor", "prune"];
                    if dry_run {
                        args.push("--dry-run");
                    }
                    if json {
                        args.push("--json");
                    }
                    let output = command(home.path(), home.path(), &args).output().unwrap();
                    assert!(output.status.success(), "{output:?}");
                    assert_eq!(receipt.exists(), dry_run || !settled, "{args:?}");
                    if !dry_run {
                        let row: Option<Option<String>> = conn
                            .query_row("SELECT outcome FROM processes WHERE id=?1", [&id], |row| {
                                row.get(0)
                            })
                            .optional()
                            .unwrap();
                        assert_eq!(row, (!settled).then_some(None), "{args:?}");
                    }
                    if receipt.exists() {
                        assert_eq!(std::fs::read(&receipt).unwrap(), bytes);
                    }
                    if json {
                        let report: serde_json::Value =
                            serde_json::from_slice(&output.stdout).unwrap();
                        assert_eq!(report["dry_run"], dry_run);
                        assert_eq!(
                            report["removed_process_receipts"],
                            u32::from(!dry_run && settled)
                        );
                        assert_eq!(report["errors"], 0);
                    }
                }
            }
            if receipt.exists() {
                std::fs::remove_file(receipt).unwrap();
            }
        }
    }
}

#[tokio::test]
async fn historical_feedback_is_not_live_availability() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let store = SqliteStore::new(&database).unwrap();
    for id in ["waiting", "finished", "legacy_finished", "unobserved"] {
        reserve_session(&store, id, home.path());
    }
    let finished = store
        .claim_session_attachment("finished", None, &recorded_lf_process(&database), true)
        .unwrap();
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute(
            r#"INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('waiting','observed','legacy_review_feedback',1,'{"type":"legacy_review_feedback","summary":"Review the API"}')"#,
            [],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE agent_sessions SET completed_at=2 WHERE id='legacy_finished'",
            [],
        )
        .unwrap();
    connection.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
    connection
        .execute(
            "UPDATE processes SET completed_at=2 WHERE id=?1",
            [&finished.agent_process_id],
        )
        .unwrap();
    for id in ["waiting", "finished", "legacy_finished", "unobserved"] {
        let output = command(home.path(), home.path(), &["session", "card", id, "--json"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let card: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let session = &card["session"];
        assert!(session["availability"].is_null(), "{session}");
        assert_ne!(session["state"], "active", "{session}");
    }
    for args in [
        vec!["process", "list", "--help"],
        vec!["process", "top", "--help"],
        vec!["process", "--help"],
    ] {
        let help = command(home.path(), home.path(), &args).output().unwrap();
        assert!(help.status.success(), "{help:?}");
        assert!(String::from_utf8_lossy(&help.stdout).contains("process"));
    }
}

#[tokio::test]
async fn inspection_records_no_process_and_starts_no_work() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let parent = RunningParent::insert(&database);
    let output = child(
        home.path(),
        home.path(),
        &["session", "list", "--all", "--json"],
        &parent,
    )
    .output()
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    let connection = rusqlite::Connection::open(&database).unwrap();
    let count: i64 = connection
        .query_row(
            "SELECT count(*) FROM processes WHERE parent_lf_process_id=?1",
            [parent.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0, "a read records no Process");
    let work: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='runs'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(work, 0, "inspection must not reserve agent or Task work");
}
