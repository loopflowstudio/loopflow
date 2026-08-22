use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use loopflow::build_info::MigrationAuthority;
use loopflow::lf::commands::install::{
    CandidateIdentity, Compatibility, ExecutableCompatibility, PromotionPreview, Verdict,
};
use loopflow::machine_install::{
    account_home, authorize, clear_switch, entry_gate_path, read_state, root_for_home,
    settle_switch, write_active, write_switch, ActivationTargets, ActiveInstall, ArtifactIdentity,
    ArtifactRole, ArtifactSet, ForkEvidence, InstallSelection, InstallSource, MachineInstallState,
    RecoveryOwner, SwitchPhase, SwitchReceipt, WorkDisposition, WorkDispositionReceipt,
};
use loopflow::store::sqlite::SqliteStore;
use sha2::{Digest, Sha256};

fn write_executable(path: &Path, contents: impl AsRef<[u8]>) {
    fs::write(path, contents).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn account_home_interposer(directory: &Path) -> PathBuf {
    let source = directory.join("account_home.c");
    let library = if cfg!(target_os = "macos") {
        directory.join("libaccount_home.dylib")
    } else {
        directory.join("libaccount_home.so")
    };
    let function_name = if cfg!(target_os = "macos") {
        "lf_test_getpwuid_r"
    } else {
        "getpwuid_r"
    };
    let interpose = if cfg!(target_os = "macos") {
        r#"__attribute__((used)) static struct {
    const void *replacement;
    const void *replacee;
} lf_test_interpose __attribute__((section("__DATA,__interpose"))) = {
    (const void *)(unsigned long)&lf_test_getpwuid_r,
    (const void *)(unsigned long)&getpwuid_r
};"#
    } else {
        ""
    };
    fs::write(
        &source,
        format!(
            r#"#include <errno.h>
#include <pwd.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>

int {function_name}(uid_t uid, struct passwd *pwd, char *buf, size_t buflen,
                    struct passwd **result) {{
    const char *home = getenv("LF_TEST_ACCOUNT_HOME");
    const char *name = "loopflow-test";
    size_t home_len = strlen(home) + 1;
    size_t name_len = strlen(name) + 1;
    if (buflen < home_len + name_len) return ERANGE;
    memset(pwd, 0, sizeof(*pwd));
    memcpy(buf, home, home_len);
    memcpy(buf + home_len, name, name_len);
    pwd->pw_dir = buf;
    pwd->pw_name = buf + home_len;
    pwd->pw_uid = uid;
    *result = pwd;
    return 0;
}}

{interpose}
"#
        ),
    )
    .unwrap();
    let mut compiler = Command::new("cc");
    if cfg!(target_os = "macos") {
        compiler.args(["-dynamiclib", "-o"]);
    } else {
        compiler.args(["-shared", "-fPIC", "-o"]);
    }
    let status = compiler.arg(&library).arg(&source).status().unwrap();
    assert!(
        status.success(),
        "failed to compile account-home interposer"
    );
    library
}

fn test_tmux_socket_dir(account_home: &Path) -> PathBuf {
    account_home
        .parent()
        .expect("test account Home has a parent")
        .join("tmux")
}

fn isolated_command(binary: &Path, account_home: &Path, interposer: &Path) -> Command {
    let tmux_socket_dir = test_tmux_socket_dir(account_home);
    fs::create_dir_all(&tmux_socket_dir).unwrap();
    fs::set_permissions(&tmux_socket_dir, fs::Permissions::from_mode(0o700)).unwrap();
    let mut command = Command::new(binary);
    command
        .env("LF_TEST_ACCOUNT_HOME", account_home)
        .env("TMUX_TMPDIR", tmux_socket_dir)
        .env_remove("LF_HOME")
        .env_remove("LF_DB_PATH")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_RUN_CONTEXT")
        .env_remove("LF_RUN_LEASE")
        .env_remove("LF_AGENT_INVOCATION_ID");
    if cfg!(target_os = "macos") {
        command
            .env("DYLD_INSERT_LIBRARIES", interposer)
            .env("DYLD_FORCE_FLAT_NAMESPACE", "1");
    } else {
        command.env("LD_PRELOAD", interposer);
    }
    command
}

struct TestTmuxSession {
    name: String,
    socket_dir: PathBuf,
}

impl TestTmuxSession {
    fn new(name: String, account_home: &Path) -> Self {
        Self {
            name,
            socket_dir: test_tmux_socket_dir(account_home),
        }
    }

    fn start(name: String, account_home: &Path) -> Self {
        let session = Self::new(name, account_home);
        let status = Command::new("tmux")
            .args(["new-session", "-d", "-s", &session.name, "sleep", "300"])
            .env("TMUX_TMPDIR", &session.socket_dir)
            .status()
            .unwrap();
        assert!(status.success(), "failed to start isolated tmux session");
        session
    }

    fn is_live(&self) -> bool {
        Command::new("tmux")
            .args(["has-session", "-t", &self.name])
            .env("TMUX_TMPDIR", &self.socket_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    fn kill(&self) {
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", &self.name])
            .env("TMUX_TMPDIR", &self.socket_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

impl Drop for TestTmuxSession {
    fn drop(&mut self) {
        self.kill();
    }
}

fn copy_executable(source: &Path, target: &Path) {
    fs::copy(source, target).unwrap();
    fs::set_permissions(target, fs::Permissions::from_mode(0o755)).unwrap();
}

fn copy_build_source(source: &Path, destination: &Path) {
    fs::create_dir(destination).unwrap();
    fs::set_permissions(destination, fs::metadata(source).unwrap().permissions()).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source = entry.path();
        let destination = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source).unwrap();
        if metadata.is_dir() {
            copy_build_source(&source, &destination);
        } else if metadata.is_file() {
            fs::copy(&source, &destination).unwrap();
            fs::set_permissions(&destination, metadata.permissions()).unwrap();
        } else {
            panic!("unsupported fixture source entry {}", source.display());
        }
    }
}

struct PromotionBinaries {
    development_cli: PathBuf,
    development_daemon: PathBuf,
    changed_cli: PathBuf,
    changed_daemon: PathBuf,
    published_cli: PathBuf,
    published_daemon: PathBuf,
}

fn build_promotion_binaries(directory: &Path) -> PromotionBinaries {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    let source = directory.join("promotion-source");
    fs::create_dir(&source).unwrap();
    for name in ["Cargo.toml", "Cargo.lock"] {
        fs::copy(repo.join(name), source.join(name)).unwrap();
    }
    copy_build_source(&repo.join("rust"), &source.join("rust"));
    fs::create_dir(source.join("scripts")).unwrap();
    fs::copy(
        repo.join("scripts/canonicalize_migrations.py"),
        source.join("scripts/canonicalize_migrations.py"),
    )
    .unwrap();
    let drafts = source.join("rust/loopflow/src/store/migrations/drafts");
    fs::write(
        drafts.join("promotion_beta__11111111111111111111111111111111.sql"),
        "-- name: promotion_beta\n-- id: 11111111111111111111111111111111\n-- depends_on:\nCREATE TABLE local_promotion_release_order (\n    position INTEGER PRIMARY KEY,\n    name TEXT NOT NULL\n);\nINSERT INTO local_promotion_release_order VALUES (1, 'beta');\n",
    )
    .unwrap();
    let coda = drafts.join("promotion_coda__22222222222222222222222222222222.sql");
    let original_coda = "-- name: promotion_coda\n-- id: 22222222222222222222222222222222\n-- depends_on: promotion_beta\nINSERT INTO local_promotion_release_order VALUES (2, 'coda');\n";
    fs::write(&coda, original_coda).unwrap();

    let target = directory.join("promotion-target");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let build = |provenance: &str, authority: &str| {
        Command::new(&cargo)
            .current_dir(&source)
            .args([
                "build", "--quiet", "-p", "loopflow", "--bin", "lf", "--bin", "lfd",
            ])
            .env("CARGO_TARGET_DIR", &target)
            .env("LOOPFLOW_BUILD_PROVENANCE", provenance)
            .env("LOOPFLOW_MIGRATION_AUTHORITY", authority)
            .status()
            .unwrap()
    };
    let status = build("development", "validation_only");
    assert!(
        status.success(),
        "failed to build development controller fixture"
    );
    let development_cli = directory.join("development-lf");
    let development_daemon = directory.join("development-lfd");
    copy_executable(&target.join("debug/lf"), &development_cli);
    copy_executable(&target.join("debug/lfd"), &development_daemon);

    fs::write(
        &coda,
        "-- name: promotion_coda\n-- id: 22222222222222222222222222222222\n-- depends_on: promotion_beta\nINSERT INTO local_promotion_release_order VALUES (2, 'coda-changed');\n",
    )
    .unwrap();
    let status = build("development", "validation_only");
    assert!(status.success(), "failed to build changed-draft fixture");
    let changed_cli = directory.join("changed-lf");
    let changed_daemon = directory.join("changed-lfd");
    copy_executable(&target.join("debug/lf"), &changed_cli);
    copy_executable(&target.join("debug/lfd"), &changed_daemon);

    fs::write(&coda, original_coda).unwrap();

    let materialized = Command::new("python3")
        .current_dir(&source)
        .args([
            "scripts/canonicalize_migrations.py",
            env!("CARGO_PKG_VERSION"),
            "--materialize-for-tests",
        ])
        .status()
        .unwrap();
    assert!(
        materialized.success(),
        "failed to materialize published migration"
    );
    let status = build("release", "published");
    assert!(
        status.success(),
        "failed to build published controller fixture"
    );
    let published_cli = directory.join("published-lf");
    let published_daemon = directory.join("published-lfd");
    copy_executable(&target.join("debug/lf"), &published_cli);
    copy_executable(&target.join("debug/lfd"), &published_daemon);

    PromotionBinaries {
        development_cli,
        development_daemon,
        changed_cli,
        changed_daemon,
        published_cli,
        published_daemon,
    }
}

fn artifact_set(id: &str, source: InstallSource, cli: &Path, daemon: &Path) -> ArtifactSet {
    let mut digest = Sha256::new();
    for path in [cli, daemon] {
        digest.update(hex::encode(Sha256::digest(fs::read(path).unwrap())).as_bytes());
    }
    ArtifactSet {
        id: id.to_string(),
        source,
        source_revision: format!("revision-{id}"),
        source_identity: id.to_string(),
        content_sha256: hex::encode(digest.finalize()),
        artifacts: vec![
            ArtifactIdentity::capture(ArtifactRole::Cli, cli).unwrap(),
            ArtifactIdentity::capture(ArtifactRole::Daemon, daemon).unwrap(),
        ],
    }
}

fn candidate_artifact_set(
    id: &str,
    source: InstallSource,
    candidate: &CandidateIdentity,
    cli: &Path,
    daemon: &Path,
) -> ArtifactSet {
    let mut set = artifact_set(id, source, cli, daemon);
    set.source_revision = candidate.source_revision.clone();
    set.source_identity = candidate.source_identity.clone();
    set
}

fn selection(
    id: &str,
    source: InstallSource,
    artifact_set: ArtifactSet,
    store: &Path,
) -> InstallSelection {
    InstallSelection {
        installation_id: id.to_string(),
        source,
        artifact_set,
        store: store.canonicalize().unwrap(),
    }
}

fn _apply_embedded_drafts(store: &Path) {
    let drafts = loopflow::build_info::migration_draft_manifest();
    if drafts.is_empty() {
        return;
    }
    let connection = rusqlite::Connection::open(store).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE development_migrations (
                 position INTEGER NOT NULL UNIQUE,
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL UNIQUE,
                 checksum TEXT NOT NULL,
                 applied_at INTEGER NOT NULL
             );",
        )
        .unwrap();
    for (position, draft) in drafts.iter().enumerate() {
        connection.execute_batch(draft.sql).unwrap();
        connection
            .execute(
                "INSERT INTO development_migrations (
                     position, id, name, checksum, applied_at
                 ) VALUES (?1, ?2, ?3, ?4, 1)",
                rusqlite::params![position as i64, draft.id, draft.name, draft.checksum],
            )
            .unwrap();
    }
}

fn switch_receipt(
    id: &str,
    prior: InstallSelection,
    target: InstallSelection,
    published_fallback: ArtifactSet,
    target_published_fallback: Option<ArtifactSet>,
    first_fork: ForkEvidence,
    work_dispositions: Vec<WorkDispositionReceipt>,
) -> SwitchReceipt {
    let directory = target.store.parent().unwrap().to_path_buf();
    SwitchReceipt {
        schema_version: 1,
        id: id.to_string(),
        coordinator: prior
            .artifact_set
            .artifact(&ArtifactRole::Cli)
            .unwrap()
            .clone(),
        candidate: target
            .artifact_set
            .artifact(&ArtifactRole::Cli)
            .unwrap()
            .clone(),
        prior,
        target,
        published_fallback,
        target_published_fallback,
        phase: SwitchPhase::Planned,
        recovery_owner: RecoveryOwner::Coordinator,
        target_store_advance_started: false,
        target_store_advanced: false,
        active_selection_committed: false,
        activation: ActivationTargets {
            cli: directory.join("active-lf"),
            daemon: directory.join("active-lfd"),
            app: None,
            legacy_app: None,
        },
        app_was_running: false,
        disposable_store_owned: false,
        interrupted_work: Vec::new(),
        first_fork: Some(first_fork),
        work_dispositions,
    }
}

#[test]
fn local_promotion_controller_bootstraps_reuses_dev_and_returns_to_published() {
    let directory = tempfile::tempdir().unwrap();
    let account_home = directory.path().join("account");
    let public = directory.path().join("public");
    let repo = directory.path().join("repo");
    fs::create_dir_all(&account_home).unwrap();
    fs::create_dir_all(&public).unwrap();
    fs::create_dir_all(&repo).unwrap();
    let account_home = account_home.canonicalize().unwrap();
    let public = public.canonicalize().unwrap();
    let interposer = account_home_interposer(directory.path());
    let binaries = build_promotion_binaries(directory.path());

    let reliable = account_home.join(".lf/loopflow.db");
    fs::create_dir_all(reliable.parent().unwrap()).unwrap();
    let seed = directory.path().join("published-seed.db");
    let initialized = isolated_command(&binaries.development_cli, &account_home, &interposer)
        .env("LF_DB_PATH", &seed)
        .args(["doctor", "--json"])
        .output()
        .unwrap();
    assert!(
        initialized.status.success(),
        "{}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    fs::copy(seed, &reliable).unwrap();
    let connection = rusqlite::Connection::open(&reliable).unwrap();
    let seeded_frontier: String = connection
        .query_row(
            "SELECT version FROM schema_migrations ORDER BY rowid DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        seeded_frontier,
        CandidateIdentity::current().latest_known_migration
    );
    let home_id: String = connection
        .query_row("SELECT id FROM homes WHERE route='local'", [], |row| {
            row.get(0)
        })
        .unwrap();
    let wave_id = loopflow::id::WaveId::new();
    let tui_session = TestTmuxSession::start(
        format!("promotion-tui-{}", &wave_id.as_str()[5..13]),
        &account_home,
    );
    connection
        .execute(
            "INSERT INTO waves (id, name, repo, created_at)
             VALUES (?1, 'promotion-test', ?2, 1)",
            rusqlite::params![wave_id.as_str(), repo.display().to_string()],
        )
        .unwrap();
    let epoch_id = loopflow::durable::EpochId::new();
    connection
        .execute(
            "INSERT INTO epochs (
                id, number, wave_id, project_id, task_id, state, current_rev,
                created_at, terminal_at
             ) VALUES (?1, 1, ?2, NULL, NULL, 'open', 0, 1, NULL)",
            rusqlite::params![epoch_id.as_str(), wave_id.as_str()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO work_placements (wave_id, home_id, placed_at, enabled)
             VALUES (?1, ?2, 1, 1)",
            rusqlite::params![wave_id.as_str(), home_id.as_str()],
        )
        .unwrap();
    let drained_run_id = loopflow::durable::RunId::new();
    connection
        .execute(
            "INSERT INTO runs (
                id, epoch_id, home_id, state, trigger_json,
                source_kind, source_id, created_at, containment_kind,
                containment_id, cwd, started_at
             ) VALUES (?1, ?2, ?3, 'active', ?4, 'wave', ?5, 2,
                       'tmux', ?6, ?7, 2)",
            rusqlite::params![
                drained_run_id.as_str(),
                epoch_id.as_str(),
                home_id.as_str(),
                r#"{"kind":"user"}"#,
                wave_id.as_str(),
                tui_session.name,
                repo.display().to_string()
            ],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO agent_invocations (
                 id, run_id, process_id, started_at, ended_at, repo, worktree,
                 wave, flow, skill, project, task, provider, model, surface,
                 capture_status, incomplete_reason, outcome, artifact_dir,
                 conversation_path, provider_events_path, provider_session_id,
                 provider_session_path, conversation_event_count,
                 conversation_bytes, supervising_run_id, account_id,
                 resume_token, answer_ask_id
             ) VALUES (
                 'invocation-tui', 'trace-tui', 'exec-tui', 2, NULL, ?1, ?1,
                 'promotion-test', NULL, NULL, NULL, NULL, 'codex', NULL, 'tui',
                 'capturing', NULL, 'running', '', '', NULL, NULL, NULL, 0, 0,
                 ?2, NULL, NULL, NULL
             )",
            rusqlite::params![repo.display().to_string(), drained_run_id.as_str()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO blob_tokens (sha, lines, bytes, tokens)
             VALUES ('published', 1, 1, 1)",
            [],
        )
        .unwrap();
    drop(connection);
    let _lfd_session = TestTmuxSession::new(format!("lfd-{home_id}"), &account_home);

    let current = CandidateIdentity::current();
    let published_version = "0.12.7-test";
    let published = CandidateIdentity {
        source_revision: "published-p1".to_string(),
        source_identity: "published-p1".to_string(),
        authority: MigrationAuthority::Published,
        package_version: published_version.to_string(),
        build_version: Some(published_version.to_string()),
        latest_known_migration: current.latest_known_migration.clone(),
    };
    let preview = PromotionPreview {
        candidate: published,
        database_path: reliable.display().to_string(),
        compatibility: Compatibility::Exact {
            frontier: current.latest_known_migration,
        },
        executable_compatibility: ExecutableCompatibility::Compatible { references: 0 },
        active_runs: Vec::new(),
        verdict: Verdict::Promote,
    };
    let preview_path = directory.path().join("published-preview.json");
    fs::write(&preview_path, serde_json::to_vec(&preview).unwrap()).unwrap();
    let cli_target = public.join("lf");
    write_executable(
        &cli_target,
        format!(
            "#!/bin/sh\nif [ \"$1\" = install ] && [ \"$2\" = preflight ]; then /bin/cat \"{}\"; exit 0; fi\nexit 64\n",
            preview_path.display()
        ),
    );
    let daemon_target = public.join("lfd");
    write_executable(
        &daemon_target,
        format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then printf '%s\\n' 'lfd {published_version}'; exit 0; fi\nexit 64\n"
        ),
    );

    let promote = |candidate: &Path, daemon: &Path, fresh: bool| {
        let mut command = isolated_command(candidate, &account_home, &interposer);
        command
            .args(["install", "promote", "--from-build"])
            .arg(candidate)
            .arg("--cli-target")
            .arg(&cli_target)
            .arg("--daemon-source")
            .arg(daemon)
            .arg("--daemon-target")
            .arg(&daemon_target);
        if fresh {
            command.arg("--fresh");
        }
        command.output().unwrap()
    };

    let blocked_tui = promote(
        &binaries.development_cli,
        &binaries.development_daemon,
        false,
    );
    assert!(!blocked_tui.status.success());
    assert!(
        String::from_utf8_lossy(&blocked_tui.stderr).contains("defers before pause or signal"),
        "{}",
        String::from_utf8_lossy(&blocked_tui.stderr)
    );
    assert!(tui_session.is_live());
    let root = root_for_home(&account_home);
    assert!(matches!(
        read_state(&root).unwrap(),
        MachineInstallState::Legacy
    ));
    assert_eq!(
        rusqlite::Connection::open(&reliable)
            .unwrap()
            .query_row(
                "SELECT state FROM runs WHERE id=?1",
                [drained_run_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "active"
    );
    tui_session.kill();
    rusqlite::Connection::open(&reliable)
        .unwrap()
        .execute(
            "INSERT INTO run_events (
                 run_id, process_id, parent_process_id, seq, ts, repo, worktree,
                 wave, node, event, command, flow, skill, step_index, error
             ) VALUES (
                 'trace-tui', 'exec-tui', NULL, 1, 3, ?1, ?1,
                 'promotion-test', 'run', 'errored', NULL, NULL, NULL, NULL, NULL
             )",
            [repo.display().to_string()],
        )
        .unwrap();

    let first = promote(
        &binaries.development_cli,
        &binaries.development_daemon,
        false,
    );
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_active = match read_state(&root).unwrap() {
        MachineInstallState::Settled(active) => *active,
        state => panic!("expected settled development install, got {state:?}"),
    };
    assert_eq!(first_active.selection.source, InstallSource::Development);
    assert_ne!(first_active.selection.store, reliable);
    let first_fork = first_active
        .first_fork
        .as_ref()
        .expect("first local promotion records its fork boundary");
    assert_eq!(
        first_fork.enabled_work,
        vec![loopflow::durable::WorkRef::Wave(wave_id.clone())]
    );
    assert_eq!(first_fork.drained_run_ids, vec![drained_run_id.to_string()]);
    assert_eq!(
        fs::canonicalize(&cli_target).unwrap(),
        fs::canonicalize(entry_gate_path(&root, &ArtifactRole::Cli).unwrap()).unwrap()
    );
    assert_eq!(
        fs::canonicalize(&daemon_target).unwrap(),
        fs::canonicalize(entry_gate_path(&root, &ArtifactRole::Daemon).unwrap()).unwrap()
    );
    let settled_gate = isolated_command(&cli_target, &account_home, &interposer)
        .arg("--version")
        .output()
        .unwrap();
    assert!(settled_gate.status.success());

    let daemon_gate = entry_gate_path(&root, &ArtifactRole::Daemon).unwrap();
    let gate_before_refusal = fs::read(&daemon_gate).unwrap();
    let invalid_daemon = directory.path().join("invalid-lfd");
    write_executable(&invalid_daemon, "#!/bin/sh\nexit 64\n");
    let invalid = promote(&binaries.development_cli, &invalid_daemon, false);
    assert!(!invalid.status.success());
    assert_eq!(fs::read(&daemon_gate).unwrap(), gate_before_refusal);
    let daemon_after_refusal = isolated_command(&daemon_target, &account_home, &interposer)
        .arg("--version")
        .output()
        .unwrap();
    assert!(daemon_after_refusal.status.success());

    let mut fenced = switch_receipt(
        "switch-entry-gate-proof",
        first_active.selection.clone(),
        first_active.selection.clone(),
        first_active.published_fallback.clone(),
        None,
        first_active
            .first_fork
            .clone()
            .expect("first local promotion records its fork boundary"),
        first_active.work_dispositions.clone(),
    );
    fenced.coordinator = first_active
        .selection
        .artifact_set
        .artifact(&ArtifactRole::Cli)
        .unwrap()
        .clone();
    write_switch(&root, &fenced).unwrap();
    let blocked_gate = isolated_command(&cli_target, &account_home, &interposer)
        .arg("--version")
        .output()
        .unwrap();
    assert!(!blocked_gate.status.success());
    assert!(
        String::from_utf8_lossy(&blocked_gate.stderr).contains("ordinary startup is blocked"),
        "{}",
        String::from_utf8_lossy(&blocked_gate.stderr)
    );
    clear_switch(&root, &fenced.id).unwrap();

    let development_store = first_active.selection.store.clone();
    let development = rusqlite::Connection::open(&development_store).unwrap();
    assert_eq!(
        development
            .prepare("SELECT name FROM local_promotion_release_order ORDER BY position")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        vec!["beta".to_string(), "coda".to_string()]
    );
    let expected_draft_count = loopflow::build_info::migration_draft_manifest().len() as i64 + 2;
    assert_eq!(
        development
            .query_row("SELECT COUNT(*) FROM development_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        expected_draft_count
    );
    development
        .execute(
            "INSERT INTO blob_tokens (sha, lines, bytes, tokens)
             VALUES ('development', 2, 2, 2)",
            [],
        )
        .unwrap();
    drop(development);
    let receipts_before_reinstall = fs::read_dir(root.join("receipts")).unwrap().count();
    let generations_before_reinstall = rusqlite::Connection::open(&development_store)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM home_runtime_generations", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap();

    let second = promote(
        &binaries.development_cli,
        &binaries.development_daemon,
        false,
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(
        String::from_utf8_lossy(&second.stdout).contains("already installed"),
        "{}",
        String::from_utf8_lossy(&second.stdout)
    );
    let second_active = match read_state(&root).unwrap() {
        MachineInstallState::Settled(active) => *active,
        state => panic!("expected settled compatible update, got {state:?}"),
    };
    assert_eq!(second_active, first_active);
    assert_eq!(
        fs::read_dir(root.join("receipts")).unwrap().count(),
        receipts_before_reinstall
    );
    assert_eq!(
        rusqlite::Connection::open(&development_store)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM home_runtime_generations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        generations_before_reinstall
    );
    assert_eq!(second_active.selection.store, development_store);
    assert_eq!(
        rusqlite::Connection::open(&development_store)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM blob_tokens", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        2
    );
    assert_eq!(
        rusqlite::Connection::open(&reliable)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM blob_tokens", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        1
    );
    assert!(rusqlite::Connection::open(&reliable)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM local_promotion_release_order",
            [],
            |_| Ok(())
        )
        .is_err());

    let changed = promote(&binaries.changed_cli, &binaries.changed_daemon, false);
    assert!(!changed.status.success());
    assert!(
        String::from_utf8_lossy(&changed.stderr).contains("--fresh"),
        "{}",
        String::from_utf8_lossy(&changed.stderr)
    );
    let fresh = promote(&binaries.changed_cli, &binaries.changed_daemon, true);
    assert!(
        fresh.status.success(),
        "{}",
        String::from_utf8_lossy(&fresh.stderr)
    );
    let fresh_active = match read_state(&root).unwrap() {
        MachineInstallState::Settled(active) => *active,
        state => panic!("expected settled fresh development fork, got {state:?}"),
    };
    let fresh_store = fresh_active.selection.store.clone();
    assert_ne!(fresh_store, development_store);
    assert!(development_store.is_file());
    assert_eq!(
        fresh_active
            .work_dispositions
            .iter()
            .map(|disposition| (disposition.work.clone(), disposition.outcome))
            .collect::<Vec<_>>(),
        vec![(
            loopflow::durable::WorkRef::Wave(wave_id.clone()),
            WorkDisposition::Disabled
        )]
    );
    for (store, expected) in [
        (&reliable, true),
        (&development_store, true),
        (&fresh_store, false),
    ] {
        let enabled = rusqlite::Connection::open(store)
            .unwrap()
            .query_row(
                "SELECT enabled FROM work_placements WHERE wave_id=?1",
                [wave_id.as_str()],
                |row| row.get::<_, bool>(0),
            )
            .unwrap();
        assert_eq!(enabled, expected, "{}", store.display());
    }
    assert_eq!(
        rusqlite::Connection::open(&fresh_store)
            .unwrap()
            .prepare("SELECT name FROM local_promotion_release_order ORDER BY position")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        vec!["beta".to_string(), "coda-changed".to_string()]
    );

    let published_return = isolated_command(&binaries.published_cli, &account_home, &interposer)
        .args(["install", "promote"])
        .arg("--cli-target")
        .arg(&cli_target)
        .arg("--daemon-source")
        .arg(&binaries.published_daemon)
        .arg("--daemon-target")
        .arg(&daemon_target)
        .output()
        .unwrap();
    assert!(
        published_return.status.success(),
        "{}",
        String::from_utf8_lossy(&published_return.stderr)
    );
    let published_active = match read_state(&root).unwrap() {
        MachineInstallState::Settled(active) => *active,
        state => panic!("expected settled published return, got {state:?}"),
    };
    assert_eq!(published_active.selection.source, InstallSource::Published);
    assert_eq!(published_active.selection.store, reliable);
    assert!(published_active.first_fork.is_none());
    assert!(published_active.work_dispositions.is_empty());
    let published_receipts_before_reinstall = fs::read_dir(root.join("receipts")).unwrap().count();
    let published_reinstall = isolated_command(&binaries.published_cli, &account_home, &interposer)
        .args(["install", "promote"])
        .arg("--cli-target")
        .arg(&cli_target)
        .arg("--daemon-source")
        .arg(&binaries.published_daemon)
        .arg("--daemon-target")
        .arg(&daemon_target)
        .output()
        .unwrap();
    assert!(
        published_reinstall.status.success(),
        "{}",
        String::from_utf8_lossy(&published_reinstall.stderr)
    );
    assert!(
        String::from_utf8_lossy(&published_reinstall.stdout).contains("already installed"),
        "{}",
        String::from_utf8_lossy(&published_reinstall.stdout)
    );
    assert_eq!(
        *match read_state(&root).unwrap() {
            MachineInstallState::Settled(active) => active,
            state => panic!("expected settled published reinstall, got {state:?}"),
        },
        published_active
    );
    assert_eq!(
        fs::read_dir(root.join("receipts")).unwrap().count(),
        published_receipts_before_reinstall
    );
    assert_eq!(
        rusqlite::Connection::open(&reliable)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM blob_tokens", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        1
    );
    let reliable_after = rusqlite::Connection::open(&reliable).unwrap();
    assert_eq!(
        reliable_after
            .prepare("SELECT name FROM local_promotion_release_order ORDER BY position")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        vec!["beta".to_string(), "coda".to_string()]
    );
    assert!(reliable_after
        .query_row("SELECT COUNT(*) FROM development_migrations", [], |row| row
            .get::<_, i64>(0))
        .is_err());
    assert!(!reliable_after
        .query_row(
            "SELECT enabled FROM work_placements WHERE wave_id=?1",
            [wave_id.as_str()],
            |row| row.get::<_, bool>(0),
        )
        .unwrap());
    assert_eq!(
        rusqlite::Connection::open(&development_store)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM blob_tokens", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        2
    );
    assert_eq!(
        rusqlite::Connection::open(&fresh_store)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM blob_tokens", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        1
    );
    let receipts = fs::read_dir(root.join("receipts"))
        .unwrap()
        .map(|entry| {
            let bytes = fs::read(entry.unwrap().path()).unwrap();
            serde_json::from_slice::<SwitchReceipt>(&bytes).unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(receipts.len(), 3);
    let published_receipt = receipts
        .iter()
        .find(|receipt| {
            receipt.prior.source == InstallSource::Development
                && receipt.target.source == InstallSource::Published
        })
        .expect("published return archives its fork disposition evidence");
    assert_eq!(
        published_receipt
            .first_fork
            .as_ref()
            .unwrap()
            .drained_run_ids,
        vec![drained_run_id.to_string()]
    );
    assert_eq!(
        published_receipt.work_dispositions,
        vec![WorkDispositionReceipt {
            work: loopflow::durable::WorkRef::Wave(wave_id),
            outcome: WorkDisposition::Disabled,
        }]
    );
    let published_gate = isolated_command(&cli_target, &account_home, &interposer)
        .arg("--version")
        .output()
        .unwrap();
    assert!(published_gate.status.success());
    let published_source = isolated_command(&binaries.published_cli, &account_home, &interposer)
        .arg("--version")
        .output()
        .unwrap();
    assert!(published_source.status.success());
    assert_eq!(published_gate.stdout, published_source.stdout);
}

#[test]
fn switch_recovery_obeys_every_persisted_phase_owner() {
    let directory = tempfile::tempdir().unwrap();
    let interposer = account_home_interposer(directory.path());
    let source_cli = Path::new(env!("CARGO_BIN_EXE_lf"));
    let source_daemon = Path::new(env!("CARGO_BIN_EXE_lfd"));
    let candidate = CandidateIdentity::current();

    for (phase, target_store_advanced, active_selection_committed) in [
        (SwitchPhase::Planned, false, false),
        (SwitchPhase::Quiesced, false, false),
        (SwitchPhase::TargetPrepared, false, false),
        (SwitchPhase::Advancing, false, false),
        (SwitchPhase::Activated, true, false),
        (SwitchPhase::Reconciling, true, false),
        (SwitchPhase::Settled, true, true),
    ] {
        let name = format!("{phase:?}").to_lowercase();
        let account_home = directory.path().join(format!("account-{name}"));
        let binaries = directory.path().join(format!("binaries-{name}"));
        let repo = directory.path().join(format!("repo-{name}"));
        fs::create_dir_all(&account_home).unwrap();
        fs::create_dir_all(&binaries).unwrap();
        fs::create_dir_all(&repo).unwrap();
        let account_home = account_home.canonicalize().unwrap();
        let coordinator_cli = binaries.join("coordinator-lf");
        let coordinator_daemon = binaries.join("coordinator-lfd");
        let candidate_cli = binaries.join("candidate-lf");
        let candidate_daemon = binaries.join("candidate-lfd");
        copy_executable(source_cli, &coordinator_cli);
        copy_executable(source_daemon, &coordinator_daemon);
        copy_executable(source_cli, &candidate_cli);
        copy_executable(source_daemon, &candidate_daemon);

        let store = account_home.join(".lf-dev/installed/recovery/loopflow.db");
        fs::create_dir_all(store.parent().unwrap()).unwrap();
        let initialized = isolated_command(source_cli, &account_home, &interposer)
            .env("LF_DB_PATH", &store)
            .args(["doctor", "--json"])
            .output()
            .unwrap();
        assert!(
            initialized.status.success(),
            "{}",
            String::from_utf8_lossy(&initialized.stderr)
        );
        if target_store_advanced {
            _apply_embedded_drafts(&store);
        }
        let connection = rusqlite::Connection::open(&store).unwrap();
        let home_id: String = connection
            .query_row("SELECT id FROM homes WHERE route='local'", [], |row| {
                row.get(0)
            })
            .unwrap();
        connection
            .execute(
                "INSERT INTO waves (id, name, repo, created_at) VALUES (?1, ?2, ?3, 1)",
                rusqlite::params![
                    loopflow::id::WaveId::new().as_str(),
                    format!("recovery-{name}"),
                    repo.display().to_string()
                ],
            )
            .unwrap();
        drop(connection);
        let _lfd_session = TestTmuxSession::new(format!("lfd-{home_id}"), &account_home);

        let prior_set = candidate_artifact_set(
            &format!("prior-{name}"),
            InstallSource::Development,
            &candidate,
            &coordinator_cli,
            &coordinator_daemon,
        );
        let target_set = candidate_artifact_set(
            &format!("target-{name}"),
            InstallSource::Development,
            &candidate,
            &candidate_cli,
            &candidate_daemon,
        );
        let fallback_set = candidate_artifact_set(
            &format!("fallback-{name}"),
            InstallSource::Published,
            &candidate,
            &coordinator_cli,
            &coordinator_daemon,
        );
        let prior = selection(
            &format!("prior-{name}"),
            InstallSource::Development,
            prior_set,
            &store,
        );
        let target = selection(
            &format!("target-{name}"),
            InstallSource::Development,
            target_set,
            &store,
        );
        let root = root_for_home(&account_home);
        write_active(
            &root,
            &ActiveInstall {
                schema_version: 1,
                selection: prior.clone(),
                published_fallback: fallback_set.clone(),
                retained_published_sets: vec![fallback_set.clone()],
                first_fork: Some(ForkEvidence {
                    enabled_work: Vec::new(),
                    drained_run_ids: Vec::new(),
                }),
                work_dispositions: Vec::new(),
            },
        )
        .unwrap();
        let mut receipt = switch_receipt(
            &format!("switch-{name}"),
            prior.clone(),
            target.clone(),
            fallback_set,
            None,
            ForkEvidence {
                enabled_work: Vec::new(),
                drained_run_ids: Vec::new(),
            },
            Vec::new(),
        );
        receipt.phase = phase;
        receipt.target_store_advance_started = matches!(
            phase,
            SwitchPhase::Advancing
                | SwitchPhase::Activated
                | SwitchPhase::Reconciling
                | SwitchPhase::Settled
        );
        receipt.target_store_advanced = target_store_advanced;
        receipt.active_selection_committed = active_selection_committed;
        if receipt.target_store_advance_started {
            receipt.recovery_owner = RecoveryOwner::Candidate;
        }

        let cli_gate = loopflow::machine_install::install_entry_gate(
            &root,
            &ArtifactRole::Cli,
            &candidate_cli,
        )
        .unwrap();
        let daemon_gate = loopflow::machine_install::install_entry_gate(
            &root,
            &ArtifactRole::Daemon,
            &candidate_daemon,
        )
        .unwrap();
        std::os::unix::fs::symlink(&cli_gate, &receipt.activation.cli).unwrap();
        std::os::unix::fs::symlink(&daemon_gate, &receipt.activation.daemon).unwrap();
        write_switch(&root, &receipt).unwrap();

        let blocked = isolated_command(&receipt.activation.cli, &account_home, &interposer)
            .arg("--version")
            .output()
            .unwrap();
        assert!(!blocked.status.success(), "phase {phase:?}");
        assert!(
            String::from_utf8_lossy(&blocked.stderr).contains("ordinary startup is blocked"),
            "phase {phase:?}: {}",
            String::from_utf8_lossy(&blocked.stderr)
        );

        if !receipt.target_store_advance_started {
            let wrong_owner = isolated_command(&candidate_cli, &account_home, &interposer)
                .args(["install", "recover-switch", "--switch", &receipt.id])
                .output()
                .unwrap();
            assert!(!wrong_owner.status.success(), "phase {phase:?}");
            assert!(
                String::from_utf8_lossy(&wrong_owner.stderr)
                    .contains(&receipt.coordinator.path.display().to_string()),
                "phase {phase:?}: {}",
                String::from_utf8_lossy(&wrong_owner.stderr)
            );
        } else {
            let wrong_owner = isolated_command(&coordinator_cli, &account_home, &interposer)
                .args(["install", "recover-switch", "--switch", &receipt.id])
                .output()
                .unwrap();
            assert!(!wrong_owner.status.success(), "phase {phase:?}");
            assert!(
                String::from_utf8_lossy(&wrong_owner.stderr)
                    .contains(&receipt.candidate.path.display().to_string()),
                "phase {phase:?}: {}",
                String::from_utf8_lossy(&wrong_owner.stderr)
            );
        }
        let recovery_owner = if receipt.target_store_advance_started {
            &candidate_cli
        } else {
            &coordinator_cli
        };
        let recovered = isolated_command(recovery_owner, &account_home, &interposer)
            .args(["install", "recover-switch", "--switch", &receipt.id])
            .output()
            .unwrap();
        assert!(
            recovered.status.success(),
            "phase {phase:?}: {}",
            String::from_utf8_lossy(&recovered.stderr)
        );
        let settled = match read_state(&root).unwrap() {
            MachineInstallState::Settled(active) => *active,
            state => panic!("phase {phase:?} did not settle: {state:?}"),
        };
        let expected = if receipt.target_store_advance_started {
            &target
        } else {
            &prior
        };
        assert_eq!(&settled.selection, expected, "phase {phase:?}");
        let repaired_gate = isolated_command(&receipt.activation.cli, &account_home, &interposer)
            .arg("--version")
            .output()
            .unwrap();
        assert!(
            repaired_gate.status.success(),
            "phase {phase:?}: {}",
            String::from_utf8_lossy(&repaired_gate.stderr)
        );
    }
}

#[test]
fn installed_development_artifact_uses_its_pinned_store_but_source_build_does_not() {
    let directory = tempfile::tempdir().unwrap();
    let account_home = directory.path().join("account");
    let installed_dir = directory.path().join("installed");
    fs::create_dir_all(&account_home).unwrap();
    fs::create_dir_all(&installed_dir).unwrap();

    let source_binary = Path::new(env!("CARGO_BIN_EXE_lf"));
    let installed_cli = installed_dir.join("lf");
    let installed_daemon = installed_dir.join("lfd");
    let fallback_cli = installed_dir.join("published-lf");
    let fallback_daemon = installed_dir.join("published-lfd");
    for target in [
        &installed_cli,
        &installed_daemon,
        &fallback_cli,
        &fallback_daemon,
    ] {
        copy_executable(source_binary, target);
    }

    let development_store = directory.path().join("dev/loopflow.db");
    SqliteStore::new(&development_store).unwrap();
    let reliable_store = account_home.join(".lf/loopflow.db");
    fs::create_dir_all(reliable_store.parent().unwrap()).unwrap();
    fs::write(&reliable_store, b"reliable-store-sentinel").unwrap();

    let development = artifact_set(
        "development",
        InstallSource::Development,
        &installed_cli,
        &installed_daemon,
    );
    let published = artifact_set(
        "published",
        InstallSource::Published,
        &fallback_cli,
        &fallback_daemon,
    );
    write_active(
        &root_for_home(&account_home),
        &ActiveInstall {
            schema_version: 1,
            selection: InstallSelection {
                installation_id: "local-test".to_string(),
                source: InstallSource::Development,
                artifact_set: development,
                store: development_store.canonicalize().unwrap(),
            },
            published_fallback: published.clone(),
            retained_published_sets: vec![published],
            first_fork: None,
            work_dispositions: Vec::new(),
        },
    )
    .unwrap();

    let root = root_for_home(&account_home);
    let installed = authorize(&root, &installed_cli, &ArtifactRole::Cli)
        .unwrap()
        .unwrap();
    assert_eq!(installed.store, development_store.canonicalize().unwrap());
    assert!(authorize(&root, source_binary, &ArtifactRole::Cli)
        .unwrap()
        .is_none());
    assert_eq!(
        fs::read(&reliable_store).unwrap(),
        b"reliable-store-sentinel"
    );
}

#[test]
fn inherited_home_cannot_move_the_development_production_store_guard() {
    let directory = tempfile::tempdir().unwrap();
    let fake_home = directory.path().join("fake-account-home");
    let fake_lf_home = directory.path().join("source-home");
    fs::create_dir_all(&fake_home).unwrap();
    fs::create_dir_all(&fake_lf_home).unwrap();
    let production_store = account_home().unwrap().join(".lf/loopflow.db");

    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["home", "id"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("HOME", &fake_home)
        .env("LF_HOME", &fake_lf_home)
        .env("LF_DB_PATH", &production_store)
        .env("NO_COLOR", "1")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_RUN_CONTEXT")
        .env_remove("LF_RUN_LEASE")
        .env_remove("LF_AGENT_INVOCATION_ID")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("refuses production database"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn published_settlement_model_does_not_import_the_development_store() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("machine-install");
    let binaries = directory.path().join("binaries");
    fs::create_dir_all(&binaries).unwrap();
    let source_binary = Path::new(env!("CARGO_BIN_EXE_lf"));
    let mut sets = Vec::new();
    for (id, source) in [
        ("p1", InstallSource::Published),
        ("d1", InstallSource::Development),
        ("p2", InstallSource::Published),
    ] {
        let cli = binaries.join(format!("{id}-lf"));
        let daemon = binaries.join(format!("{id}-lfd"));
        copy_executable(source_binary, &cli);
        copy_executable(source_binary, &daemon);
        sets.push(artifact_set(id, source, &cli, &daemon));
    }
    let p1_set = sets.remove(0);
    let d1_set = sets.remove(0);
    let p2_set = sets.remove(0);

    let reliable = directory.path().join("reliable.db");
    let connection = rusqlite::Connection::open(&reliable).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE records (id INTEGER PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE schema_migrations (name TEXT PRIMARY KEY);
             INSERT INTO records VALUES (1, 'published');",
        )
        .unwrap();
    drop(connection);
    let development = directory.path().join("development.db");
    fs::copy(&reliable, &development).unwrap();

    let p1 = selection(
        "published-p1",
        InstallSource::Published,
        p1_set.clone(),
        &reliable,
    );
    write_active(
        &root,
        &ActiveInstall {
            schema_version: 1,
            selection: p1.clone(),
            published_fallback: p1_set.clone(),
            retained_published_sets: vec![p1_set.clone()],
            first_fork: None,
            work_dispositions: Vec::new(),
        },
    )
    .unwrap();

    let work = loopflow::durable::WorkRef::Wave(loopflow::id::WaveId::new());
    let evidence = ForkEvidence {
        enabled_work: vec![work.clone()],
        drained_run_ids: vec![loopflow::durable::RunId::new().to_string()],
    };
    let pending = vec![WorkDispositionReceipt {
        work: work.clone(),
        outcome: WorkDisposition::Pending,
    }];
    let d1 = selection(
        "development-d1",
        InstallSource::Development,
        d1_set,
        &development,
    );
    let mut enter_development = switch_receipt(
        "switch-p1-d1",
        p1,
        d1.clone(),
        p1_set.clone(),
        None,
        evidence.clone(),
        pending.clone(),
    );
    write_switch(&root, &enter_development).unwrap();

    let connection = rusqlite::Connection::open(&development).unwrap();
    connection
        .execute_batch(
            "ALTER TABLE records ADD COLUMN local_feature TEXT;
             CREATE TABLE development_migrations (name TEXT PRIMARY KEY);
             INSERT INTO development_migrations VALUES ('A');
             UPDATE records SET local_feature='development' WHERE id=1;
             INSERT INTO records VALUES (2, 'dev-only', 'development');",
        )
        .unwrap();
    drop(connection);
    enter_development.phase = SwitchPhase::Settled;
    enter_development.recovery_owner = RecoveryOwner::Candidate;
    enter_development.target_store_advance_started = true;
    enter_development.target_store_advanced = true;
    enter_development.active_selection_committed = true;
    write_switch(&root, &enter_development).unwrap();
    settle_switch(
        &root,
        &enter_development,
        &ActiveInstall {
            schema_version: 1,
            selection: d1.clone(),
            published_fallback: p1_set.clone(),
            retained_published_sets: vec![p1_set.clone()],
            first_fork: Some(evidence.clone()),
            work_dispositions: pending.clone(),
        },
    )
    .unwrap();

    let connection = rusqlite::Connection::open(&reliable).unwrap();
    let transaction = connection.unchecked_transaction().unwrap();
    transaction
        .execute_batch(
            "ALTER TABLE records ADD COLUMN local_feature TEXT;
             INSERT INTO schema_migrations VALUES ('A');",
        )
        .unwrap();
    transaction.commit().unwrap();
    drop(connection);

    let p2 = selection(
        "published-p2",
        InstallSource::Published,
        p2_set.clone(),
        &reliable,
    );
    let mut return_published = switch_receipt(
        "switch-d1-p2",
        d1,
        p2.clone(),
        p1_set.clone(),
        Some(p2_set.clone()),
        evidence.clone(),
        pending,
    );
    write_switch(&root, &return_published).unwrap();
    return_published.phase = SwitchPhase::Settled;
    return_published.recovery_owner = RecoveryOwner::Candidate;
    return_published.target_store_advance_started = true;
    return_published.target_store_advanced = true;
    return_published.active_selection_committed = true;
    return_published.published_fallback = p2_set.clone();
    return_published.work_dispositions = vec![WorkDispositionReceipt {
        work,
        outcome: WorkDisposition::Disabled,
    }];
    write_switch(&root, &return_published).unwrap();
    settle_switch(
        &root,
        &return_published,
        &ActiveInstall {
            schema_version: 1,
            selection: p2.clone(),
            published_fallback: p2_set.clone(),
            retained_published_sets: vec![p1_set, p2_set],
            first_fork: None,
            work_dispositions: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        read_state(&root).unwrap(),
        MachineInstallState::Settled(active)
            if active.selection == p2
                && active.selection.store == reliable.canonicalize().unwrap()
                && active.first_fork.is_none()
                && active.work_dispositions.is_empty()
    ));
    let reliable = rusqlite::Connection::open(&reliable).unwrap();
    assert_eq!(
        reliable
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE name='A'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
    assert_eq!(
        reliable
            .query_row("SELECT COUNT(*) FROM records", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(reliable
        .query_row(
            "SELECT COUNT(*) FROM development_migrations",
            [],
            |_| Ok(())
        )
        .is_err());
    let development = rusqlite::Connection::open(&development).unwrap();
    assert_eq!(
        development
            .query_row("SELECT COUNT(*) FROM records", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        development
            .query_row("SELECT COUNT(*) FROM development_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        1
    );
}
