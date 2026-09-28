use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use chrono::{Local, Timelike};
use loopflow::durable::{CronReceiptId, HomeId};
use loopflow::id::WaveId;
use loopflow::ops::{CronOutcome, CronReceipt, CronSource, CronTargetKind};
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::RunEventRow;
use loopflow::work::wave::Wave;
use time::OffsetDateTime;

use loopflow_test_support::TestRepo;

fn run_lf(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(args)
        .current_dir(home)
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_DB_PATH", home.join("loopflow.db"))
        .env("NO_COLOR", "1")
        // Doctor prefers the compiled source root; scope even git -C there to
        // this fixture so freshness checks cannot fetch into a shared checkout.
        .env("GIT_DIR", home.join(".git"))
        .env("GIT_WORK_TREE", home)
        .env("GIT_ALLOW_PROTOCOL", "file")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_PROCESS_ID")
        .env_remove("LF_WAVE_ID")
        .env_remove("LF_RUN_ID")
        .env_remove("LF_RUN_CONTEXT")
        .env_remove("LF_RUN_LEASE")
        .env_remove("LF_AGENT_INVOCATION_ID")
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

fn insert_run_event(store: &SqliteStore, id: &str, ts: i64) {
    store
        .insert_run_event(
            &RunEventRow {
                run_id: id.to_string(),
                process_id: id.to_string(),
                parent_process_id: None,
                seq: 0,
                ts,
                repo: Some("/src/loopflow".to_string()),
                worktree: Some("/src/loopflow".to_string()),
                wave: Some("infrastructure".to_string()),
                node: "run".to_string(),
                event: "completed".to_string(),
                command: Some(r#"["lf","flow","telemetry-daily"]"#.to_string()),
                flow: Some("telemetry-daily".to_string()),
                skill: None,
                step_index: None,
                error: None,
            },
            None,
            None,
            None,
        )
        .unwrap();
}

fn install_current_telemetry_obligation(home: &Path) {
    let now = Local::now();
    let scheduled = now - chrono::Duration::minutes(5);
    let schedule = format!("0 {} {} * * *", scheduled.minute(), scheduled.hour());
    let started_at = scheduled.with_second(0).unwrap().timestamp() + 10;
    let home_id = HomeId::parse("home_11111111111111111111111111111111").unwrap();
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
<key>LoopflowHomeId</key><string>{home_id}</string>
<key>LoopflowActivatedAt</key><string>1787419431</string>
<key>LoopflowRepo</key><string>{repo}</string>
<key>LoopflowLfPath</key><string>/usr/local/bin/lf</string>
<key>LoopflowLfHome</key><string>{lf_home}</string>
<key>LoopflowDbPath</key><string>{database}</string>
<key>LoopflowPath</key><string>/usr/bin:/bin</string>
</dict></plist>
"#,
            repo = home.display(),
            lf_home = home.display(),
            database = home.join("loopflow.db").display(),
        ),
    )
    .unwrap();
    let receipt = CronReceipt {
        schema_version: 1,
        id: CronReceiptId::new(),
        runner_pid: 123,
        home_id,
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
    let output = run_lf(home.path(), &["doctor", "--json"]);
    assert!(
        output.status.success(),
        "lf doctor failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let fetched = fs::read_to_string(home.path().join(".git/FETCH_HEAD")).unwrap();
    assert!(fetched.contains(&home.head_sha()));
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
    insert_run_event(
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
        insert_run_event(&store, &format!("after-gap-{ordinal}"), timestamp);
        timestamp += 86_400;
        ordinal += 1;
    }
    let original_events = store.list_run_events_since(0).unwrap();
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

    let doctor = run_lf(home.path(), &["doctor", "--json"]);
    assert!(
        doctor.status.success(),
        "lf doctor failed: {}{}",
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
    assert!(report.contains("Elapsed / Run"), "{report}");
    assert!(report.contains("Land request → merge"), "{report}");
    let events_after_telemetry = store.list_run_events_since(0).unwrap();
    for original in original_events {
        assert!(events_after_telemetry.contains(&original));
    }
}
