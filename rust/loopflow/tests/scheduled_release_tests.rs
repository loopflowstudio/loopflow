mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{Local, Timelike, Utc};
use loopflow::durable::HomeId;
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
        "telemetry-history",
        "smoke-failure",
        "missing-stage",
    ]);
}

#[test]
fn interrupted_telemetry_recovers_only_after_its_runner_and_child_exit() {
    run_scenarios(&["telemetry-interrupted"]);
}

#[test]
fn legacy_telemetry_without_runner_identity_stays_unresolved() {
    run_scenarios(&["telemetry-unknown"]);
}

fn run_scenarios(scenarios: &[&str]) {
    for &scenario in scenarios {
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
    commit=$(git rev-parse v0.9.1)
    printf '[{{"databaseId":42,"headBranch":"v0.9.1","headSha":"%s","status":"completed","conclusion":"success","url":"https://example.test/run/42"}}]\n' "$commit" ;;
  'release view')
    [ -f '{}' ] || exit 1
    printf '{{"isDraft":false}}\n' ;;
  'run download') exit 0 ;;
  'pr list') printf '[]\n' ;;
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
  publish) printf 'published\n' > '{}' ;;
  reconcile)
    [ '{scenario}' != smoke-failure ] || {{ echo 'exact-tag smoke failed' >&2; exit 73; }}
    mkdir -p "$LF_RELEASE_MAIN_REPO/.lf/logs"
    cp '{}' "$LF_RELEASE_MAIN_REPO/.lf/logs/release.v0.9.1.verified.json" ;;
  *) echo "unexpected publisher stage: $1" >&2; exit 93 ;;
esac
"#,
                published.display(),
                public_proof.display(),
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
        repo.create_file(".lf/flows/release-run.yaml", "- op: release run patch\n");
        repo.create_file("wave/infrastructure/GOAL.md", "# Infrastructure\n");
        repo.stage_all();
        repo.commit("Release fixture");
        repo.push();
        for args in [["tag", "v0.9.1"], ["push", "--tags"]] {
            git(repo.path(), &args);
        }
        let stages: Vec<_> = [
            "ui_host_verified",
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
                "tag": "v0.9.1", "source_commit": repo.head_sha(), "workflow_run_id": "42",
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
            home_id: HomeId::new(),
            lf_home: lf_home.clone(),
            db_path: lf_home.join("loopflow.db"),
            path_env: std::env::var("PATH").unwrap(),
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        host.home_id = runtime.block_on(async {
            let store = loopflow::store::open_ephemeral_store(
                &loopflow::store::StorageConfig::sqlite(host.db_path.clone()),
            )
            .await
            .unwrap();
            let wave = loopflow::work::wave::Wave::new(
                loopflow::id::WaveId::new(),
                "infrastructure".into(),
                repo_path.display().to_string(),
            );
            store.create_wave(&wave).await.unwrap();
            store.local_home().await.unwrap().id
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
                .env("LF_DB_PATH", &host.db_path)
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
                &host.home_id,
                &host.home_id,
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

        // Model an already-installed daily obligation in a disposable Home. No
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
            let record_path = fs::read_dir(lf_home.join("cron/obligations"))
                .unwrap()
                .map(|e| e.unwrap().path())
                .find(|p| p.extension().is_some_and(|e| e == "json"))
                .unwrap();
            let mut record: serde_json::Value =
                serde_json::from_slice(&fs::read(&record_path).unwrap()).unwrap();
            record["observed_at"] = activation.into();
            fs::write(record_path, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
            let mut old = original_receipts[0].clone();
            old.id = loopflow::durable::CronReceiptId::new();
            old.started_at -= 2 * 86400;
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
                &host.home_id,
                &host.home_id,
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
                &host.home_id,
                &host.home_id,
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

        let result = run_cron(
            &agents,
            &release.wave,
            &release.flow,
            &host.home_id,
            &host.home_id,
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
            if interrupted.is_some() { 3 } else { 1 },
            "{scenario}: {log}"
        );
        let attempt = *attempts.last().unwrap();
        assert!(attempt.covered.len() >= 3);
        assert!(
            history.summary.qualifying_pairs.is_empty(),
            "synthetic historical coverage cannot qualify"
        );
        match scenario {
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
            "published"
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
        if scenario == "telemetry-recovered" {
            assert_eq!(history.summary.failed_verifications, 1);
            assert!(history
                .summary
                .undispositioned_failures
                .contains(&original_receipts[0].id.to_string()));
        }
        assert_eq!(caller_state(), before, "{scenario}: caller work changed");
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
