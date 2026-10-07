use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use chrono::{Local, Timelike};
use loopflow::durable::{CronReceiptId, MachineId};
use loopflow::exec::Exec;
use loopflow::id::WaveId;
use loopflow::ops::{CronOutcome, CronReceipt, CronSource, CronTargetKind};
use loopflow::store::sqlite::SqliteStore;
use loopflow::work::wave::Wave;
use time::OffsetDateTime;

use loopflow_test_support::TestRepo;

fn run_lf(home: &Path, args: &[&str]) -> Output {
    let binary_dir = Path::new(env!("CARGO_BIN_EXE_lf")).parent().unwrap();
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(args)
        .current_dir(home)
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("NO_COLOR", "1")
        .env(
            "PATH",
            format!(
                "{}:{}",
                binary_dir.display(),
                std::env::var("PATH").unwrap()
            ),
        )
        // Doctor prefers the compiled source root; scope even git -C there to
        // this fixture so freshness checks cannot fetch into a shared checkout.
        .env("GIT_DIR", home.join(".git"))
        .env("GIT_WORK_TREE", home)
        .env("GIT_ALLOW_PROTOCOL", "file")
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_PROCESS_ID")
        .env_remove("LF_WAVE_ID")
        .env_remove("LF_CAPTURE_KEY")
        .output()
        .unwrap()
}

fn continuity_check(output: &Output) -> serde_json::Value {
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "continuity")
        .unwrap()
        .clone()
}

fn insert_exec(store: &SqliteStore, _id: &str, ts: i64) {
    store
        .record_exec(&Exec {
            id: loopflow::id::ExecId::new(),
            trace_id: loopflow::id::TraceId::new(),
            parent_exec_id: None,
            via_agent: Some(false),
            caller_session_id: None,
            caller_provider_generation: None,
            command: Some(r#"["lf","flow","telemetry-daily"]"#.into()),
            repo: Some("/src/loopflow".into()),
            cwd: None,
            started_at: ts,
            completed_at: Some(ts),
            outcome: Some("succeeded".into()),
            exit_code: Some(0),
            signal: None,
            error: None,
        })
        .unwrap();
}

fn install_current_telemetry_obligation(home: &Path) {
    let now = Local::now();
    let scheduled = now - chrono::Duration::minutes(5);
    let schedule = format!("0 {} {} * * *", scheduled.minute(), scheduled.hour());
    let started_at = scheduled.with_second(0).unwrap().timestamp() + 10;
    let machine_id = MachineId::parse("home_11111111111111111111111111111111").unwrap();
    let launch_agents = home.join("Library/LaunchAgents");
    fs::create_dir_all(&launch_agents).unwrap();
    fs::write(
        launch_agents.join("loopflow.cron.infrastructure.telemetry-daily.plist"),
        format!(
            r#"<plist><dict>
<key>LoopflowWave</key><string>infrastructure</string>
<key>LoopflowFlow</key><string>telemetry-daily</string>
<key>LoopflowTargetKind</key><string>flow</string>
<key>LoopflowSchedule</key><string>{schedule}</string>
<key>LoopflowHomeId</key><string>{machine_id}</string>
<key>LoopflowActivatedAt</key><string>1787419431</string>
<key>LoopflowRepo</key><string>{repo}</string>
<key>LoopflowLfPath</key><string>/usr/local/bin/lf</string>
<key>LoopflowLfHome</key><string>{lf_home}</string>
<key>LoopflowPath</key><string>/usr/bin:/bin</string>
</dict></plist>
"#,
            repo = home.display(),
            lf_home = home.display(),
        ),
    )
    .unwrap();
    let receipt = CronReceipt {
        schema_version: 1,
        id: CronReceiptId::new(),
        runner_pid: 123,
        runner_started_at: None,
        machine_id,
        wave: "infrastructure".to_string(),
        flow: "telemetry-daily".to_string(),
        target_kind: CronTargetKind::Flow,
        source: CronSource::Scheduled,
        schedule,
        repo: home.to_path_buf(),
        lf_path: "/usr/local/bin/lf".into(),
        log_path: home.join("cron.log"),
        started_at,
        finished_at: Some(started_at + 60),
        outcome: CronOutcome::Succeeded,
        exit_code: Some(0),
        error: None,
    };
    let receipt_dir = home.join("cron/receipts/infrastructure/telemetry-daily");
    fs::create_dir_all(&receipt_dir).unwrap();
    fs::write(
        receipt_dir.join(format!("{}-{}.json", receipt.started_at, receipt.id)),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

#[test]
fn doctor_json_reports_the_build_revision_and_freshness_check() {
    let home = TestRepo::new();
    SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let fetch_head = home.path().join(".git/FETCH_HEAD");
    let before = fs::read(&fetch_head).ok();
    let output = run_lf(home.path(), &["machine", "doctor", "--json"]);
    assert!(
        output.status.success(),
        "lf machine doctor failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(fs::read(fetch_head).ok(), before);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["store"]["build_source_revision"],
        loopflow::build_info::source_revision()
    );
    assert!(report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check["name"] == "binary-freshness"));
}

#[test]
fn copied_production_history_does_not_block_the_telemetry_scorecard() {
    let home = TestRepo::new();
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    insert_exec(
        &store,
        "august-03",
        OffsetDateTime::parse(
            "2026-08-03T12:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap()
        .unix_timestamp(),
    );
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let mut timestamp = OffsetDateTime::parse(
        "2026-08-12T12:00:00Z",
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap()
    .unix_timestamp();
    let mut ordinal = 12;
    while timestamp <= now {
        insert_exec(&store, &format!("after-gap-{ordinal}"), timestamp);
        timestamp += 86_400;
        ordinal += 1;
    }
    let original_events = store.execs_since(0).unwrap();
    install_current_telemetry_obligation(home.path());
    fs::create_dir_all(home.path().join(".lf/flows")).unwrap();
    fs::create_dir_all(home.path().join("scripts")).unwrap();
    fs::write(
        home.path().join(".lf/flows/telemetry-daily.yaml"),
        "- cmd: doctor\n- cmd: __telemetry-scorecard\n",
    )
    .unwrap();
    fs::create_dir_all(home.path().join("performance")).unwrap();
    fs::create_dir_all(home.path().join("wave/product/metrics")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for file in ["scripts/lifecycle_scorecard.py", "performance/budgets.json"] {
        fs::copy(source.join(file), home.path().join(file)).unwrap();
    }
    fs::write(
        home.path().join("wave/product/metrics/task-loop-trust.md"),
        "---\nschema: 1\nid: task-loop-trust\nstage: installed\ninstrument: lifecycle-scorecard\nunit: ratio\nwindow: 7d\nfreshness: 30h\n---\n\n# Task loops earn trust\n\nCount settled Task loops.\n",
    )
    .unwrap();
    store
        .create_wave(&Wave::new(
            WaveId::new(),
            "product".to_string(),
            home.path().display().to_string(),
        ))
        .unwrap();

    // Recovery success cannot stand in for the missing natural firing, even
    // when the current SessionHistory scorecard can run after continuity passes.
    let receipt_path = fs::read_dir(
        home.path()
            .join("cron/receipts/infrastructure/telemetry-daily"),
    )
    .unwrap()
    .next()
    .unwrap()
    .unwrap()
    .path();
    let original_receipt = fs::read(&receipt_path).unwrap();
    let mut recovery: CronReceipt = serde_json::from_slice(&original_receipt).unwrap();
    recovery.source = CronSource::Recovery;
    fs::write(&receipt_path, serde_json::to_vec(&recovery).unwrap()).unwrap();
    let missing = run_lf(home.path(), &["doctor", "--json"]);
    assert_eq!(continuity_check(&missing)["status"], "fail");
    let blocked = run_lf(home.path(), &["--batch", "flow", "telemetry-daily"]);
    assert!(!blocked.status.success());
    assert!(!String::from_utf8_lossy(&blocked.stdout).contains("Lifecycle scorecard"));
    assert_eq!(
        serde_json::from_slice::<CronReceipt>(&fs::read(&receipt_path).unwrap())
            .unwrap()
            .source,
        CronSource::Recovery
    );
    fs::write(&receipt_path, original_receipt).unwrap();

    let doctor = run_lf(home.path(), &["doctor", "--json"]);
    assert!(
        doctor.status.success(),
        "lf machine doctor failed: {}{}",
        String::from_utf8_lossy(&doctor.stdout),
        String::from_utf8_lossy(&doctor.stderr)
    );
    let continuity = continuity_check(&doctor);
    assert_eq!(continuity["status"], "ok");
    let detail = continuity["detail"].as_str().unwrap();
    assert!(detail.contains("8 historical ledger gap-day(s) predate first cron activation"));
    for day in 4..=11 {
        assert!(detail.contains(&format!("2026-08-{day:02}")), "{detail}");
    }

    let telemetry = run_lf(home.path(), &["--batch", "flow", "telemetry-daily"]);
    assert!(
        telemetry.status.success(),
        "telemetry-daily failed: {}{}",
        String::from_utf8_lossy(&telemetry.stdout),
        String::from_utf8_lossy(&telemetry.stderr)
    );
    let report = String::from_utf8_lossy(&telemetry.stdout);
    assert!(report.contains("Lifecycle scorecard"), "{report}");
    assert!(report.contains("Recorded input elapsed"), "{report}");
    assert!(report.contains("Land request → merge"), "{report}");
    let events_after_telemetry = store.execs_since(0).unwrap();
    for original in original_events {
        assert!(events_after_telemetry.contains(&original));
    }
}

#[test]
fn doctor_accepts_machine_commands_without_a_repository() {
    let home = TestRepo::new();
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    insert_exec(
        &store,
        "machine-command",
        OffsetDateTime::now_utc().unix_timestamp(),
    );
    let mut event = store.execs_since(0).unwrap().pop().unwrap();
    event.id = loopflow::id::ExecId::new();
    event.repo = None;
    event.command = Some("lf help".to_string());
    store.record_exec(&event).unwrap();

    let output = run_lf(home.path(), &["machine", "doctor", "--json"]);
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let identity = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "identity")
        .unwrap();
    assert_eq!(identity["status"], "ok");
    assert!(identity["detail"]
        .as_str()
        .unwrap()
        .contains("2 Exec(s) without repository scope"));
}

#[test]
fn doctor_does_not_initialize_a_missing_database() {
    let home = TestRepo::new();
    let output = run_lf(home.path(), &["machine", "doctor", "--json"]);
    assert!(!output.status.success());
    assert!(!home.path().join("loopflow.db").exists());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["store"]["migration_error"]
        .as_str()
        .unwrap()
        .contains("does not exist"));
    assert!(report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check["name"] == "install-selection"));
}

#[test]
fn doctor_reports_execs_and_scheduler_despite_an_unknown_migration() {
    let home = TestRepo::new();
    let path = home.path().join("loopflow.db");
    let store = SqliteStore::new(&path).unwrap();
    insert_exec(&store, "recent", OffsetDateTime::now_utc().unix_timestamp());
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute("INSERT INTO schema_migrations (version, applied_at) VALUES ('9.0.001_future', unixepoch() + 1)", []).unwrap();
    let output = run_lf(home.path(), &["machine", "doctor", "--json"]);
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["store"]["migration_error"]
        .as_str()
        .unwrap()
        .contains("9.0.001_future"));
    assert!(report["rows"].as_u64().unwrap() >= 1);
    assert_eq!(continuity_check(&output)["status"], "ok");
    assert!(report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check["name"] == "attribution" && check["status"] == "ok"));
    let latest: String = connection
        .query_row(
            "SELECT version FROM schema_migrations ORDER BY applied_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(latest, "9.0.001_future");
}

#[test]
fn doctor_keeps_reporting_when_the_database_is_corrupt() {
    let home = TestRepo::new();
    let path = home.path().join("loopflow.db");
    fs::write(&path, b"not a sqlite database").unwrap();
    let output = run_lf(home.path(), &["machine", "doctor", "--json"]);
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["store"]["migration_error"].is_string());
    assert!(report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check["name"] == "execs" && check["status"] == "fail"));
    assert_eq!(continuity_check(&output)["status"], "ok");
    assert_eq!(fs::read(path).unwrap(), b"not a sqlite database");
}
