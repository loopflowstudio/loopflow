use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;

use crate::lf::commands::install::Compatibility;

/// The pin is a preference, not permission to open a schema it cannot read.
pub(super) fn select_cli(
    database: &Path,
    current: &Path,
    selected: Option<&Path>,
    installed: Option<&Path>,
) -> Result<PathBuf> {
    let Some(selected) = selected else {
        return installed.map(Path::to_path_buf).context(
            "no installed lf; install a published release or select an explicit disposable LF_HOME for this build",
        );
    };
    let preferred = std::fs::canonicalize(selected)
        .with_context(|| format!("resolve selected lf {}", selected.display()))
        .and_then(|preferred| {
            if installed != Some(preferred.as_path()) {
                check_compatibility(database, current, &preferred)?;
            }
            Ok(preferred)
        });
    let error = match preferred {
        Ok(preferred) => return Ok(preferred),
        Err(error) => error,
    };
    let Some(installed) = installed else {
        return Err(error.context(
            "no compatible installed lf; install a published release, then reconnect the existing Session",
        ));
    };
    check_compatibility(database, current, installed).with_context(|| {
        format!(
            "selected lf {} is unavailable ({error:#}); installed lf {} cannot read the same Machine; run `lf doctor` with a compatible release",
            selected.display(), installed.display()
        )
    })?;
    Ok(installed.to_path_buf())
}

fn check_compatibility(database: &Path, current: &Path, candidate: &Path) -> Result<()> {
    if candidate == current {
        if !database.exists() {
            return Ok(());
        }
        let connection = Connection::open_with_flags(
            database,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        crate::store::migrations::validate_sqlite_schema(&connection)?;
        if let Some(pending) = crate::store::migrations::pending_shared_migration(&connection)? {
            return Err(anyhow!("Machine is missing {pending}; run `lf install`"));
        }
        return Ok(());
    }

    match read_compatibility(database, candidate)? {
        Compatibility::Exact { .. } => Ok(()),
        other => Err(anyhow!(
            "lf {} cannot use this Machine: {other:?}",
            candidate.display()
        )),
    }
}

pub(super) fn read_compatibility(database: &Path, candidate: &Path) -> Result<Compatibility> {
    // This protocol already exists in released binaries. Do not let an old
    // candidate resolve caller identity or inherit a promotion capability.
    let mut command = Command::new(candidate);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("LF_") {
            command.env_remove(name);
        }
    }
    command.args(["self", "install", "preflight", "--json"]);
    let output = crate::os_process::bounded_output(&mut command, Duration::from_secs(5))
        .with_context(|| format!("inspect selected lf {}", candidate.display()))?;
    #[derive(Deserialize)]
    struct SchemaEvidence {
        database_path: PathBuf,
        compatibility: Compatibility,
    }
    let evidence: SchemaEvidence = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("lf {} returned no schema evidence", candidate.display()))?;
    if !crate::store::same_database_file(database, &evidence.database_path)? {
        return Err(anyhow!(
            "lf {} inspected a different Machine",
            candidate.display()
        ));
    }
    Ok(evidence.compatibility)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    use rusqlite::Connection;
    use serde_json::json;

    use super::select_cli;

    fn database(directory: &Path) -> PathBuf {
        let path = directory.join("loopflow.db");
        crate::store::migrations::apply_sqlite(&Connection::open(&path).unwrap()).unwrap();
        fs::canonicalize(path).unwrap()
    }

    fn candidate(directory: &Path, database: &Path, compatibility: serde_json::Value) -> PathBuf {
        let path = directory.join("selected-lf");
        let evidence = json!({"database_path": database, "compatibility": compatibility});
        fs::write(
            &path,
            format!(
                "#!/bin/sh\nprintf '%s\\n' {}\n",
                crate::os_process::shell_escape(&evidence.to_string())
            ),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        fs::canonicalize(path).unwrap()
    }

    #[test]
    fn stale_or_removed_pin_recovers_without_changing_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let database = database(dir.path());
        let current = std::env::current_exe().unwrap();
        let old = candidate(
            dir.path(),
            &database,
            json!({"kind":"incompatible", "reason":"unknown newer migration"}),
        );
        let before = fs::read(&database).unwrap();
        for pin in [old, dir.path().join("removed")] {
            assert_eq!(
                select_cli(&database, &current, Some(&pin), Some(&current)).unwrap(),
                current
            );
            assert_eq!(fs::read(&database).unwrap(), before);
        }
    }

    #[test]
    fn compatible_development_selection_survives_an_install() {
        let dir = tempfile::tempdir().unwrap();
        let database = database(dir.path());
        let current = std::env::current_exe().unwrap();
        let development = candidate(
            dir.path(),
            &database,
            json!({"kind":"exact", "frontier":"fixture"}),
        );
        assert_eq!(
            select_cli(&database, &current, Some(&development), Some(&current)).unwrap(),
            development
        );
        assert_eq!(
            select_cli(&database, &current, Some(&current), Some(&development)).unwrap(),
            current
        );
    }

    #[test]
    fn no_compatible_binary_reports_recovery_without_modifying_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let database = database(dir.path());
        Connection::open(&database).unwrap().execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES ('99.0.0.001_release', 0)", [],
        ).unwrap();
        let current = std::env::current_exe().unwrap();
        let old = candidate(
            dir.path(),
            &database,
            json!({"kind":"incompatible", "reason":"unknown newer migration"}),
        );
        let before = fs::read(&database).unwrap();
        let error = select_cli(&database, &current, Some(&old), Some(&current)).unwrap_err();
        assert!(format!("{error:#}").contains("run `lf doctor`"));
        let error = select_cli(&database, &current, Some(&old), None).unwrap_err();
        assert!(format!("{error:#}").contains("reconnect the existing Session"));
        assert_eq!(fs::read(&database).unwrap(), before);
    }

    #[test]
    fn evidence_for_another_machine_cannot_select_a_binary() {
        let dir = tempfile::tempdir().unwrap();
        let database = database(dir.path());
        let current = std::env::current_exe().unwrap();
        let foreign = candidate(
            dir.path(),
            &dir.path().join("different.db"),
            json!({"kind":"exact", "frontier":"fixture"}),
        );
        assert_eq!(
            select_cli(&database, &current, Some(&foreign), Some(&current)).unwrap(),
            current
        );
    }

    #[test]
    #[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
    fn installed_entry_keeps_session_history_and_pending_steers() {
        use crate::installation::{
            self, ActiveInstall, ArtifactIdentity, ArtifactRole, ArtifactSet, InstallSelection,
            InstallSource,
        };
        use crate::store::sqlite::SqliteStore;
        use sha2::{Digest, Sha256};
        use std::process::{Command, Stdio};

        assert!(Path::new("/.dockerenv").is_file());
        let account = installation::account_home().unwrap();
        assert_eq!(account, Path::new("/home/lf-task-proof"));
        let root = installation::root().unwrap();
        let home = account.join(".lf");
        assert!(!root.exists());
        assert!(!home.exists());
        fs::create_dir(&home).unwrap();
        let database = database(&home);
        let store = SqliteStore::new(&database).unwrap();
        let session = store.test_session(
            "upgrade-session",
            &crate::session_record::new_artifact_key(),
        );
        store.retain_session_observation(&session, &crate::session::SessionObservation {
            artifact_key: session.artifact_key.clone(),
            source: "provider-session:fixture".into(),
            observed_at: 1,
            task_id: None,
            wave_id: None,
            payload: json!({"input_id":session.artifact_key, "source":"provider-session:fixture", "evidence":{
                "schema_version":1,"provider_session_id":"retained-native","account_id":null
            }}),
        }).unwrap();
        let steer = store
            .save_session_steer(&session.id, "Fixture", "Keep this pending")
            .unwrap();
        let history = store.session_history(&session.id, 0, 100).unwrap();
        let files = tempfile::tempdir().unwrap();
        let old = candidate(
            files.path(),
            &database,
            json!({"kind":"incompatible", "reason":"unknown newer migration"}),
        );
        let binary = PathBuf::from(loopflow_test_support::lf_binary());
        let artifact = ArtifactIdentity::capture(ArtifactRole::Cli, &binary).unwrap();
        let set = ArtifactSet {
            id: "upgrade-proof".into(),
            source: InstallSource::Published,
            source_revision: "fixture".into(),
            source_identity: "fixture".into(),
            content_sha256: hex::encode(Sha256::digest(artifact.sha256.as_bytes())),
            artifacts: vec![artifact],
        };
        installation::write_active(
            &root,
            &ActiveInstall {
                schema_version: 1,
                selection: InstallSelection {
                    installation_id: "upgrade-proof".into(),
                    source: InstallSource::Published,
                    artifact_set: set.clone(),
                    store: database.clone(),
                },
                published_fallback: set.clone(),
                retained_published_sets: vec![set],
            },
        )
        .unwrap();
        let gate = installation::install_entry_gate(&root, &ArtifactRole::Cli, &binary).unwrap();
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(".lf/flows/upgrade-proof.yaml", "- cmd: sync --plan\n");
        let delivery = crate::context_block::ContextDelivery {
            repository: None,
            repo: repo.path().to_path_buf(),
            home: home.clone(),
            wave: None,
            skill_file: None,
            references: Vec::new(),
        };
        let delivery_path = files.path().join("delivery.json");
        fs::write(&delivery_path, serde_json::to_vec(&delivery).unwrap()).unwrap();
        {
            let _environment = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
            std::env::set_var("LF_HOME", &home);
            std::env::set_var("LF_BIN", &binary);
            let hooks = delivery.hook_settings().unwrap();
            assert!(hooks["Stop"][0]["hooks"][0]["command"]
                .as_str()
                .unwrap()
                .contains(gate.to_str().unwrap()));
        }
        let caller = json!({"session_id":session.id,"agent_process_id":null,"origin_exec_id":"e3904e02-0219-45fd-8c4e-c7f289c60552"}).to_string();
        let run = |entry: &Path, pin: &Path, args: &[&str]| {
            // Enter through the source CLI too: selecting the gate must reach
            // its artifact without repeatedly returning through that gate.
            let mut command = Command::new(entry);
            command
                .env_clear()
                .env("HOME", &account)
                .env("LF_HOME", &home)
                .env("LF_BIN", pin)
                .env("LF_AGENT_CALLER", &caller)
                .env("PATH", "/usr/bin:/bin")
                .current_dir(repo.path())
                .stdin(Stdio::null())
                .args(args);
            let output =
                crate::os_process::bounded_output(&mut command, std::time::Duration::from_secs(30))
                    .unwrap();
            assert!(
                output.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            output
        };
        for (entry, pin) in [(&gate, &old), (&binary, &gate)] {
            let output = run(entry, pin, &["session", "list", "--all", "--json"]);
            assert!(String::from_utf8_lossy(&output.stdout).contains(&session.id));
            run(
                entry,
                pin,
                &[
                    "__planning-check",
                    "--delivery",
                    delivery_path.to_str().unwrap(),
                ],
            );
        }
        run(&gate, &old, &["flow", "upgrade-proof", "--batch"]);
        assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
        assert_eq!(store.session_history(&session.id, 0, 100).unwrap(), history);
        assert_eq!(
            store
                .input_provider_session(&session.artifact_key)
                .unwrap()
                .unwrap()
                .agent_session,
            "retained-native".into()
        );
        let pending = store.session_steers(&session.id, 0, 100).unwrap();
        assert_eq!(pending.pending, 1);
        assert_eq!(pending.items[0].steer.id, steer.id);
        let connection = Connection::open(&database).unwrap();
        let nested: i64 = connection.query_row("SELECT count(*) FROM flow_process_steps s JOIN processes p ON p.id=s.lf_process_id WHERE p.exit_code=0", [], |row| row.get(0)).unwrap();
        assert_eq!(nested, 1);
        let attributed: i64 = connection
            .query_row(
                "SELECT count(*) FROM processes WHERE caller_session_id=?1 AND via_agent=1",
                [&session.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(attributed >= 2);
        drop(connection);
        drop(store);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(home).unwrap();
    }
}
