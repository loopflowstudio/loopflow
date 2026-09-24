mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{Local, Timelike, Utc};
use loopflow::durable::HomeId;
use loopflow::ops::cron::accounting::ScheduledReleaseOutcome;
use loopflow::ops::cron::history::release_history;
use loopflow::ops::{
    add_cron, run_cron, schedule_from_cron, CronHost, CronSource, CronSpec, CronTargetKind,
    SystemLaunchctl,
};
use loopflow_test_support::TestRepo;
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
    for scenario in [
        "published",
        "no-change",
        "telemetry-failure",
        "smoke-failure",
        "missing-stage",
    ] {
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
        let host = CronHost {
            home_id: HomeId::new(),
            lf_home: lf_home.clone(),
            db_path: lf_home.join("loopflow.db"),
            path_env: std::env::var("PATH").unwrap(),
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
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
        });
        let scheduled = Local::now() - chrono::Duration::minutes(1);
        let schedule = schedule_from_cron(&format!(
            "0 {} {} * * *",
            scheduled.minute(),
            scheduled.hour()
        ))
        .unwrap();
        let telemetry = CronSpec {
            wave: "infrastructure".into(),
            flow: "telemetry-daily".into(),
            target_kind: CronTargetKind::Flow,
            schedule: schedule.clone(),
            working_directory: repo_path.clone(),
            // Only the external verification is simulated. The release uses the real CLI.
            lf_path: PathBuf::from(if scenario == "telemetry-failure" {
                "/usr/bin/false"
            } else {
                "/usr/bin/true"
            }),
            host: host.clone(),
        };
        add_cron(&agents, &telemetry, &SystemLaunchctl).unwrap();
        let telemetry_result = run_cron(
            &agents,
            &telemetry.wave,
            &telemetry.flow,
            &host.home_id,
            &host.home_id,
            CronSource::Scheduled,
        );
        assert_eq!(telemetry_result.is_err(), scenario == "telemetry-failure");

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
            5,
            Utc::now().timestamp(),
        )
        .unwrap();
        let attempts: Vec<_> = history
            .obligations
            .iter()
            .flat_map(|r| &r.opportunities)
            .flat_map(|o| &o.attempts)
            .collect();
        assert_eq!(attempts.len(), 1, "{scenario}: {log}");
        let attempt = attempts[0];
        assert!(attempt.covered.len() >= 3);
        assert!(
            history.summary.qualifying_pairs.is_empty(),
            "synthetic historical coverage cannot qualify"
        );
        match scenario {
            "published" | "no-change" => {
                assert!(result.is_ok(), "{scenario}: {result:?}\n{log}");
                if scenario == "published" {
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
                    assert_eq!(history.summary.failed_verifications, 1);
                    assert!(attempt.selection.is_none());
                } else {
                    assert_eq!(attempt.selection.as_ref().unwrap().tag, "v0.9.1");
                }
            }
        }
        assert_eq!(caller_state(), before, "{scenario}: caller work changed");
    }
}
