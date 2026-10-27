//! Set up each area of work once: a Wave's committed `config.yaml` and your
//! own Machine and account routes choose how its work runs, with no flags.
use std::path::Path;
use std::process::Command;

use loopflow::durable::{DefaultScope, MachineSetting};
use loopflow::profile::{ProviderRoute, RouteScope};
use loopflow::provider_auth::Provider;
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::ProviderAccountId;
use loopflow_test_support::TestRepo;
use serde_json::Value;

fn lf(repo: &Path, home: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("PATH", "/usr/bin:/bin")
        .env("NO_COLOR", "1")
        .current_dir(repo)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap()
}

fn json(output: std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// The agent a launch resolves, from the reproducible command `-v` prints
/// before starting it. No provider is installed, so nothing runs.
fn launch_agent(repo: &Path, home: &Path, wave: Option<&str>) -> String {
    let mut args = vec!["-v", "-b"];
    if let Some(wave) = wave {
        args.extend(["--wave", wave]);
    }
    args.extend(["skill", "probe"]);
    let output = lf(repo, home, &args);
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr
        .lines()
        .find_map(|line| line.trim().strip_prefix("lf ")?.split(" -a ").nth(1))
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or_else(|| panic!("no launch command in {stderr}"))
        .to_owned()
}

#[test]
fn each_wave_runs_its_own_setup_and_the_repository_is_the_default() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(
        ".lf/config.yaml",
        "workflow: repo-flow\nagent: codex\ndiff: false\ndiff_files: false\npaste: false\n",
    );
    repo.create_file(".lf/skills/probe.md", "Probe.");
    repo.create_file(".lf/wave/infra/GOAL.md", "Keep it boring.\n");
    repo.create_file(
        ".lf/wave/infra/config.yaml",
        "workflow: code\nagent: claude\nskill_agents:\n  probe: claude:opus\n",
    );
    repo.create_file(".lf/wave/research/GOAL.md", "Find out.\n");
    repo.create_file(".lf/wave/research/config.yaml", "agent: codex:o3\n");
    repo.create_file(".lf/wave/plain/GOAL.md", "Inherit everything.\n");
    repo.stage_all();
    repo.commit("Set up three areas of work");

    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let checkout = repo.path().to_str().unwrap();
    let infra = store.ensure_wave(checkout, "infra").unwrap();
    let research = store.ensure_wave(checkout, "research").unwrap();
    store.ensure_wave(checkout, "plain").unwrap();
    let here = store.local_machine().unwrap().id;
    store
        .set_machine_default(
            &DefaultScope::Wave(infra.clone()),
            MachineSetting::Tasks,
            &here,
        )
        .unwrap();
    store
        .upsert_provider_account(&loopflow::store::ProviderAccount {
            provider: "codex".into(),
            account_id: ProviderAccountId::parse("work").unwrap(),
            home: None,
            login_email: None,
            observed_email: None,
            observed_subject: None,
            observed_credential_digest: None,
            observed_plan: None,
            credential_state: loopflow::store::CredentialState::Connected,
            routing_state: loopflow::store::RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: 1,
            updated_at: 1,
        })
        .unwrap();
    store
        .set_provider_route(&ProviderRoute {
            scope: RouteScope::Wave(research.clone()),
            provider: Provider::Codex,
            accounts: vec![ProviderAccountId::parse("work").unwrap()],
            created_at: 1,
            updated_at: 1,
        })
        .unwrap();
    drop(store);

    // Launches take their Wave's agent, per skill first; no Wave is the repository's.
    assert_eq!(
        launch_agent(repo.path(), home.path(), Some("infra")),
        "claude:opus"
    );
    assert_eq!(
        launch_agent(repo.path(), home.path(), Some("research")),
        "codex:o3"
    );
    assert_eq!(launch_agent(repo.path(), home.path(), None), "codex");

    let setup = |wave: &str| {
        json(lf(
            repo.path(),
            home.path(),
            &["wave", "status", wave, "--json"],
        ))["setup"]
            .clone()
    };
    let infra = setup("infra");
    assert_eq!(infra["workflow"]["value"], "code");
    assert_eq!(infra["workflow"]["source"], "wave");
    assert_eq!(infra["agent"]["value"], "claude");
    assert_eq!(infra["skill_agents"]["probe"]["value"], "claude:opus");
    assert_eq!(infra["tasks_machine"]["source"], "wave");
    assert_eq!(infra["session_machine"]["source"], "default");
    assert_eq!(infra["accounts"]["codex"]["source"], "default");

    let research = setup("research");
    assert_eq!(research["workflow"]["value"], "repo-flow");
    assert_eq!(research["workflow"]["source"], "repo");
    assert_eq!(research["agent"]["value"], "codex:o3");
    assert_eq!(research["agent"]["source"], "wave");
    assert_eq!(research["tasks_machine"]["source"], "default");
    assert_eq!(research["accounts"]["codex"]["accounts"][0], "work");
    assert_eq!(research["accounts"]["codex"]["source"], "wave");

    let plain = setup("plain");
    assert_eq!(plain["workflow"]["source"], "repo");
    assert_eq!(plain["agent"]["value"], "codex");
    assert_eq!(plain["agent"]["source"], "repo");
}

#[test]
fn a_wave_file_rejects_repository_settings() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(".lf/skills/probe.md", "Probe.");
    repo.create_file(".lf/wave/infra/GOAL.md", "Keep it boring.\n");
    repo.create_file(".lf/wave/infra/config.yaml", "release:\n  targets: {}\n");
    repo.stage_all();
    repo.commit("Misplace a repository setting");
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    store
        .ensure_wave(repo.path().to_str().unwrap(), "infra")
        .unwrap();
    drop(store);
    let output = lf(
        repo.path(),
        home.path(),
        &["-b", "--wave", "infra", "skill", "probe"],
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("`release` belongs to the repository"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
