mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{Local, Timelike, Utc};
use loopflow::durable::MachineId;
use loopflow::ops::cron::accounting::ScheduledReleaseOutcome;
use loopflow::ops::cron::history::release_history;
use loopflow::ops::{
    add_cron, run_cron, schedule_from_cron, CronHost, CronSource, CronSpec, CronTargetKind,
    SystemLaunchctl,
};
use loopflow_test_support::TestRepo;
use sha2::Digest;
use support::EnvGuard;

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

#[test]
fn scheduled_release_flow_settles_product_results_and_preserves_failures() {
    run_scenarios(&[
        "published",
        "no-change",
        "telemetry-failure",
        "telemetry-recovered",
        "telemetry-missing",
        "smoke-failure",
        "missing-stage",
    ]);
}

#[test]
fn public_reconciliation_survives_controller_death_without_republishing() {
    run_scenarios(&["reconcile-interrupted"]);
}

#[test]
fn public_verification_recovers_interrupted_checkout_materialization() {
    run_scenarios(&["reconcile-empty", "reconcile-branch", "reconcile-missing"]);
}

#[test]
fn historical_telemetry_segments_survive_schedule_replacement_in_release_history() {
    run_scenarios(&["telemetry-history"]);
}

#[test]
fn telemetry_history_does_not_imply_release_coverage() {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/release_history.json"
    ))
    .unwrap();
    let mut segment = fixture["obligations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["flow"] == "telemetry-daily")
        .unwrap()
        .clone();
    segment["repo"] = serde_json::json!(repo.path().canonicalize().unwrap());
    let now = Utc::now().timestamp();
    for field in ["installation_activated_at", "activated_at", "observed_at"] {
        segment[field] = now.into();
    }
    let directory = home.path().join("cron/obligations");
    fs::create_dir_all(&directory).unwrap();
    for flow in ["telemetry-daily", "release-run"] {
        segment["id"] = flow.into();
        segment["flow"] = flow.into();
        fs::write(
            directory.join(format!("{flow}.json")),
            serde_json::to_vec(&segment).unwrap(),
        )
        .unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        command
            .args(["release", "history", "--wave", "infrastructure"])
            .current_dir(repo.path());
        let output = command.output().unwrap();
        assert!(output.status.success(), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("infrastructure/telemetry-daily"));
        assert_eq!(
            text.contains("Opportunity coverage unknown"),
            flow == "telemetry-daily",
            "{text}"
        );
        let output = command.arg("--json").output().unwrap();
        assert!(output.status.success(), "{output:?}");
        let history: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            history["observation_frontier"].is_null(),
            flow == "telemetry-daily"
        );
        assert_eq!(history["summary"]["published"], 0);
        assert_eq!(history["summary"]["no_change"], 0);
    }
}

#[test]
fn interrupted_telemetry_recovers_only_after_its_runner_and_child_exit() {
    run_scenarios(&["telemetry-interrupted"]);
}

#[test]
fn closed_release_history_records_repair_without_settling_or_transferring_work() {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    let task = support::register_task(
        home.path(),
        repo.path(),
        &git(repo.path(), &["branch", "--show-current"]),
        &git(repo.path(), &["rev-parse", "HEAD"]),
    );
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/release_history.json"
    ))
    .unwrap();
    let mut segment = fixture["obligations"][0].clone();
    segment["repo"] = serde_json::json!(repo.path().canonicalize().unwrap());
    segment["wave"] = "task-pr-tests".into();
    segment["closed_at"] = 122500.into();
    let owner = &mut segment["opportunities"][0];
    owner["attempts"][0]["outcome"] = serde_json::json!({"status": "running"});
    owner["attempts"][0]["finished_at"] = serde_json::Value::Null;
    let directory = home.path().join("cron/obligations");
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("fixture.json");
    let original = serde_json::to_vec(&segment).unwrap();
    fs::write(&path, &original).unwrap();
    let history = || {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .args([
                "release",
                "history",
                "--wave",
                "task-pr-tests",
                "--days",
                "1",
                "--json",
            ])
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    let before = history();
    assert_eq!(before["summary"]["due"], 0);
    assert_eq!(
        before["summary"]["closed_unsettled"],
        serde_json::json!(["opportunity_0"])
    );
    assert_eq!(
        before["summary"]["undispositioned_failures"],
        serde_json::json!(["opportunity_0"])
    );
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args([
            "cron",
            "disposition",
            "opportunity_0",
            "--wave",
            "task-pr-tests",
            "--owner",
            task.task.id.as_str(),
            "--reason",
            "reconcile v1.2.3 on the original Machine",
        ])
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let after = history();
    assert_eq!(
        after["summary"]["closed_unsettled"],
        before["summary"]["closed_unsettled"]
    );
    assert_eq!(
        after["summary"]["late_dispositions"],
        serde_json::json!(["opportunity_0"])
    );
    assert_eq!(
        after["summary"]["undispositioned_failures"],
        serde_json::json!([])
    );
    assert_eq!(after["dispositions"][0]["owner"], task.task.id.as_str());
    assert_eq!(after["summary"]["published"], 0);
    assert_eq!(after["summary"]["qualifying_pairs"], serde_json::json!([]));
    assert_eq!(fs::read(&path).unwrap(), original);
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args([
            "release",
            "history",
            "--wave",
            "task-pr-tests",
            "--days",
            "1",
        ])
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("blocked opportunity_0"), "{text}");
    assert!(text.contains("v1.2.3 at abc"), "{text}");
    assert!(text.contains("lf cron disposition opportunity_0"), "{text}");
    assert!(text.contains(task.task.id.as_str()), "{text}");

    // A saved wait predates closure and cannot promise another firing afterward.
    let pending = &mut segment["opportunities"][1];
    pending["attempts"] = serde_json::json!([]);
    pending["wait"] = serde_json::json!({
        "recorded_at": 122450,
        "reason": "became due after this wake froze its coverage",
        "retry_at": 208800
    });
    let retained = serde_json::to_vec(&segment).unwrap();
    fs::write(&path, &retained).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args([
            "release",
            "history",
            "--wave",
            "task-pr-tests",
            "--days",
            "30000",
        ])
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("blocked opportunity_1"), "{text}");
    assert!(!text.contains("next firing 208800"), "{text}");
    assert!(text.contains("previously expected firing 208800"), "{text}");
    assert_eq!(fs::read(&path).unwrap(), retained);
    assert_eq!(
        history()["obligations"][0]["opportunities"][1]["wait"],
        segment["opportunities"][1]["wait"]
    );
}

#[test]
fn legacy_telemetry_without_runner_identity_stays_unresolved() {
    run_scenarios(&["telemetry-unknown"]);
}

#[test]
fn release_overlap_records_exact_next_firing_without_mutation() {
    run_scenarios(&["release-overlap"]);
}

#[test]
fn saved_candidate_recovers_only_with_affirmative_unpublished_source_evidence() {
    run_scenarios(&[
        "candidate-successor",
        "candidate-unknown",
        "candidate-partial",
        "candidate-valid",
    ]);
}

#[test]
fn replacement_schedule_resumes_candidate_with_predecessor_and_successor_dues() {
    run_scenarios(&["candidate-valid-closed"]);
}

fn run_scenarios(scenarios: &[&str]) {
    for &scenario in scenarios {
        let recovering = scenario.starts_with("candidate-");
        let valid_candidate = scenario.starts_with("candidate-valid");
        let expected_tag = "v0.9.1";
        let baseline_tag = if recovering && !valid_candidate {
            "v0.9.0"
        } else {
            "v0.9.1"
        };
        let home = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let published = state.path().join("published");
        if scenario == "no-change" {
            fs::write(&published, "already published").unwrap();
        }
        let gh = format!(
            r#"#!/bin/sh
[ "$1" != --version ] || exit 0
case "$1 $2" in
  'run list')
    commit=$(git rev-parse {baseline_tag})
    if [ '{recovering}' = true ]; then
      successor=$(git rev-parse origin/main)
      printf '[{{"databaseId":43,"headBranch":"release-candidate/default/v0-9-1/%s","headSha":"%s","status":"completed","conclusion":"success","url":"https://example.test/run/43"}},' "$successor" "$successor"
    else printf '['; fi
    printf '{{"databaseId":42,"headBranch":"v0.9.1","headSha":"%s","status":"completed","conclusion":"success","url":"https://example.test/run/42"}}]\n' "$commit" ;;
  'release view')
    [ -f '{}' ] || {{ echo 'release not found' >&2; exit 1; }}
    printf '{{"isDraft":false}}\n' ;;
  'run download') exit 0 ;;
  'pr list')
    case "$*" in
      *--head*)
        if [ '{recovering}' = true ]; then
          printf '[{{"number":5,"state":"MERGED","mergeCommit":{{"oid":"%s"}}}}]\n' "$(git rev-parse origin/main)"
        else printf '[]\n'; fi ;;
      *) printf '[]\n' ;;
    esac ;;
  'repo view') printf 'test/repo\n' ;;
  *) echo "unexpected gh invocation: $*" >&2; exit 91 ;;
esac
"#,
            published.display()
        );
        let _env = EnvGuard::with_home(
            &[("gh", &gh), ("launchctl", "#!/bin/sh\nexit 0\n")],
            Some(home.path()),
        );
        let repo = TestRepo::new();
        let repo_path = repo.path().canonicalize().unwrap();
        let publisher = state.path().join("publisher.sh");
        let public_proof = state.path().join("public-proof.json");
        fs::write(
            &publisher,
            format!(
                r#"#!/bin/sh
set -eu
case "$1" in
  check) exit 0 ;;
  inspect)
    if [ '{recovering}' = true ] && [ "$3" = "$(git rev-parse {baseline_tag})" ] && [ '{valid_candidate}' != true ]; then
      [ '{scenario}' != candidate-unknown ] || {{ echo 'cannot establish publication state' >&2; exit 74; }}
      publications='[]'
      [ '{scenario}' != candidate-partial ] || publications='["versioned DMG"]'
      printf '{{"preparation_required":["drafts/incoming.sql"],"publications":%s}}\n' "$publications"
    else
      printf '{{"preparation_required":[],"publications":null}}\n'
    fi ;;
  prepare)
    while [ "$#" -gt 0 ]; do
      if [ "$1" = --output ]; then
        mkdir -p "$2"
        printf '{{}}\n' > "$2/candidate.json"
        exit 0
      fi
      shift
    done
    exit 92 ;;
  publish) printf 'published\n' >> '{}' ;;
  reconcile)
    if [ '{scenario}' = reconcile-interrupted ] && [ ! -f '{state}/allow' ]; then
      pwd > '{state}/checkout'
      kill -9 "$PPID"
      : > '{state}/ready'
      while [ ! -f '{state}/allow' ]; do
        [ -d '{state}' ] || exit 1
        sleep 0.02
      done
    fi
    [ '{scenario}' != smoke-failure ] || {{ echo 'exact-tag smoke failed' >&2; exit 73; }}
    mkdir -p "$LF_RELEASE_MAIN_REPO/.lf/logs"
    cp '{}' "$LF_RELEASE_MAIN_REPO/.lf/logs/release.{expected_tag}.verified.json" ;;
  *) echo "unexpected publisher stage: $1" >&2; exit 93 ;;
esac
"#,
                published.display(),
                public_proof.display(),
                state = state.path().display(),
            ),
        )
        .unwrap();
        repo.create_file(
            ".lf/config.yaml",
            &format!(
                "release:\n  targets:\n    default:\n      workflow: release.yml\n      completion: github-release\n      verify: ['test -f README.md']\n      publisher: ['sh', '{}']\n",
                publisher.display()
            ),
        );
        repo.create_file(
            ".lf/flows/release-run.yaml",
            "- cmd: repo release run patch\n",
        );
        repo.create_file("wave/infrastructure/GOAL.md", "# Infrastructure\n");
        repo.stage_all();
        repo.commit("Release fixture");
        repo.push();
        for args in [["tag", baseline_tag], ["push", "--tags"]] {
            git(repo.path(), &args);
        }
        let rejected_commit = repo.head_sha();
        if recovering {
            repo.create_file("prepared.txt", "Successor with migrations materialized\n");
            repo.stage_all();
            repo.commit("Prepared successor");
            repo.push();
        }
        let successor_commit = repo.head_sha();
        let stages: Vec<_> = [
            "public_artifacts_verified",
            "versioned_dmg_verified",
            "latest_dmg_verified",
            "website_release_verified",
            "crate_version_verified",
            "exact_tag_smoke_passed",
        ]
        .into_iter()
        .filter(|stage| scenario != "missing-stage" || *stage != "latest_dmg_verified")
        .collect();
        fs::write(
            &public_proof,
            serde_json::to_vec(&serde_json::json!({
                "tag": expected_tag,
                "source_commit": if valid_candidate { &rejected_commit } else { &successor_commit },
                "workflow_run_id": if recovering && !valid_candidate { "43" } else { "42" },
                "artifact_sha256": {"simulated-artifact": "simulated-hash"},
                "completed_stages": stages,
            }))
            .unwrap(),
        )
        .unwrap();

        repo.create_file("local.txt", "unpublished caller commit\n");
        repo.stage_all();
        repo.commit("Caller local work");
        repo.create_file("README.md", "staged caller bytes\n");
        git(repo.path(), &["add", "README.md"]);
        repo.create_file("README.md", "unstaged caller bytes\n");
        repo.create_file("notes.txt", "untracked caller bytes\n");
        let caller_state = || {
            (
                repo.head_sha(),
                git(repo.path(), &["symbolic-ref", "HEAD"]),
                fs::read(repo.path().join(".git/index")).unwrap(),
                fs::read(repo.path().join("README.md")).unwrap(),
                fs::read(repo.path().join("notes.txt")).unwrap(),
                fs::read(repo.path().join("local.txt")).unwrap(),
            )
        };
        let before = caller_state();

        let agents = home.path().join("Library/LaunchAgents");
        let lf_home = home.path().join(".lf");
        let mut host = CronHost {
            machine_id: MachineId::new(),
            lf_home: lf_home.clone(),
            path_env: std::env::var("PATH").unwrap(),
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        host.machine_id = runtime.block_on(async {
            let store = loopflow::store::open_ephemeral_store(
                &loopflow::store::StorageConfig::sqlite(host.lf_home.join("loopflow.db")),
            )
            .await
            .unwrap();
            let wave = loopflow::work::wave::Wave::new(
                loopflow::id::WaveId::new(),
                "infrastructure".into(),
                repo_path.display().to_string(),
            );
            store.create_wave(&wave).await.unwrap();
            store.local_machine().await.unwrap().id
        });
        let scheduled = Local::now() - chrono::Duration::minutes(1);
        let schedule = schedule_from_cron(&format!(
            "0 {} {} * * *",
            scheduled.minute(),
            scheduled.hour()
        ))
        .unwrap();
        let telemetry_runner = state.path().join("telemetry.sh");
        let telemetry_calls = state.path().join("telemetry-calls");
        fs::write(
            &telemetry_runner,
            format!(
                r#"#!/bin/sh
printf 'attempt\n' >> '{}'
if [ '{scenario}' = telemetry-interrupted ] && [ ! -f '{state}/allow' ]; then
  touch '{state}/ready'
  while [ ! -f '{state}/allow' ]; do sleep 0.05; done
  printf 'original check finished\n' > '{state}/completed'
fi
[ '{scenario}' != telemetry-failure ] || exit 71
if [ '{scenario}' = telemetry-recovered ] && [ "$(wc -l < '{}')" -eq 1 ]; then exit 72; fi
exit 0
"#,
                telemetry_calls.display(),
                telemetry_calls.display(),
                state = state.path().display()
            ),
        )
        .unwrap();
        fs::set_permissions(&telemetry_runner, fs::Permissions::from_mode(0o755)).unwrap();
        let telemetry = CronSpec {
            wave: "infrastructure".into(),
            flow: "telemetry-daily".into(),
            target_kind: CronTargetKind::Flow,
            schedule: schedule.clone(),
            working_directory: repo_path.clone(),
            // Only the external verification is simulated. The release uses the real CLI.
            lf_path: telemetry_runner,
            host: host.clone(),
        };
        add_cron(&agents, &telemetry, &SystemLaunchctl).unwrap();
        let mut interrupted = if scenario == "telemetry-interrupted" {
            let log = fs::File::create(state.path().join("controller.log")).unwrap();
            let child = Command::new(env!("CARGO_BIN_EXE_lf"))
                .args([
                    "cron",
                    "run",
                    "--wave",
                    "infrastructure",
                    "--flow",
                    "telemetry-daily",
                    "--scheduled",
                ])
                .current_dir(&repo_path)
                .env("LF_HOME", &lf_home)
                .stdout(Stdio::from(log.try_clone().unwrap()))
                .stderr(Stdio::from(log))
                .spawn()
                .unwrap();
            let parent = TelemetryParent {
                child,
                state: state.path().to_path_buf(),
            };
            wait_for(&state.path().join("ready"));
            Some(parent)
        } else {
            None
        };
        if !matches!(scenario, "telemetry-missing" | "telemetry-interrupted") {
            let telemetry_result = run_cron(
                &agents,
                &telemetry.wave,
                &telemetry.flow,
                &host.machine_id,
                &host.machine_id,
                CronSource::Scheduled,
            );
            assert_eq!(
                telemetry_result.is_err(),
                matches!(scenario, "telemetry-failure" | "telemetry-recovered")
            );
        }
        let mut original_receipts = loopflow::ops::list_cron_receipts(
            &loopflow::ops::receipt_root(&lf_home),
            "infrastructure",
            Some("telemetry-daily"),
            5,
        )
        .unwrap();

        if scenario == "telemetry-unknown" {
            let receipt = &mut original_receipts[0];
            receipt.runner_pid = u32::MAX;
            receipt.runner_started_at = None;
            receipt.outcome = loopflow::ops::CronOutcome::Running;
            receipt.finished_at = None;
            receipt.exit_code = None;
            let path = loopflow::ops::receipt_root(&lf_home)
                .join("infrastructure/telemetry-daily")
                .join(format!("{}-{}.json", receipt.started_at, receipt.id));
            let mut legacy = serde_json::to_value(receipt).unwrap();
            legacy.as_object_mut().unwrap().remove("runner_started_at");
            fs::write(path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();
        }

        if scenario == "telemetry-recovered" {
            // A same-second pass with a lexically later UUID cannot establish
            // that the failed check was subsequently repaired.
            let mut tied = original_receipts[0].clone();
            tied.id =
                loopflow::durable::CronReceiptId::parse("cron_ffffffffffffffffffffffffffffffff")
                    .unwrap();
            tied.outcome = loopflow::ops::CronOutcome::Succeeded;
            tied.exit_code = Some(0);
            tied.error = None;
            let path = loopflow::ops::receipt_root(&lf_home)
                .join("infrastructure/telemetry-daily")
                .join(format!("{}-{}.json", tied.started_at, tied.id));
            fs::write(path, serde_json::to_vec_pretty(&tied).unwrap()).unwrap();
            original_receipts.push(tied);
        }

        // Model an already-installed daily obligation in a disposable Machine. No
        // production schedule or clock is changed; these are synthetic due dates.
        let mut release = CronSpec {
            flow: "release-template".into(),
            lf_path: PathBuf::from(env!("CARGO_BIN_EXE_lf")),
            ..telemetry
        };
        let installed = add_cron(&agents, &release, &SystemLaunchctl).unwrap();
        let plist = fs::read_to_string(&installed.path)
            .unwrap()
            .replace("release-template", "release-run")
            .replace(
                &format!("<string>{}</string>", installed.activated_at),
                &format!("<string>{}</string>", Utc::now().timestamp() - 3 * 86400),
            );
        let path = PathBuf::from(
            installed
                .path
                .to_string_lossy()
                .replace("release-template", "release-run"),
        );
        fs::rename(installed.path, &path).unwrap();
        fs::write(path, plist).unwrap();
        release.flow = "release-run".into();
        if scenario == "telemetry-history" {
            // Seed a previously observed, unchanged schedule and one retained
            // failed prerequisite older than the history presentation window.
            let activation = Utc::now().timestamp() - 3 * 86400;
            let telemetry_path = loopflow::ops::list_crons(&agents, &SystemLaunchctl)
                .unwrap()
                .into_iter()
                .find(|job| job.flow == "telemetry-daily")
                .unwrap();
            let plist = fs::read_to_string(&telemetry_path.path).unwrap().replace(
                &format!("<string>{}</string>", telemetry_path.activated_at),
                &format!("<string>{activation}</string>"),
            );
            fs::write(telemetry_path.path, plist).unwrap();
            add_cron(&agents, &release, &SystemLaunchctl).unwrap();
            let paths: Vec<_> = fs::read_dir(lf_home.join("cron/obligations"))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect();
            for path in paths {
                if path.extension().is_none_or(|e| e != "json") {
                    continue;
                }
                let mut record: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                record["observed_at"] = activation.into();
                if record["flow"] == "telemetry-daily" {
                    let boundary = activation + 2 * 86400;
                    let mut prior = record.clone();
                    let prior_id = format!("{}-prior", record["id"].as_str().unwrap());
                    let previous_time = scheduled - chrono::Duration::minutes(1);
                    prior["id"] = prior_id.clone().into();
                    prior["schedule"] = format!(
                        "0 {} {} * * *",
                        previous_time.minute(),
                        previous_time.hour()
                    )
                    .into();
                    prior["activated_at"] = activation.into();
                    prior["installation_activated_at"] = activation.into();
                    prior["closed_at"] = boundary.into();
                    fs::write(
                        path.with_file_name(format!("{prior_id}.json")),
                        serde_json::to_vec_pretty(&prior).unwrap(),
                    )
                    .unwrap();
                    record["replaces"] = prior_id.into();
                    record["activated_at"] = boundary.into();
                    record["installation_activated_at"] = activation.into();
                }
                fs::write(path, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
            }
            let mut old = original_receipts[0].clone();
            old.id = loopflow::durable::CronReceiptId::new();
            old.started_at -= 2 * 86400;
            let previous_time = scheduled - chrono::Duration::minutes(1);
            old.schedule = format!(
                "0 {} {} * * *",
                previous_time.minute(),
                previous_time.hour()
            );
            old.finished_at = Some(old.started_at + 1);
            old.outcome = loopflow::ops::CronOutcome::Failed;
            old.exit_code = Some(72);
            old.error = Some("historical scorecard failure".into());
            let path = loopflow::ops::receipt_root(&lf_home)
                .join("infrastructure/telemetry-daily")
                .join(format!("{}-{}.json", old.started_at, old.id));
            fs::write(path, serde_json::to_vec_pretty(&old).unwrap()).unwrap();
            original_receipts.push(old);
        }

        if let Some(parent) = interrupted.as_mut() {
            // A live exact runner cannot be displaced, even though its receipt
            // has no terminal result. No retry is reserved in this wake.
            assert!(run_cron(
                &agents,
                &release.wave,
                &release.flow,
                &host.machine_id,
                &host.machine_id,
                CronSource::Scheduled
            )
            .is_err());
            let first = release_history(
                &lf_home,
                &repo_path,
                &release.wave,
                5,
                Utc::now().timestamp(),
            )
            .unwrap();
            let attempt = first
                .obligations
                .iter()
                .flat_map(|r| &r.opportunities)
                .flat_map(|o| &o.attempts)
                .last()
                .unwrap();
            assert!(matches!(
                attempt.outcome,
                ScheduledReleaseOutcome::Deferred { .. }
            ));
            assert!(attempt
                .telemetry
                .as_ref()
                .unwrap()
                .recovery_receipt
                .is_none());
            assert!(!published.exists());
            parent.child.kill().unwrap();
            parent.child.wait().unwrap();

            // The controller is gone, but its check still owns the job lock.
            assert!(run_cron(
                &agents,
                &release.wave,
                &release.flow,
                &host.machine_id,
                &host.machine_id,
                CronSource::Scheduled
            )
            .is_err());
            let second = release_history(
                &lf_home,
                &repo_path,
                &release.wave,
                5,
                Utc::now().timestamp(),
            )
            .unwrap();
            let attempt = second
                .obligations
                .iter()
                .flat_map(|r| &r.opportunities)
                .flat_map(|o| &o.attempts)
                .last()
                .unwrap();
            assert!(matches!(
                attempt.outcome,
                ScheduledReleaseOutcome::Deferred { .. }
            ));
            assert!(
                attempt
                    .telemetry
                    .as_ref()
                    .unwrap()
                    .recovery_receipt
                    .is_some(),
                "confirmed runner death must reach the executor's surviving-child fence"
            );
            let reservation = attempt
                .telemetry
                .as_ref()
                .unwrap()
                .recovery_receipt
                .as_ref()
                .unwrap();
            let blocked = second
                .receipts
                .iter()
                .find(|r| &r.id == reservation)
                .unwrap();
            assert_eq!(blocked.outcome, loopflow::ops::CronOutcome::Failed);
            assert!(blocked.error.as_ref().unwrap().contains("already active"));
            assert_eq!(
                fs::read_to_string(&telemetry_calls)
                    .unwrap()
                    .lines()
                    .count(),
                1
            );
            assert!(!published.exists());
            fs::write(state.path().join("allow"), "").unwrap();
            wait_for(&state.path().join("completed"));
            // The completion marker precedes shell exit; acquire the same job
            // lock before the next firing so this assertion is not a timing guess.
            let key = hex::encode(sha2::Sha256::digest(
                serde_json::to_vec(&(&repo_path, "infrastructure", "telemetry-daily")).unwrap(),
            ));
            let lock = fs::File::open(lf_home.join("cron/locks").join(key)).unwrap();
            let deadline = Instant::now() + Duration::from_secs(20);
            while fs2::FileExt::try_lock_exclusive(&lock).is_err() {
                assert!(Instant::now() < deadline, "surviving check did not exit");
                thread::sleep(Duration::from_millis(10));
            }
            drop(lock);
        }

        let target_lock = if scenario == "release-overlap" {
            let locks = repo_path.join(".lf/locks");
            fs::create_dir_all(&locks).unwrap();
            let file = fs::File::create(locks.join(format!(
                "release-{}.lock",
                hex::encode(sha2::Sha256::digest("default"))
            )))
            .unwrap();
            fs2::FileExt::lock_exclusive(&file).unwrap();
            Some(file)
        } else {
            None
        };
        let mut seeded_attempt = None;
        if recovering {
            let directory = repo_path.join(".lf/locks");
            fs::create_dir_all(&directory).unwrap();
            let held = fs::File::create(directory.join(format!(
                "release-{}.lock",
                hex::encode(sha2::Sha256::digest("default"))
            )))
            .unwrap();
            fs2::FileExt::lock_exclusive(&held).unwrap();
            assert!(run_cron(
                &agents,
                &release.wave,
                &release.flow,
                &host.machine_id,
                &host.machine_id,
                CronSource::Scheduled
            )
            .is_err());
            drop(held);
            // The starting historical failure is seeded, like the historical due dates.
            // Subsequent inspection, retry, selection and settlement use the real CLI.
            for entry in fs::read_dir(lf_home.join("cron/obligations")).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().is_none_or(|e| e != "json") {
                    continue;
                }
                let mut record: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                if record["flow"] != "release-run" {
                    continue;
                }
                for opportunity in record["opportunities"].as_array_mut().unwrap() {
                    if scenario == "candidate-valid-closed" {
                        for key in ["due_at", "next_due_at"] {
                            opportunity[key] = (opportunity[key].as_i64().unwrap() - 86400).into();
                        }
                        opportunity["due_local"] = chrono::DateTime::from_timestamp(
                            opportunity["due_at"].as_i64().unwrap(),
                            0,
                        )
                        .unwrap()
                        .with_timezone(&Local)
                        .to_rfc3339()
                        .into();
                    }
                    if let Some(attempt) =
                        opportunity["attempts"].as_array_mut().unwrap().last_mut()
                    {
                        if scenario == "candidate-valid-closed" {
                            attempt["started_at"] =
                                (attempt["started_at"].as_i64().unwrap() - 86400).into();
                            if let Some(finished) = attempt["finished_at"].as_i64() {
                                attempt["finished_at"] = (finished - 86400).into();
                            }
                        }
                        attempt["selection"] = serde_json::json!({"tag":"v0.9.1", "commit": rejected_commit, "workflow_run_id":42});
                        attempt["outcome"] = serde_json::json!({"status":"failed", "cause":"packaged install preflight rejected"});
                        attempt["target"] = "default".into();
                        let evidence = b"packaged install preflight rejected";
                        let evidence_path = state.path().join("historical-preflight.txt");
                        fs::write(&evidence_path, evidence).unwrap();
                        attempt["verification"] = serde_json::json!([{
                            "name":"packaged-preflight", "subject":rejected_commit, "passed":false,
                            "evidence_path":evidence_path, "sha256":hex::encode(sha2::Sha256::digest(evidence))
                        }]);
                        seeded_attempt = Some(attempt.clone());
                    }
                }
                if scenario == "candidate-valid-closed" {
                    // Synthetic historical replacement; real execution below must resume
                    // the old candidate and materialize today's due in the new segment.
                    let boundary = scheduled
                        .with_second(0)
                        .unwrap()
                        .with_nanosecond(0)
                        .unwrap()
                        .timestamp()
                        - 1;
                    let mut successor = record.clone();
                    let id = format!("{}-successor", record["id"].as_str().unwrap());
                    successor["id"] = id.clone().into();
                    successor["replaces"] = record["id"].clone();
                    successor["activated_at"] = boundary.into();
                    successor["observed_at"] = boundary.into();
                    successor["opportunities"] = serde_json::json!([]);
                    record["closed_at"] = boundary.into();
                    fs::write(
                        path.with_file_name(format!("{id}.json")),
                        serde_json::to_vec_pretty(&successor).unwrap(),
                    )
                    .unwrap();
                }
                fs::write(path, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
            }
            assert!(seeded_attempt.is_some());
        }
        if matches!(
            scenario,
            "reconcile-empty" | "reconcile-branch" | "reconcile-missing"
        ) {
            let name = format!("verify-public-default-{rejected_commit}");
            let checkout = repo_path.parent().unwrap().join(format!(
                "{}.{}",
                repo_path.file_name().unwrap().to_str().unwrap(),
                name
            ));
            let branch = format!("jack/{name}");
            match scenario {
                "reconcile-empty" => fs::create_dir(&checkout).unwrap(),
                "reconcile-branch" => {
                    git(&repo_path, &["branch", &branch, &rejected_commit]);
                }
                _ => {
                    git(
                        &repo_path,
                        &[
                            "worktree",
                            "add",
                            "-b",
                            &branch,
                            checkout.to_str().unwrap(),
                            &rejected_commit,
                        ],
                    );
                    fs::remove_dir_all(&checkout).unwrap();
                }
            }
        }
        let mut killed_attempt = None;
        if scenario == "reconcile-interrupted" {
            let log = fs::File::create(state.path().join("controller.log")).unwrap();
            let child = Command::new(env!("CARGO_BIN_EXE_lf"))
                .args([
                    "cron",
                    "run",
                    "--wave",
                    "infrastructure",
                    "--flow",
                    "release-run",
                    "--scheduled",
                ])
                .current_dir(&repo_path)
                .env("LF_HOME", &lf_home)
                .stdout(Stdio::from(log.try_clone().unwrap()))
                .stderr(Stdio::from(log))
                .spawn()
                .unwrap();
            let mut parent = TelemetryParent {
                child,
                state: state.path().to_path_buf(),
            };
            wait_for(&state.path().join("ready"));
            let checkout = PathBuf::from(
                fs::read_to_string(state.path().join("checkout"))
                    .unwrap()
                    .trim(),
            );
            assert!(matches!(
                loopflow::ops::release_tag(&repo_path, "0.9.2", None),
                Err(loopflow::ops::OpsError::ReleaseDeferred { .. })
            ));
            assert!(loopflow::engine::git::worktree_remove(&repo_path, &checkout).is_err());
            assert_eq!(git(&checkout, &["rev-parse", "HEAD"]), rejected_commit);
            assert_eq!(caller_state(), before);
            let pending = release_history(
                &lf_home,
                &repo_path,
                &release.wave,
                5,
                Utc::now().timestamp(),
            )
            .unwrap();
            let attempt = pending
                .obligations
                .iter()
                .flat_map(|r| &r.opportunities)
                .flat_map(|o| &o.attempts)
                .next()
                .unwrap()
                .clone();
            assert!(!matches!(
                attempt.outcome,
                ScheduledReleaseOutcome::Published { .. }
            ));
            assert_eq!(pending.summary.published, 0);
            assert_eq!(fs::read_to_string(&published).unwrap(), "published\n");
            fs::write(state.path().join("allow"), "").unwrap();
            let deadline = Instant::now() + Duration::from_secs(20);
            while parent.child.try_wait().unwrap().is_none() {
                assert!(
                    Instant::now() < deadline,
                    "cron did not record controller death"
                );
                thread::sleep(Duration::from_millis(10));
            }
            assert!(!parent.child.wait().unwrap().success());
            let failed = release_history(
                &lf_home,
                &repo_path,
                &release.wave,
                5,
                Utc::now().timestamp(),
            )
            .unwrap();
            let physical = failed
                .receipts
                .iter()
                .find(|r| r.id == attempt.receipt_id)
                .unwrap()
                .clone();
            assert_eq!(physical.outcome, loopflow::ops::CronOutcome::Failed);
            assert_eq!(failed.summary.published, 0);
            // Cron can finish before its orphaned verifier releases inherited locks.
            let key = hex::encode(sha2::Sha256::digest(
                serde_json::to_vec(&(&repo_path, "infrastructure", "release-run")).unwrap(),
            ));
            let lock = fs::File::open(lf_home.join("cron/locks").join(key)).unwrap();
            let deadline = Instant::now() + Duration::from_secs(20);
            while fs2::FileExt::try_lock_exclusive(&lock).is_err() {
                assert!(
                    Instant::now() < deadline,
                    "verifier did not release job lock"
                );
                thread::sleep(Duration::from_millis(10));
            }
            drop(lock);
            killed_attempt = Some((attempt, physical));
        }
        let result = run_cron(
            &agents,
            &release.wave,
            &release.flow,
            &host.machine_id,
            &host.machine_id,
            CronSource::Scheduled,
        );
        let log =
            fs::read_to_string(repo_path.join(".lf/logs/cron.infrastructure.release-run.log"))
                .unwrap_or_default();
        let history = release_history(
            &lf_home,
            &repo_path,
            &release.wave,
            if scenario == "telemetry-history" {
                1
            } else {
                5
            },
            Utc::now().timestamp(),
        )
        .unwrap();
        let attempts: Vec<_> = history
            .obligations
            .iter()
            .flat_map(|r| &r.opportunities)
            .flat_map(|o| &o.attempts)
            .collect();
        assert_eq!(
            attempts.len(),
            if interrupted.is_some() {
                3
            } else if recovering || killed_attempt.is_some() {
                2
            } else {
                1
            },
            "{scenario}: {log}"
        );
        let attempt = *attempts.last().unwrap();
        assert!(attempt.covered.len() >= 3);
        if let Some((killed, physical)) = &killed_attempt {
            assert_eq!(attempt.selection, killed.selection);
            assert_eq!(attempt.covered, killed.covered);
            assert_eq!(
                history.receipts.iter().find(|r| r.id == physical.id),
                Some(physical)
            );
            assert!(matches!(
                attempt.outcome,
                ScheduledReleaseOutcome::Published { .. }
            ));
            assert_eq!(history.summary.published, 1);
            assert_eq!(fs::read_to_string(&published).unwrap(), "published\n");
            assert_eq!(caller_state(), before);
        }

        assert!(
            history.summary.qualifying_pairs.is_empty(),
            "synthetic historical coverage cannot qualify"
        );
        if let Some(seed) = &seeded_attempt {
            assert_eq!(
                serde_json::to_value(attempts[0]).unwrap(),
                *seed,
                "historical attempt changed"
            );
            if scenario == "candidate-valid-closed" {
                assert!(attempt.covered.len() > attempts[0].covered.len());
                assert!(attempts[0]
                    .covered
                    .iter()
                    .all(|id| attempt.covered.contains(id)));
                let owner = history
                    .obligations
                    .iter()
                    .find(|r| {
                        r.opportunities.iter().any(|o| {
                            o.attempts
                                .iter()
                                .any(|a| a.receipt_id == attempt.receipt_id)
                        })
                    })
                    .unwrap();
                assert!(owner.closed_at.is_some());
                assert_ne!(attempt.execution_obligation.as_ref(), Some(&owner.id));
                assert!(history.summary.closed_unsettled.is_empty());
                assert_eq!(history.summary.unresolved, 0);
                let output = Command::new(env!("CARGO_BIN_EXE_lf"))
                    .args([
                        "release",
                        "history",
                        "--wave",
                        "infrastructure",
                        "--days",
                        "5",
                    ])
                    .current_dir(&repo_path)
                    .output()
                    .unwrap();
                assert!(output.status.success(), "{output:?}");
                let text = String::from_utf8(output.stdout).unwrap();
                for id in &attempt.covered {
                    let line = text.lines().find(|line| line.starts_with(id)).unwrap();
                    assert!(line.contains("Published"), "{line}");
                }
                assert_eq!(
                    attempt.telemetry.as_ref().unwrap().original.len(),
                    attempt.covered.len()
                );
            } else {
                assert_eq!(attempt.covered, attempts[0].covered);
            }
            assert_eq!(
                git(&repo_path, &["rev-parse", baseline_tag]),
                rejected_commit
            );
        }
        match scenario {
            "candidate-unknown" | "candidate-partial" => {
                assert!(result.is_err(), "{scenario}: {log}");
                assert!(matches!(
                    attempt.outcome,
                    ScheduledReleaseOutcome::Failed { .. }
                ));
                assert_eq!(attempt.selection.as_ref().unwrap().commit, rejected_commit);
                assert!(attempt.replacement.is_none());
                assert!(!published.exists());
                assert_eq!(git(&repo_path, &["tag", "--list"]), baseline_tag);
                assert_eq!(history.summary.published + history.summary.no_change, 0);
            }
            "release-overlap" => {
                assert!(result.is_err(), "{scenario}: {log}");
                let owner = history
                    .obligations
                    .iter()
                    .flat_map(|r| &r.opportunities)
                    .find(|o| {
                        o.attempts
                            .iter()
                            .any(|a| a.receipt_id == attempt.receipt_id)
                    })
                    .unwrap();
                let ScheduledReleaseOutcome::Deferred {
                    reason,
                    continuation,
                } = &attempt.outcome
                else {
                    panic!("expected overlap deferral: {attempt:?}");
                };
                assert!(reason.contains("active execution"));
                assert!(
                    continuation.contains(&format!(
                        "next configured release due {}",
                        owner.next_due_at
                    )),
                    "{continuation}"
                );
                assert!(log.contains(continuation), "{log}");
                assert!(attempt.selection.is_none());
                assert!(attempt.telemetry.is_none());
                assert!(!published.exists());
                assert_eq!(history.summary.published + history.summary.no_change, 0);
                assert!(target_lock.is_some());
            }
            "telemetry-unknown" => {
                assert!(result.is_err(), "{scenario}: {log}");
                assert!(
                    matches!(&attempt.outcome, ScheduledReleaseOutcome::Deferred { reason, .. }
                if reason.contains("Unknown")),
                    "{attempt:?}"
                );
                assert!(attempt.selection.is_none());
                assert!(!published.exists());
                assert_eq!(history.summary.published + history.summary.no_change, 0);
            }
            "candidate-successor"
            | "candidate-valid"
            | "candidate-valid-closed"
            | "published"
            | "reconcile-interrupted"
            | "reconcile-empty"
            | "reconcile-branch"
            | "reconcile-missing"
            | "no-change"
            | "telemetry-recovered"
            | "telemetry-missing"
            | "telemetry-history"
            | "telemetry-interrupted" => {
                assert!(result.is_ok(), "{scenario}: {result:?}\n{log}");
                if scenario != "no-change" {
                    assert!(
                        matches!(attempt.outcome, ScheduledReleaseOutcome::Published { .. }),
                        "{attempt:?}"
                    );
                } else {
                    assert!(
                        matches!(attempt.outcome, ScheduledReleaseOutcome::NoChange { .. }),
                        "{attempt:?}"
                    );
                }
                assert_eq!(history.summary.published + history.summary.no_change, 1);
                if recovering {
                    assert_eq!(attempt.selection.as_ref().unwrap().tag, expected_tag);
                    if scenario == "candidate-successor" {
                        let replacement = attempt.replacement.as_ref().unwrap();
                        assert_eq!(replacement.rejected.commit, rejected_commit);
                        let proof: serde_json::Value = serde_json::from_slice(
                            &fs::read(&replacement.inspection.evidence_path).unwrap(),
                        )
                        .unwrap();
                        assert_eq!(proof["publications"], serde_json::json!([]));
                        assert_eq!(attempt.selection.as_ref().unwrap().commit, successor_commit);
                    } else {
                        assert!(attempt.replacement.is_none());
                    }
                }
                assert!(attempt
                    .verification
                    .iter()
                    .any(|p| p.name == "public-release" && p.passed));
                assert!(attempt
                    .verification
                    .iter()
                    .any(|p| p.name == "scheduled-telemetry" && p.passed));
            }
            _ => {
                assert!(result.is_err(), "{scenario}: {log}");
                assert!(
                    matches!(attempt.outcome, ScheduledReleaseOutcome::Failed { .. }),
                    "{scenario}: {attempt:?}\n{log}"
                );
                assert_eq!(history.summary.published + history.summary.no_change, 0);
                assert_eq!(
                    published.exists(),
                    scenario != "telemetry-failure",
                    "publication survives post-publication failure"
                );
                if scenario == "telemetry-failure" {
                    assert_eq!(history.summary.failed_verifications, 2);
                    assert!(attempt.selection.is_none());
                } else {
                    assert_eq!(attempt.selection.as_ref().unwrap().tag, "v0.9.1");
                }
            }
        }
        assert_eq!(caller_state(), before, "{scenario}: caller work changed");
        if scenario == "release-overlap" {
            let output = Command::new(env!("CARGO_BIN_EXE_lf"))
                .args([
                    "release",
                    "history",
                    "--wave",
                    "infrastructure",
                    "--days",
                    "5",
                    "--json",
                ])
                .current_dir(&repo_path)
                .env("LF_HOME", &lf_home)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let cli: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(cli["summary"]["published"], 0);
            assert!(String::from_utf8_lossy(&output.stdout).contains("next configured release due"));
            assert_eq!(git(&repo_path, &["tag", "--list"]), baseline_tag);
            assert_eq!(
                fs::read_to_string(&telemetry_calls)
                    .unwrap()
                    .lines()
                    .count(),
                1
            );
            assert_eq!(caller_state(), before, "history changed caller work");
            continue;
        }
        let prerequisite = attempt
            .telemetry
            .as_ref()
            .expect("retained prerequisite before mutation");
        assert_eq!(
            prerequisite
                .original
                .iter()
                .map(|p| &p.opportunity_id)
                .collect::<Vec<_>>(),
            attempt.covered.iter().collect::<Vec<_>>()
        );
        if scenario == "telemetry-history" {
            assert!(prerequisite
                .original
                .iter()
                .all(|p| p.due_at.is_some() && p.uncertainty.is_none()));
            assert!(prerequisite
                .original
                .iter()
                .any(|p| p.receipts.contains(&original_receipts[1].id)));
            let segments: std::collections::HashSet<_> = prerequisite
                .original
                .iter()
                .map(|p| p.obligation_id.as_ref().unwrap())
                .collect();
            assert_eq!(
                segments.len(),
                2,
                "catch-up retains both historical schedules"
            );
            for id in segments {
                let segment = history.obligations.iter().find(|s| &s.id == id).unwrap();
                assert_eq!(segment.flow, "telemetry-daily");
                assert!(segment.opportunities.is_empty());
            }
            assert_eq!(history.summary.failed_verifications, 1);
        } else {
            assert!(
                prerequisite
                    .original
                    .iter()
                    .all(|p| p.due_at.is_none() && p.uncertainty.is_some()),
                "pre-observation schedule/timezone must remain unknown"
            );
        }
        for original in &original_receipts {
            assert_eq!(
                history.receipts.iter().find(|r| r.id == original.id),
                Some(original),
                "retry must not rewrite historical failure"
            );
        }
        let recovered = matches!(
            scenario,
            "telemetry-failure"
                | "telemetry-recovered"
                | "telemetry-missing"
                | "telemetry-interrupted"
        );
        assert_eq!(prerequisite.recovery_receipt.is_some(), recovered);
        let calls = fs::read_to_string(&telemetry_calls)
            .unwrap()
            .lines()
            .count();
        assert_eq!(
            calls,
            if recovered && scenario != "telemetry-missing" {
                2
            } else {
                1
            },
            "one prerequisite retry per wake, not per collapsed due"
        );
        if let Some(id) = &prerequisite.recovery_receipt {
            let receipt = history.receipts.iter().find(|r| &r.id == id).unwrap();
            assert_eq!(receipt.source, CronSource::Recovery);
            if scenario != "telemetry-failure" {
                let proof = attempt
                    .verification
                    .iter()
                    .find(|p| p.name == "scheduled-telemetry" && p.passed)
                    .unwrap();
                assert_eq!(proof.subject, id.as_str());
            }
        }
        if scenario == "candidate-valid-closed" {
            // Reconstruct the crash boundary after the simulated external publication
            // but before the atomic settlement write. The next wake uses real dispatch.
            let owner = history
                .obligations
                .iter()
                .find(|record| {
                    record.opportunities.iter().any(|o| {
                        o.attempts
                            .iter()
                            .any(|a| a.receipt_id == attempt.receipt_id)
                    })
                })
                .unwrap();
            let mut interrupted = owner.clone();
            let pending = interrupted
                .opportunities
                .iter_mut()
                .flat_map(|o| &mut o.attempts)
                .find(|a| a.receipt_id == attempt.receipt_id)
                .unwrap();
            pending.outcome = ScheduledReleaseOutcome::Running;
            pending.verification.clear();
            pending.finished_at = None;
            fs::write(
                lf_home
                    .join("cron/obligations")
                    .join(format!("{}.json", owner.id)),
                serde_json::to_vec_pretty(&interrupted).unwrap(),
            )
            .unwrap();
            assert_eq!(fs::read_to_string(&published).unwrap().lines().count(), 1);
            let result = run_cron(
                &agents,
                &release.wave,
                &release.flow,
                &host.machine_id,
                &host.machine_id,
                CronSource::Scheduled,
            );
            assert!(result.is_ok(), "{result:?}");
            let recovered = release_history(
                &lf_home,
                &repo_path,
                &release.wave,
                5,
                Utc::now().timestamp(),
            )
            .unwrap();
            let owner = recovered
                .obligations
                .iter()
                .find(|r| r.id == owner.id)
                .unwrap();
            let attempts: Vec<_> = owner
                .opportunities
                .iter()
                .flat_map(|o| &o.attempts)
                .collect();
            let resumed = attempts.last().unwrap();
            assert_eq!(attempts.len(), 3);
            assert_eq!(resumed.selection, attempt.selection);
            assert_eq!(resumed.covered, attempt.covered);
            assert!(matches!(
                resumed.outcome,
                ScheduledReleaseOutcome::Published { .. }
            ));
            assert_eq!(recovered.summary.published, 1);
            assert_eq!(recovered.summary.unresolved, 0);
            assert!(recovered.summary.qualifying_pairs.is_empty());
            assert_eq!(
                fs::read_to_string(&published).unwrap().lines().count(),
                1,
                "recovery must reconcile the existing external publication"
            );
            assert_eq!(
                caller_state(),
                before,
                "publication recovery changed caller work"
            );
        }
        if scenario == "telemetry-recovered" {
            assert_eq!(history.summary.failed_verifications, 1);
            assert!(history
                .summary
                .undispositioned_failures
                .contains(&original_receipts[0].id.to_string()));
        }
    }
}

struct TelemetryParent {
    child: Child,
    state: PathBuf,
}

impl Drop for TelemetryParent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::write(self.state.join("allow"), "");
    }
}

fn wait_for(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !path.exists() {
        assert!(Instant::now() < deadline, "waiting for {}", path.display());
        thread::sleep(Duration::from_millis(10));
    }
}
