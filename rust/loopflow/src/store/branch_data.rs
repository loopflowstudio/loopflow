//! Select private branch data before a source executable starts any runtime.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};

use super::{canonicalize_with_missing_tail, same_database_file};

pub(super) fn owned_stores() -> io::Result<Vec<PathBuf>> {
    let mut stores = crate::machine_install::owned_stores(
        &crate::machine_install::root().map_err(io::Error::other)?,
    )
    .map_err(io::Error::other)?;
    stores.push(super::production_database_path());
    Ok(stores)
}

pub(super) fn is_owned_store(path: &Path, stores: &[PathBuf]) -> io::Result<bool> {
    for store in stores {
        if same_database_file(path, store)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn inherited_store(
    home: &Path,
    database: &Path,
    stores: &[PathBuf],
) -> io::Result<Option<PathBuf>> {
    let home = canonicalize_with_missing_tail(home)?;
    for store in stores {
        if same_database_file(database, store)?
            || store
                .parent()
                .map(canonicalize_with_missing_tail)
                .transpose()?
                .as_ref()
                == Some(&home)
        {
            return Ok(Some(store.clone()));
        }
    }
    Ok(None)
}

/// Called before threads, journaling, or Home-scoped resources are initialized.
pub fn isolate_branch_data() -> io::Result<()> {
    if crate::build_info::provenance().is_release()
        || crate::machine_install::selection_for_current_executable()
            .map_err(io::Error::other)?
            .is_some()
    {
        return Ok(());
    }
    let (destination, database, source) = private_data_paths()?;
    if let Some(source) = &source {
        seed_store(source, &database)?;
    }
    let context_changed = source.is_some()
        || !canonicalize_with_missing_tail(&super::authority_home_dir())
            .is_ok_and(|home| home == destination)
        || !super::observability_database_path()
            .and_then(|path| same_database_file(&path, &database))
            .unwrap_or(false)
        || std::env::var_os("LF_RUN_DIR").is_some_and(|dir| {
            !canonicalize_with_missing_tail(&PathBuf::from(dir))
                .is_ok_and(|dir| dir.starts_with(destination.join("runs")))
        });
    if context_changed {
        clear_inherited_execution();
    }
    for name in ["LF_HOME", super::CONTROL_HOME_ENV] {
        std::env::set_var(name, &destination);
    }
    for name in ["LF_DB_PATH", super::CONTROL_DB_PATH_ENV] {
        std::env::set_var(name, &database);
    }
    if let Some(source) = source {
        eprintln!(
            "Branch lf is using data directory {}; installed store {} is unchanged.",
            destination.display(),
            source.display()
        );
    }
    Ok(())
}

/// Select the same private destination without creating, copying or migrating it.
pub(crate) fn observation_database_path() -> io::Result<PathBuf> {
    let selection =
        crate::machine_install::selection_for_current_executable().map_err(io::Error::other)?;
    let path = if crate::build_info::provenance().is_release() || selection.is_some() {
        super::database_path_from_env()?
    } else {
        private_data_paths()?.1
    };
    if is_owned_store(&path, &owned_stores()?)?
        && !selection
            .as_ref()
            .is_some_and(|selection| same_database_file(&path, &selection.store).unwrap_or(false))
    {
        return Err(io::Error::other(
            "process ledger belongs to another installation",
        ));
    }
    Ok(path)
}

fn private_data_paths() -> io::Result<(PathBuf, PathBuf, Option<PathBuf>)> {
    let ordinary_home = std::env::var_os("LF_HOME").filter(|value| !value.is_empty());
    let home = ordinary_home
        .clone()
        .or_else(|| std::env::var_os(super::CONTROL_HOME_ENV).filter(|value| !value.is_empty()))
        .map(PathBuf::from)
        .unwrap_or_else(super::default_lf_home_dir);
    let home = canonicalize_with_missing_tail(&home)?;
    let database = std::env::var_os("LF_DB_PATH")
        .filter(|value| !value.is_empty())
        .or_else(|| {
            ordinary_home
                .is_none()
                .then(|| {
                    std::env::var_os(super::CONTROL_DB_PATH_ENV).filter(|value| !value.is_empty())
                })
                .flatten()
        })
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                home.join(path)
            }
        })
        .unwrap_or_else(|| home.join("loopflow.db"));
    let stores = owned_stores()?;
    let source = inherited_store(&home, &database, &stores)?;
    let (destination, database) = if source.is_some() {
        let destination = super::default_lf_home_dir_for(
            &super::machine_home_dir(),
            crate::build_info::BuildProvenance::Development,
            &crate::build_info::source_identity(),
        );
        let database = destination.join("loopflow.db");
        if is_owned_store(&database, &stores)? {
            return Err(io::Error::other(format!(
                "branch data directory {} aliases an installed store; remove that alias before using this source build",
                destination.display()
            )));
        }
        (destination, database)
    } else {
        (home, database)
    };
    Ok((destination, database, source))
}

fn clear_inherited_execution() {
    // The snapshot does not inherit the launching Session's execution authority
    // or permission to append to its Run bundle. Clear executable pins too:
    // discovery selects this CLI or the daemon's sibling CLI, never lfd itself.
    for name in [
        "LF_BIN",
        "LF_RUN_ID",
        "LF_PARENT_RUN_ID",
        "LF_RUN_DIR",
        "LF_RUN_CONTEXT",
        "LF_WORK_ADVANCE_CLAIM",
        "LF_PROCESS_ID",
        "LF_TRACE_ID",
        "LF_WAVE_ID",
        "LF_FLOW_STEP",
        "LF_HUMAN_SESSION",
        "LF_HUMAN_SESSION_RUN_BIND",
        "LF_ACCOUNT_LEASE",
        "LF_AGENT_CALLER",
        super::CONTROL_BIN_ENV,
    ] {
        std::env::remove_var(name);
    }
}

fn seed_store(source: &Path, destination: &Path) -> io::Result<()> {
    if destination.exists() || !source.exists() {
        return Ok(());
    }
    let parent = destination
        .parent()
        .expect("branch database has a parent directory");
    fs::create_dir_all(parent)?;
    let temporary = tempfile::NamedTempFile::new_in(parent)?;
    {
        let source = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(io::Error::other)?;
        let mut target = Connection::open(temporary.path()).map_err(io::Error::other)?;
        // Preserve recorded Home identities and placements. A data copy is not
        // registration of another execution destination or process authority.
        let backup = Backup::new(&source, &mut target).map_err(io::Error::other)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if backup.step(4096).map_err(io::Error::other)? == StepResult::Done {
                break;
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "branch data snapshot exceeded 10 seconds; retry",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(backup);
        // Copy history, never a socket or a live driver's write capability.
        // Installed schemas may predate these columns; the snapshot is not a migration.
        for table in ["agent_sessions", "sessions"] {
            let columns = target
                .prepare(&format!("PRAGMA table_info({table})"))
                .map_err(io::Error::other)?
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(io::Error::other)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(io::Error::other)?;
            let assignments = [
                ("driver_exec_id", "driver_exec_id=NULL"),
                ("driver_generation", "driver_generation=driver_generation+1"),
                ("provider_endpoint", "provider_endpoint=NULL"),
                ("provider_pid", "provider_pid=NULL"),
                ("provider_started_at", "provider_started_at=NULL"),
            ]
            .into_iter()
            .filter(|(column, _)| columns.iter().any(|name| name == column))
            .map(|(_, assignment)| assignment)
            .collect::<Vec<_>>();
            if !assignments.is_empty() {
                target
                    .execute(&format!("UPDATE {table} SET {}", assignments.join(",")), [])
                    .map_err(io::Error::other)?;
            }
        }
    }
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(destination) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.error),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process::Command;

    use rusqlite::Connection;
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    use super::{inherited_store, is_owned_store, isolate_branch_data, seed_store};
    use crate::engine::process::{current_home_execution_context, pinned_execution_context};
    use crate::id::WaveId;
    use crate::run_record::{CaptureHandle, RunFlowMembership, RunSpec};
    use crate::store::migrations::{
        apply_installed_development_sqlite, development_store_diagnostic,
    };
    use crate::store::sqlite::SqliteStore;
    use crate::trace::PreparedTurnContext;
    use crate::work::wave::Wave;

    #[test]
    fn explicit_private_data_unifies_storage_observation_runs_and_children() {
        const CHILD: &str = "LF_BRANCH_HOME_TEST";
        if let Some(root) = std::env::var_os(CHILD).map(PathBuf::from) {
            let home = root.join("private");
            let expected_db = std::env::var_os("LF_BRANCH_TEST_DB")
                .map(PathBuf::from)
                .unwrap();
            isolate_branch_data().unwrap();
            assert_eq!(crate::store::lf_home_dir(), home);
            assert_eq!(crate::store::authority_home_dir(), home);
            assert_eq!(crate::store::database_path_from_env().unwrap(), expected_db);
            assert_eq!(
                crate::store::observability_database_path().unwrap(),
                expected_db
            );
            assert!(std::env::var_os("LF_RUN_ID").is_none());
            assert!(std::env::var_os("LF_RUN_DIR").is_none());
            assert!(std::env::var_os("LF_WORK_ADVANCE_CLAIM").is_none());
            assert!(std::env::var_os("LF_AGENT_CALLER").is_none());

            let store = SqliteStore::new(&expected_db).unwrap();
            let wave = Wave::new(WaveId::new(), "private-proof".into(), "/repo".into());
            store.create_wave(&wave).unwrap();
            let observer = SqliteStore::open_run_ledger_read_only(
                &crate::store::observability_database_path().unwrap(),
            )
            .unwrap();
            assert!(observer
                .work_identities()
                .unwrap()
                .iter()
                .any(|row| row.subject == "private-proof"));

            let capture = CaptureHandle::begin_with_context(
                RunSpec {
                    harness: "proof".into(),
                    model: None,
                    surface: "headless".into(),
                    cwd: root.clone(),
                    repo: None,
                    worktree: None,
                    skill: None,
                    subjects: Vec::new(),
                    flow: RunFlowMembership::Independent,
                    work: None,
                },
                &PreparedTurnContext::from_prompts("", "private data proof"),
                None,
            )
            .unwrap();
            capture.record_raw("stdout", "branch output");
            capture.finish("completed").unwrap();
            assert!(capture.artifact_dir().starts_with(home.join("runs")));
            assert!(capture.artifact_dir().join("manifest.json").exists());

            for context in [
                pinned_execution_context().unwrap(),
                current_home_execution_context().unwrap(),
            ] {
                assert_eq!(context.lf_home, home);
                assert_eq!(context.db_path, expected_db);
                assert_eq!(context.lf_bin, std::env::current_exe().unwrap());
            }
            // Re-entering the same private data directory keeps its own Run and lease.
            std::env::set_var("LF_RUN_ID", capture.run_id().as_str());
            std::env::set_var("LF_RUN_DIR", capture.artifact_dir());
            std::env::set_var("LF_ACCOUNT_LEASE", "private-lease");
            isolate_branch_data().unwrap();
            assert_eq!(
                std::env::var("LF_RUN_ID").unwrap(),
                capture.run_id().as_str()
            );
            assert_eq!(std::env::var("LF_ACCOUNT_LEASE").unwrap(), "private-lease");
            return;
        }

        for (database, invalid_control) in [
            (None, false),
            (Some("custom.db"), false),
            (Some("absolute"), false),
            (None, true),
        ] {
            let root = tempdir().unwrap();
            let root = root.path().canonicalize().unwrap();
            let installed = root.join("installed");
            std::fs::create_dir(&installed).unwrap();
            std::fs::write(installed.join("loopflow.db"), b"installed bytes").unwrap();
            let expected_db = root.join("private").join(match database {
                None => "loopflow.db",
                Some("absolute") => "absolute.db",
                Some(value) => value,
            });
            let mut child = Command::new(std::env::current_exe().unwrap());
            child.args(["--exact", "store::branch_data::tests::explicit_private_data_unifies_storage_observation_runs_and_children", "--nocapture"])
                .env(CHILD, &root)
                .env("LF_BRANCH_TEST_DB", &expected_db)
                .env("LF_HOME", root.join("private"))
                .env_remove("LF_DB_PATH")
                .env("LF_CONTROL_HOME", &installed)
                .env("LF_CONTROL_DB_PATH", installed.join("loopflow.db"))
                .env("LF_BIN", installed.join("lf"))
                .env("LF_CONTROL_BIN", installed.join("lf"))
                .env("CARGO_BIN_EXE_lf", std::env::current_exe().unwrap())
                .env("LF_AGENT_CALLER", "inherited conversation caller")
                .env("LF_RUN_ID", "run_inherited")
                .env("LF_RUN_DIR", installed.join("runs/parent"))
                .env_remove("LF_TASK_ORIGIN")
                .env("LF_WORK_ADVANCE_CLAIM", "inherited claim");
            if invalid_control {
                child
                    .env("LF_CONTROL_HOME", root.join("private"))
                    .env("LF_CONTROL_DB_PATH", "../stale.db");
            }
            if let Some(database) = database {
                child.env(
                    "LF_DB_PATH",
                    if database == "absolute" {
                        expected_db.as_os_str()
                    } else {
                        std::ffi::OsStr::new(database)
                    },
                );
            }
            let output = child.output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                std::fs::read(installed.join("loopflow.db")).unwrap(),
                b"installed bytes"
            );
            assert_eq!(std::fs::read_dir(&installed).unwrap().count(), 1);
        }
    }

    #[test]
    fn copied_conversation_keeps_history_without_live_connection_or_driver_authority() {
        let root = tempdir().unwrap();
        let source_path = root.path().join("source.db");
        let target_path = root.path().join("private/copy.db");
        let source = SqliteStore::open_ephemeral(&source_path).unwrap();
        let exec = crate::id::ExecId::new();
        source.test_session("conversation", "run_00000000000000000000000000000001");
        {
            let conn = Connection::open(&source_path).unwrap();
            conn.execute(
                "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,'trace',1)",
                [exec.as_str()],
            )
            .unwrap();
        }
        let driver = source
            .claim_session_driver("conversation", None, &exec, true)
            .unwrap();
        source
            .record_session_connection("conversation", &driver, "/private/original.sock", "thread")
            .unwrap();
        source
            .record_session_provider_process("conversation", &driver, 12345, 12)
            .unwrap();
        source
            .record_session_turn_origin(
                "conversation",
                "thread",
                "turn",
                driver.provider_generation,
                &exec,
            )
            .unwrap();
        source
            .record_session_event(
                "conversation",
                "thread",
                "turn",
                crate::session::SessionEventKind::Completed,
                &serde_json::json!({"status":"completed"}),
            )
            .unwrap();
        let history = source.session_history("conversation", 0, 0).unwrap();
        seed_store(&source_path, &target_path).unwrap();
        let target = SqliteStore::open_read_only(&target_path).unwrap();
        assert_eq!(
            target.session_history("conversation", 0, 0).unwrap(),
            history
        );
        assert!(target.session_connection("conversation").unwrap().is_none());
        assert!(target
            .session_provider_process("conversation")
            .unwrap()
            .is_none());
        assert_eq!(
            target.session_thread("conversation").unwrap().as_deref(),
            Some("thread")
        );
        assert_eq!(
            source.session_provider_process("conversation").unwrap(),
            Some((12345, 12))
        );
        let detached = target.session_driver("conversation").unwrap().unwrap();
        assert!(detached.exec_id.is_none());
        assert_ne!(detached.generation, driver.generation);
        assert_eq!(detached.provider_generation, driver.provider_generation);
        assert_eq!(detached.provider_exec_id, exec);
        let writable = SqliteStore::open_ephemeral(&target_path).unwrap();
        assert!(writable
            .with_session_driver::<()>("conversation", &driver, || panic!(
                "copied driver gained authority"
            ))
            .is_err());
        assert_eq!(source.session_driver("conversation").unwrap(), Some(driver));
        assert_eq!(
            source.session_connection("conversation").unwrap(),
            Some(("/private/original.sock".into(), "thread".into()))
        );
        assert_eq!(
            source.session_history("conversation", 0, 0).unwrap(),
            history
        );
    }

    #[test]
    fn incompatible_seed_preserves_source_receipts_and_private_writes() {
        let root = tempdir().unwrap();
        let source_path = root.path().join("installed.db");
        let private_path = root.path().join("branch/loopflow.db");
        let source = Connection::open(&source_path).unwrap();
        source.execute_batch("PRAGMA journal_mode=WAL").unwrap();
        let sql =
            "CREATE TABLE future_notes (note TEXT); INSERT INTO future_notes VALUES ('source');";
        let checksum = hex::encode(Sha256::digest(sql.as_bytes()));
        let mut drafts = crate::build_info::migration_draft_manifest().to_vec();
        drafts.push(crate::build_info::MigrationDraft {
            id: "33333333333333333333333333333333",
            name: "future_notes",
            dependencies: &[],
            sql,
            checksum: Box::leak(checksum.clone().into_boxed_str()),
        });
        apply_installed_development_sqlite(&source, &drafts).unwrap();
        let source_bytes = || {
            [
                source_path.clone(),
                PathBuf::from(format!("{}-wal", source_path.display())),
            ]
            .map(|path| std::fs::read(path).unwrap())
        };
        let original = source_bytes();
        seed_store(&source_path, &private_path).unwrap();
        let private = Connection::open(&private_path).unwrap();
        let note = |connection: &Connection| {
            connection
                .query_row("SELECT note FROM future_notes", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap()
        };
        assert_eq!(
            note(&private),
            "source",
            "snapshot includes committed WAL data"
        );
        let local_home = |connection: &Connection| -> String {
            connection
                .query_row("SELECT id FROM homes WHERE route='local'", [], |row| {
                    row.get(0)
                })
                .unwrap()
        };
        assert_eq!(local_home(&private), local_home(&source));
        private
            .execute_batch(
                "UPDATE future_notes SET note='private'; CREATE TABLE branch_only (value TEXT);",
            )
            .unwrap();
        let bytes = || {
            [source_path.clone(), private_path.clone()].map(|path| {
                [
                    path.clone(),
                    PathBuf::from(format!("{}-wal", path.display())),
                ]
                .map(|path| std::fs::read(path).ok())
            })
        };
        let before = bytes();
        let error = apply_installed_development_sqlite(
            &private,
            crate::build_info::migration_draft_manifest(),
        )
        .unwrap_err();
        let message = development_store_diagnostic(&private, error).to_string();
        assert!(
            message.contains(private_path.to_str().unwrap()),
            "{message}"
        );
        assert!(message.contains("future_notes"), "{message}");
        assert!(message.contains(&checksum), "{message}");
        assert!(!message.contains("--fresh"));
        seed_store(&source_path, &private_path).unwrap();
        assert_eq!(bytes(), before);
        assert_eq!(
            source_bytes(),
            original,
            "private data and schema stayed private"
        );
        assert_eq!(note(&source), "source");
        assert_eq!(note(&private), "private");
    }

    #[test]
    fn owned_data_directory_redirects_even_with_a_different_database_name() {
        let root = tempdir().unwrap();
        let installed = root.path().join("installed/custom.db");
        let stores = vec![installed.clone()];
        assert_eq!(
            inherited_store(
                installed.parent().unwrap(),
                &root.path().join("elsewhere.db"),
                &stores
            )
            .unwrap(),
            Some(installed)
        );
        assert_eq!(
            inherited_store(root.path(), &root.path().join("fixture.db"), &stores).unwrap(),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn owned_store_aliases_redirect_to_the_original_seed() {
        let root = tempdir().unwrap();
        let installed = root.path().join("installed.db");
        std::fs::write(&installed, b"store").unwrap();
        let stores = vec![installed.clone()];
        for (name, hardlink) in [("hardlink", true), ("symlink", false)] {
            let alias = root.path().join(name);
            if hardlink {
                std::fs::hard_link(&installed, &alias).unwrap();
            } else {
                std::os::unix::fs::symlink(&installed, &alias).unwrap();
            }
            assert!(is_owned_store(&alias, &stores).unwrap());
            assert_eq!(
                inherited_store(&PathBuf::from("/unused-home"), &alias, &stores).unwrap(),
                Some(installed.clone())
            );
        }
    }
}
