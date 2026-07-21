//! `lf status` is an audit surface, so its contract is user-facing: the JSON it
//! promises must be the JSON it emits, and the wave you are standing in must be
//! the wave it reports. Drives the real binary against a seeded `LF_HOME`.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use loopflow::child::ChildRef;
use loopflow::durable::{Containment, RunAdvance, RunTrigger};
use loopflow::id::WaveId;
use loopflow::planning::{LinearProjectId, ProjectPlan};
use loopflow::project::{Project, ProjectId};
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::{PmSnapshotRow, RunEventRow};
use loopflow::trace::{AgentInvocationRow, AgentTurnRow};
use loopflow::wave::Wave;
use time::OffsetDateTime;

/// A machine home holding one wave with lookup noise, a flow, and a skill. The
/// registry and the ledgers are the same database.
fn seed(home: &Path, wave_name: &str) -> Wave {
    std::fs::create_dir_all(home).expect("home");
    let db = home.join("loopflow.db");
    let store = SqliteStore::new(&db).expect("open store");
    let wave = Wave::new(
        WaveId::new(),
        wave_name.to_string(),
        home.join("repo").display().to_string(),
    );
    store.create_wave(&wave).expect("register wave");

    let now = chrono::Utc::now().timestamp();
    let event = |seq: i64, ts: i64, event: &str| RunEventRow {
        run_id: "run-1".to_string(),
        process_id: "proc-lookup".to_string(),
        parent_process_id: None,
        seq,
        ts,
        repo: Some(home.join("repo").display().to_string()),
        worktree: None,
        wave: Some(wave_name.to_string()),
        node: "run".to_string(),
        event: event.to_string(),
        command: Some(r#"["lf","pm","sync"]"#.to_string()),
        flow: None,
        skill: None,
        step_index: None,
        error: None,
    };
    store
        .insert_run_event(&event(0, now - 120, "started"))
        .expect("seed run start");
    store
        .insert_run_event(&event(1, now - 60, "completed"))
        .expect("seed run end");

    let mut flow_start = event(1, now - 50, "started");
    flow_start.run_id = "run-flow".to_string();
    flow_start.process_id = "proc-flow".to_string();
    flow_start.node = "flow".to_string();
    flow_start.command = Some(r#"["lf","build"]"#.to_string());
    flow_start.flow = Some("build".to_string());
    store
        .insert_run_event(&flow_start)
        .expect("seed flow start");
    let mut flow_end = flow_start.clone();
    flow_end.seq = 2;
    flow_end.ts = now - 40;
    flow_end.event = "completed".to_string();
    store.insert_run_event(&flow_end).expect("seed flow end");

    // The run boundary the resident invocation below rides on. A real agent invocation
    // always sits inside a run, so its trace is reachable by process or run id.
    let mut resident_start = event(0, now - 30, "started");
    resident_start.run_id = "run-resident".to_string();
    resident_start.process_id = "proc-resident".to_string();
    resident_start.command = Some(r#"["lf","__resident"]"#.to_string());
    store
        .insert_run_event(&resident_start)
        .expect("seed resident start");
    let mut resident_end = resident_start.clone();
    resident_end.seq = 1;
    resident_end.ts = now - 20;
    resident_end.event = "completed".to_string();
    store
        .insert_run_event(&resident_end)
        .expect("seed resident end");

    let invocation = AgentInvocationRow {
        id: "invocation-wave-mutate".to_string(),
        run_id: "run-resident".to_string(),
        answer_ask_id: None,
        process_id: "proc-resident".to_string(),
        started_at: now - 30,
        ended_at: Some(now - 20),
        repo: home.join("repo").display().to_string(),
        worktree: home.join("repo").display().to_string(),
        wave: Some(wave_name.to_string()),
        flow: Some("wave".to_string()),
        skill: Some("wave_mutate".to_string()),
        project: Some("auditability".to_string()),
        task: Some("W2-122".to_string()),
        provider: "codex".to_string(),
        model: Some("gpt-5".to_string()),
        surface: "headless".to_string(),
        capture_status: "complete".to_string(),
        incomplete_reason: None,
        outcome: "completed".to_string(),
        artifact_dir: "traces/invocation-wave-mutate".to_string(),
        conversation_path: "traces/invocation-wave-mutate/conversation.jsonl".to_string(),
        provider_events_path: None,
        provider_session_id: None,
        provider_session_path: None,
        conversation_event_count: 2,
        conversation_bytes: 10,
        supervision: None,
    };
    let turn = AgentTurnRow {
        id: "turn-wave-mutate".to_string(),
        invocation_id: invocation.id.clone(),
        ordinal: 1,
        provider_turn_id: None,
        started_at: now - 30,
        ended_at: Some(now - 20),
        status: "completed".to_string(),
        input_op: "initial".to_string(),
        context_coverage: "assembled".to_string(),
        tokenizer: "o200k_base".to_string(),
        system_prompt_path: None,
        task_prompt_path: "traces/invocation-wave-mutate/task.md".to_string(),
        system_tokens: 0,
        task_tokens: 10,
        supplied_context_tokens: 10,
        provider_input_tokens: Some(10),
        provider_total_input_tokens: Some(10),
        peak_input_tokens: Some(10),
        context_window_tokens: Some(100),
        provider_output_tokens: Some(5),
        reasoning_tokens: None,
        cache_read_tokens: Some(0),
        cache_write_tokens: None,
        cost_usd: Some(0.01),
        context_gather_ms: 1,
        context_render_ms: 1,
        context_persist_ms: 1,
        first_event_seq: None,
        last_event_seq: None,
        root_output: None,
        basis: None,
    };
    store
        .insert_trace_capture(&invocation, &turn, &[], &[])
        .expect("seed skill invocation");
    wave
}

/// `lf status --json` in a clean environment, optionally standing inside a wave.
fn status_json(home: &Path, args: &[&str], ambient_wave_id: Option<&str>) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .arg("status")
        .args(args)
        .arg("--json")
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_WAVE_ID");
    if let Some(id) = ambient_wave_id {
        command.env("LF_WAVE_ID", id);
    }
    prepend_test_bin(&mut command, home);
    let output = command.output().expect("lf status runs");
    assert!(
        output.status.success(),
        "lf status failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    serde_json::from_str(stdout.trim()).unwrap_or_else(|err| panic!("not JSON: {err}\n{stdout}"))
}

fn status_human(home: &Path, wave: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["status", wave])
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .output()
        .expect("lf status runs");
    assert!(
        output.status.success(),
        "lf status failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("status is utf8")
}

fn roadmap_json(home: &Path, wave: &str) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .args(["roadmap", "--wave", wave, "--json"])
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_WAVE_ID");
    prepend_test_bin(&mut command, home);
    let output = command.output().expect("lf roadmap runs");
    assert!(
        output.status.success(),
        "lf roadmap failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("lf roadmap emits JSON")
}

fn prepend_test_bin(command: &mut Command, home: &Path) {
    let bin = home.join("bin");
    if !bin.is_dir() {
        return;
    }
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let paths = std::iter::once(bin).chain(std::env::split_paths(&inherited));
    command.env("PATH", std::env::join_paths(paths).expect("test PATH"));
}

fn seed_stale_project_work(home: &Path) {
    const STALE_WORK_ID: &str = "proj_3998f8611e9c9069f53c44dc831803d7";
    const STALE_PROJECT_ID: &str = "0b13d98e-800e-49a3-a778-6fb13ac0f03a";

    let wave = seed(home, "product");
    let repo = home.join("repo");
    std::fs::create_dir_all(&repo).expect("repo");
    let store = SqliteStore::new(&home.join("loopflow.db")).expect("open store");
    let now = OffsetDateTime::now_utc();
    let stale = Project {
        id: ProjectId::parse(STALE_WORK_ID).expect("recorded Project Work id"),
        plan: ProjectPlan {
            id: LinearProjectId::new(STALE_PROJECT_ID).expect("recorded PM Project id"),
            slug: "wave-chat".to_string(),
            name: "Wave Chat".to_string(),
            prompt_context: "Wave Chat remains steerable.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp() - 1,
        },
        wave_id: wave.id().clone(),
        iteration: 1,
        observation_cursor: 0,
        last_state_fingerprint: None,
        agent: "codex".to_string(),
        provider: "codex".to_string(),
        provider_session_id: None,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    store.insert_project(&stale).expect("seed stale Project");

    let current = Project {
        id: ProjectId::new(),
        plan: ProjectPlan {
            id: LinearProjectId::new("95159066-9098-4d0b-8903-01459dc7ec14")
                .expect("current PM Project id"),
            slug: "auditability".to_string(),
            name: "Auditability".to_string(),
            prompt_context: "Every claim points to its receipt.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        wave_id: wave.id().clone(),
        iteration: 1,
        observation_cursor: 0,
        last_state_fingerprint: None,
        agent: "codex".to_string(),
        provider: "codex".to_string(),
        provider_session_id: None,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    store
        .insert_project(&current)
        .expect("seed current Project");

    let work = store
        .work_for_child(&ChildRef::Project(current.id.clone()))
        .expect("resolve current Project Work");
    let (_, lease) = store
        .reserve_run(&work, &RunTrigger::User)
        .expect("reserve current Project Run");
    store
        .advance_run(
            &lease,
            &RunAdvance::RunStarting {
                containment: Containment::Tmux {
                    name: "missing-current-project".to_string(),
                },
                cwd: repo.clone(),
            },
        )
        .expect("start current Project Run");

    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).expect("test bin");
    let tmux = bin.join("tmux");
    std::fs::write(&tmux, "#!/bin/sh\nexit 0\n").expect("fake tmux");
    std::fs::set_permissions(&tmux, std::fs::Permissions::from_mode(0o755))
        .expect("make fake tmux executable");

    let payload = serde_json::json!({
        "projects": [
            {
                "id": "95159066-9098-4d0b-8903-01459dc7ec14",
                "slug": "auditability",
                "name": "Auditability",
                "summary": "Every claim points to its receipt.",
                "definition": "Every product surface shows enough truth to trust the system.",
                "flows": {"first": null, "loop": null, "finally": null},
                "krs": [{"text": "Every visible state carries its reason", "holds": false}],
                "initiative_ids": ["initiative-product"],
                "team_ids": ["team-product"]
            }
        ],
        "items": [
            {
                "id": "task-prd-52",
                "identifier": "PRD-52",
                "url": "https://linear.app/loopflow/issue/PRD-52",
                "name": "Expose one fleet snapshot from Wave to raw trace",
                "description": "Keep focused reads useful through stale Work.",
                "rank": 1,
                "completed": false,
                "project_id": "95159066-9098-4d0b-8903-01459dc7ec14",
                "project": "auditability",
                "team_id": "team-product",
                "assignee": null
            }
        ]
    });
    store
        .put_pm_snapshot(&PmSnapshotRow {
            repo: std::fs::canonicalize(repo)
                .expect("canonical repo")
                .display()
                .to_string(),
            wave: "product".to_string(),
            provider: "linear".to_string(),
            initiative: "initiative-product".to_string(),
            synced_at: now.unix_timestamp(),
            payload: serde_json::to_string(&payload).expect("serialize PM snapshot"),
        })
        .expect("seed PM snapshot");
}

fn execs_json(home: &Path) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["execs", "--json"])
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .output()
        .expect("lf execs runs");
    assert!(
        output.status.success(),
        "lf execs failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("lf execs emits JSON")
}

fn runs_json(home: &Path) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["runs", "--json"])
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .output()
        .expect("lf runs runs");
    assert!(
        output.status.success(),
        "lf runs failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("lf runs emits JSON")
}

fn runs_json_filtered(home: &Path, filter: &[&str]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .arg("runs")
        .args(filter)
        .arg("--json")
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .output()
        .expect("lf runs runs");
    assert!(
        output.status.success(),
        "lf runs failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("lf runs emits JSON")
}

fn trace_json(home: &Path, exec_id: &str) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["trace", exec_id, "--json"])
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .output()
        .expect("lf trace runs");
    assert!(
        output.status.success(),
        "lf trace failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("lf trace emits JSON")
}

#[test]
fn status_reports_skill_runs_without_lookup_or_flow_processes() {
    let home = tempfile::tempdir().expect("tempdir");
    seed(home.path(), "audit-a");

    let status = status_json(home.path(), &["audit-a"], None);

    assert_eq!(status["wave"]["name"], "audit-a");
    assert_eq!(status["runs"]["state"], "ok");
    let runs = status["runs"]["items"].as_array().expect("runs array");
    assert_eq!(runs.len(), 1);
    let skill = runs
        .iter()
        .find(|run| run["skill"] == "wave_mutate")
        .expect("the wave's skill is in its status");
    assert_eq!(skill["flow"], "wave");
    assert_eq!(skill["provider"], "codex");
    assert_eq!(skill["supplied_context_tokens"], 10);
    assert_eq!(skill["input_tokens"], 10);
    assert_eq!(skill["output_tokens"], 5);

    let human = status_human(home.path(), "audit-a");
    assert!(human.contains("wave/wave_mutate"));
    assert!(human.contains("ctx      10"));
    assert!(human.contains("tok      15"));
    assert!(!human.contains("pm sync"));
    assert!(!human.contains("build"));

    // Nothing is waiting, and the snapshot says so — it does not omit the field.
    assert_eq!(status["attention"]["state"], "ok");
    assert_eq!(status["attention"]["items"], serde_json::json!([]));
}

#[test]
fn execs_keep_the_lookup_process_ledger() {
    let home = tempfile::tempdir().expect("tempdir");
    seed(home.path(), "audit-execs");

    let execs = execs_json(home.path());
    let execs = execs.as_array().expect("exec array");
    let lookup = execs
        .iter()
        .find(|exec| exec["label"] == "pm sync")
        .expect("lookup stays available as an exec");
    assert_eq!(lookup["status"], "ok");
    assert_eq!(lookup["trace_id"], "run-1");

    let trace = trace_json(home.path(), "proc-lookup");
    assert_eq!(trace["spans"][0]["process_id"], "proc-lookup");
}

#[test]
fn runs_are_skill_invocations_with_context_and_token_evidence() {
    let home = tempfile::tempdir().expect("tempdir");
    seed(home.path(), "audit-runs");

    let runs = runs_json(home.path());
    let runs = runs.as_array().expect("run array");
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    assert_eq!(run["id"], "invocation-wave-mutate");
    assert_eq!(run["trace_id"], "run-resident");
    assert_eq!(run["exec_id"], "proc-resident");
    assert_eq!(run["skill"], "wave_mutate");
    assert_eq!(run["supplied_context_tokens"], 10);
    assert_eq!(run["input_tokens"], 10);
    assert_eq!(run["output_tokens"], 5);
    // A run declares the roadmap Project/Task that owns it — the foreign key a
    // roadmap row drills through to reach its runs.
    assert_eq!(run["project"], "auditability");
    assert_eq!(run["task"], "W2-122");
}

/// The drill: `lf runs --task <id>` joins a roadmap Task to exactly the runs it
/// produced, by the Linear issue identifier. A non-matching identifier finds
/// nothing rather than guessing.
#[test]
fn runs_drill_to_one_task_by_issue_identifier() {
    let home = tempfile::tempdir().expect("tempdir");
    seed(home.path(), "audit-drill");

    let matched = runs_json_filtered(home.path(), &["--task", "W2-122"]);
    let matched = matched.as_array().expect("run array");
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0]["task"], "W2-122");
    assert_eq!(matched[0]["trace_id"], "run-resident");

    let missed = runs_json_filtered(home.path(), &["--task", "W2-999"]);
    assert_eq!(missed.as_array().expect("run array").len(), 0);
}

/// The wave drill mirrors the internal scoping `lf status` uses.
#[test]
fn runs_drill_to_one_wave_by_name() {
    let home = tempfile::tempdir().expect("tempdir");
    seed(home.path(), "audit-wave-drill");

    let matched = runs_json_filtered(home.path(), &["--wave", "audit-wave-drill"]);
    assert_eq!(matched.as_array().expect("run array").len(), 1);

    let missed = runs_json_filtered(home.path(), &["--wave", "no-such-wave"]);
    assert_eq!(missed.as_array().expect("run array").len(), 0);
}

/// The drill loop closes: `lf trace` accepts the invocation id `lf runs` prints,
/// resolving it to the complete trace.
#[test]
fn trace_opens_from_the_invocation_id_lf_runs_prints() {
    let home = tempfile::tempdir().expect("tempdir");
    seed(home.path(), "audit-trace-invocation");

    let trace = trace_json(home.path(), "invocation-wave-mutate");
    assert_eq!(trace["trace_id"], "run-resident");
    let spans = trace["spans"].as_array().expect("span array");
    assert!(spans
        .iter()
        .any(|span| span["process_id"] == "proc-resident"));
}

/// The reproduced break: inside a resident wave, `LF_WAVE_ID` is a wave id, and
/// bare `lf status` read it as a name.
#[test]
fn ambient_wave_id_resolves_the_wave_it_names() {
    let home = tempfile::tempdir().expect("tempdir");
    let wave = seed(home.path(), "audit-b");

    let status = status_json(home.path(), &[], Some(wave.id().as_str()));

    assert_eq!(status["wave"]["id"], wave.id().as_str());
    assert_eq!(status["wave"]["name"], "audit-b");
    assert_eq!(status["runs"]["state"], "ok");
}

/// A wave that has done nothing reports an empty reading, not a missing one:
/// "we looked and found nothing" is a claim a client can trust.
#[test]
fn a_wave_with_no_runs_reports_an_empty_reading_not_a_missing_one() {
    let home = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(home.path()).expect("home");
    let store = SqliteStore::new(&home.path().join("loopflow.db")).expect("open store");
    let wave = Wave::new(
        WaveId::new(),
        "audit-c".to_string(),
        home.path().join("repo").display().to_string(),
    );
    store.create_wave(&wave).expect("register wave");

    let status = status_json(home.path(), &["audit-c"], None);

    assert_eq!(status["wave"]["status"], "ready");
    assert_eq!(status["runs"]["state"], "ok");
    assert_eq!(status["runs"]["items"], serde_json::json!([]));
    assert_eq!(status["runs"]["truncated"], false);
}

#[test]
fn stale_project_work_preserves_status_and_roadmap_evidence() {
    let home = tempfile::tempdir().expect("tempdir");
    seed_stale_project_work(home.path());

    let status = status_json(home.path(), &["product"], None);
    let status_projects = status["projects"].as_array().expect("status projects");
    assert_eq!(status_projects.len(), 1);
    assert_eq!(status_projects[0]["project"]["slug"], "auditability");
    assert_eq!(
        status_projects[0]["tasks"][0]["task"]["identifier"],
        "PRD-52"
    );
    assert_eq!(status["runs"]["state"], "ok");
    assert_eq!(status["runs"]["items"].as_array().expect("runs").len(), 1);
    assert_eq!(status["attention"]["state"], "ok");
    let attention = status["attention"]["items"]
        .as_array()
        .expect("attention items");
    assert_eq!(attention.len(), 1);
    assert_eq!(attention[0]["subject"], "auditability");
    assert_eq!(attention[0]["owner"], "wave");
    assert_eq!(
        attention[0]["reason"],
        "process is gone but the Work still records 'running'"
    );

    let unavailable = status["unavailable_projects"]
        .as_array()
        .expect("status unavailable Projects");
    assert_eq!(unavailable.len(), 1);
    assert_eq!(
        unavailable[0]["work_id"],
        "proj_3998f8611e9c9069f53c44dc831803d7"
    );
    assert_eq!(
        unavailable[0]["project_id"],
        "0b13d98e-800e-49a3-a778-6fb13ac0f03a"
    );
    assert_eq!(unavailable[0]["project_slug"], "wave-chat");
    assert_eq!(
        unavailable[0]["reason"],
        "Project is absent from the current PM snapshot"
    );
    assert_eq!(
        unavailable[0]["recovery"],
        "lf project abandon wave-chat --reason \"Project is absent from the current PM snapshot\""
    );

    let roadmap = roadmap_json(home.path(), "product");
    let wave = &roadmap["waves"][0];
    assert_eq!(wave["projects"]["state"], "ok");
    let roadmap_projects = wave["projects"]["items"]
        .as_array()
        .expect("roadmap projects");
    assert_eq!(roadmap_projects.len(), 1);
    assert_eq!(roadmap_projects[0]["project"]["slug"], "auditability");
    assert_eq!(
        roadmap_projects[0]["tasks"][0]["task"]["identifier"],
        "PRD-52"
    );
    assert_eq!(wave["unavailable_projects"], status["unavailable_projects"]);
}
