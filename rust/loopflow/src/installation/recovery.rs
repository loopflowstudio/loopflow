//! Installation recovery owns a replacement native terminal, never a Flow cursor.
//! Only an explicit incompatible-schema verdict authorizes replacing a client.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

use crate::harness::agent_process::{close_session_agent_process, terminate_session_agent};
use crate::id::LfProcessId;
use crate::lf::commands::install::Compatibility;
use crate::process::SessionAttachment;
use crate::session::LfSession;
use crate::store::sqlite::SqliteStore;
use crate::store::StoreError;

use super::{ArtifactRole, InstallSelection};

/// Start isolated owners after promotion releases its lock. Each owner repeats
/// the evidence check and exact attachment comparison before touching a provider.
pub(crate) fn schedule(selection: &InstallSelection) -> Result<()> {
    let database = &selection.store;
    let binary = &selection
        .artifact_set
        .artifact(&ArtifactRole::Cli)
        .context("Installed artifact set has no CLI")?
        .path;
    let store = SqliteStore::new(database)?;
    for agent in store.agent_processes()? {
        let Some(id) = agent.process.agent_session_id.as_deref() else {
            continue;
        };
        let result = (|| -> Result<()> {
            let Some((session, _)) = broken_native(&store, database, id, &agent.process.id)? else {
                return Ok(());
            };
            let argv = vec![
                binary.to_string_lossy().into_owned(),
                "session".into(),
                "recover".into(),
                session.id.clone(),
                "--agent-process".into(),
                agent.process.id.to_string(),
            ];
            let owner = crate::transport::start(&argv, true, &session.cwd, &|command| {
                // The installer is not the recovered conversation's caller or
                // capture. Resume reconstructs those from the retained Session.
                for (key, _) in std::env::vars_os() {
                    if key.to_string_lossy().starts_with("LF_") {
                        command.env_remove(key);
                    }
                }
                command.env("LF_BIN", binary);
            })?;
            eprintln!(
                "Session {} upgrade recovery started in transport {owner}",
                session.id
            );
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("Session {id} upgrade recovery unavailable: {error:#}");
        }
    }
    Ok(())
}

/// The replacement invocation runs inside its own terminal, including for
/// Sessions whose old clients predate retained execution transports.
pub(crate) fn resume(session: &str, agent: &LfProcessId) -> Result<()> {
    crate::data_clone::require_writable_session(session)?;
    let owner =
        crate::transport::current().context("Upgrade recovery requires a terminal owner")?;
    let database = crate::store::database_path_from_env()?;
    let store = SqliteStore::new(&database)?;
    let launch = crate::ops::human_session::lock_session_process(session)?;
    let Some((session, attachment)) = broken_native(&store, &database, session, agent)? else {
        return Ok(());
    };
    let provider = store
        .input_provider_session(&session.artifact_key)?
        .context("Native history disappeared before upgrade recovery")?;
    crate::lf::commands::util::require_session_task(&store, &session)?;
    stop_native(&store, &session.id, &attachment)?;
    // Present only the winning owner, while the launch lock still excludes a
    // competing recovery. The viewer has no provider or attachment authority.
    if let Err(error) = present(&session, owner) {
        eprintln!(
            "Session {} recovery terminal could not be presented: {error:#}; use `lf session attach {}`",
            session.id, session.id
        );
    }
    // No input, take-over, Flow launch or steer retry. Resume claims only after
    // observed exit and binds this terminal with the new attachment's token.
    crate::lf::commands::util::resume_session_with_env(
        &session.artifact_key,
        Some(&provider),
        &BTreeMap::new(),
        Some(launch),
    )
}

fn present(session: &LfSession, owner: &LfProcessId) -> Result<()> {
    let config = crate::config::load_global_config()?.unwrap_or_default();
    let terminal = config
        .session
        .terminal
        .as_deref()
        .unwrap_or(if cfg!(target_os = "macos") {
            "Terminal"
        } else {
            "x-terminal-emulator"
        });
    let binary = std::env::current_exe()?;
    let home = crate::store::lf_home_dir();
    let argv = vec![
        binary.to_string_lossy().into_owned(),
        "transport".into(),
        "attach".into(),
        owner.to_string(),
    ];
    let shell = crate::os_process::lf_session_shell_command(
        &session.cwd,
        &argv,
        &[
            ("LF_BIN", &binary.to_string_lossy()),
            ("LF_HOME", &home.to_string_lossy()),
        ],
    );
    let mut command = if cfg!(target_os = "macos") && !Path::new(terminal).is_absolute() {
        let mut command = Command::new("/usr/bin/open");
        if terminal == "Terminal" {
            let script = crate::transport::directory(&home)?.join(format!("{owner}.command"));
            std::fs::write(&script, format!("#!/bin/sh\nrm -- \"$0\"\n{shell}\n"))?;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700))?;
            command.args(["-a", terminal]).arg(script);
        } else {
            command.args(["-na", terminal, "--args", "-e", "/bin/sh", "-c", &shell]);
        }
        command
    } else {
        let mut command = Command::new(terminal);
        command.args(["-e", "/bin/sh", "-c", &shell]);
        command
    };
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LF_") {
            command.env_remove(key);
        }
    }
    let mut child = command
        .current_dir(&session.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .with_context(|| format!("open terminal {terminal}"))?;
    std::thread::spawn(move || {
        if let Ok(status) = child.wait() {
            if !status.success() {
                eprintln!("Recovery terminal presentation exited with {status}; do not restart the provider");
            }
        }
    });
    Ok(())
}

/// Replace a broken native attachment without retiring its conversation. The
/// caller has established incompatibility; this operation owns only exact exit.
fn stop_native(store: &SqliteStore, session: &str, expected: &SessionAttachment) -> Result<()> {
    store.finish_session_attachment(session, expected, "upgrade", || {
        let agents = store.agent_processes()?;
        let agent = agents
            .iter()
            .find(|agent| agent.process.id == expected.agent_process_id)
            .ok_or_else(|| {
                StoreError::InvalidAuthority("Upgrade attachment is no longer running".into())
            })?;
        if !store
            .session(session)?
            .is_some_and(|session| session.interactive && session.flow_lf_process_id.is_none())
        {
            return Err(StoreError::InvalidAuthority(
                "Structured execution needs its own continuation boundary".into(),
            ));
        }
        if agent.interactive {
            if agents.iter().any(|other| {
                other.process.id != agent.process.id
                    && other.process.pid == agent.process.pid
                    && other.process.os_started_at == agent.process.os_started_at
            }) {
                return Err(StoreError::InvalidAuthority(
                    "AgentProcess OS identity has multiple owners".into(),
                ));
            }
            terminate_session_agent(agent)
                .map_err(|error| StoreError::InvalidAuthority(error.to_string()))?;
            Ok(true)
        } else if close_session_agent_process(store, session)? {
            // A native Codex client owns a separate app-server group. Its
            // existing close path checks that no other conversation shares it.
            Ok(true)
        } else {
            Err(StoreError::InvalidAuthority(
                "AgentProcess exit is unobserved".into(),
            ))
        }
    })?;
    Ok(())
}

fn broken_native(
    store: &SqliteStore,
    database: &Path,
    id: &str,
    agent: &LfProcessId,
) -> Result<Option<(LfSession, SessionAttachment)>> {
    let Some(session) = store.session(id)? else {
        return Ok(None);
    };
    // A native history resume cannot continue a headless turn or a Flow's
    // process-local cursor. Those effects require their original owner.
    if !session.interactive
        || session.flow_lf_process_id.is_some()
        || session.completed_at.is_some()
    {
        return Ok(None);
    }
    let Some(attachment) = store.session_attachment(id)? else {
        return Ok(None);
    };
    if &attachment.agent_process_id != agent {
        return Ok(None);
    }
    let Some(process) = store.process(agent)? else {
        return Ok(None);
    };
    if process.completed_at.is_some() {
        return Ok(None);
    }
    let owner = store
        .process(&attachment.provider_lf_process_id)?
        .context("Provider launch Process is unavailable")?;
    let argv: Vec<String> = serde_json::from_str(
        owner
            .command
            .as_deref()
            .context("Provider launch executable is unknown")?,
    )?;
    let binary = Path::new(
        argv.first()
            .context("Provider launch executable is unknown")?,
    );
    anyhow::ensure!(
        binary.is_absolute(),
        "Provider launch executable is not an absolute path"
    );
    // A mutable development path may now contain different bytes from those
    // running. Retained content-addressed bytes identify the launch itself.
    let artifact = super::ArtifactIdentity::capture(super::ArtifactRole::Cli, binary)?;
    anyhow::ensure!(
        artifact.path.file_name().and_then(|name| name.to_str())
            == Some(format!("lf-{}", artifact.sha256).as_str()),
        "Provider launch bytes are not a retained immutable executable"
    );
    // Errors, missing binaries, timeouts and AheadPending are not proof of a
    // broken client. In particular, a compatible development pin stays put.
    if !matches!(
        super::dispatch::read_compatibility(database, binary)?,
        Compatibility::Incompatible { .. }
    ) {
        return Ok(None);
    }
    anyhow::ensure!(session.cwd.is_dir(), "Session checkout is unavailable");
    anyhow::ensure!(
        store
            .input_provider_session(&session.artifact_key)?
            .is_some(),
        "Native conversation identity is unavailable"
    );
    Ok(Some((session, attachment)))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use rusqlite::{params, Connection};
    use serde_json::json;

    use super::{broken_native, stop_native};
    use crate::harness::agent_process::spawn_native;
    use crate::id::LfProcessId;
    use crate::process::SessionAttachment;
    use crate::session::SessionObservation;
    use crate::steer::SteerContext;
    use crate::store::{sqlite::SqliteStore, StoreError};

    struct Fixture {
        _home: tempfile::TempDir,
        store: SqliteStore,
        database: PathBuf,
        binary: PathBuf,
        attachment: SessionAttachment,
        child: Child,
    }

    impl Fixture {
        fn new() -> Self {
            let home = tempfile::tempdir().unwrap();
            let database = home.path().join("loopflow.db");
            let store = SqliteStore::open_ephemeral(&database).unwrap();
            let session = store.test_session("native", &crate::session_record::new_artifact_key());
            let binary = home.path().join("old-lf");
            let parent = LfProcessId::new();
            let sql = Connection::open(&database).unwrap();
            sql.execute(
                "UPDATE agent_sessions SET provider='claude',cwd=?1 WHERE id='native'",
                [home.path().to_str().unwrap()],
            )
            .unwrap();
            sql.execute(
                "INSERT INTO processes(id,trace_id,started_at,command) VALUES(?1,?1,1,?2)",
                params![parent, json!([binary]).to_string()],
            )
            .unwrap();
            let attachment = store
                .claim_session_attachment("native", None, &parent, true)
                .unwrap();
            store.retain_session_observation(&session, &SessionObservation {
                artifact_key: session.artifact_key.clone(),
                source: "provider-session:fixture".into(),
                observed_at: 1,
                task_id: None,
                wave_id: None,
                payload: json!({"input_id":session.artifact_key,"source":"provider-session:fixture","evidence":{
                    "schema_version":1,"provider_session_id":"saved-native","account_id":null
                }}),
            }).unwrap();
            let mut command = Command::new("/bin/sleep");
            command.env_clear().arg("60");
            let child = spawn_native(
                command,
                &(store.clone(), "native".into(), attachment.clone()),
            )
            .unwrap();
            let mut fixture = Self {
                _home: home,
                store,
                database,
                binary,
                attachment,
                child,
            };
            fixture.verdict(json!({"kind":"incompatible","reason":"unknown newer migration"}));
            fixture
        }

        fn verdict(&mut self, compatibility: serde_json::Value) {
            let evidence = json!({"database_path":self.database,"compatibility":compatibility});
            fs::write(
                &self.binary,
                format!(
                    "#!/bin/sh\nprintf '%s\\n' {}\n",
                    crate::os_process::shell_escape(&evidence.to_string())
                ),
            )
            .unwrap();
            fs::set_permissions(&self.binary, fs::Permissions::from_mode(0o755)).unwrap();
            let artifact = crate::installation::ArtifactIdentity::capture(
                crate::installation::ArtifactRole::Cli,
                &self.binary,
            )
            .unwrap();
            let next = self._home.path().join(format!("lf-{}", artifact.sha256));
            fs::rename(&self.binary, &next).unwrap();
            self.binary = next;
            Connection::open(&self.database)
                .unwrap()
                .execute(
                    "UPDATE processes SET command=?2 WHERE id=?1",
                    params![
                        self.attachment.provider_lf_process_id,
                        json!([self.binary]).to_string()
                    ],
                )
                .unwrap();
        }

        fn eligible(&self) -> anyhow::Result<bool> {
            Ok(broken_native(
                &self.store,
                &self.database,
                "native",
                &self.attachment.agent_process_id,
            )?
            .is_some())
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    #[test]
    fn compatible_or_unknown_clients_keep_their_provider_and_receipts() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let mut fixture = Fixture::new();
        let before = fixture.store.session("native").unwrap();
        for verdict in [
            json!({"kind":"exact","frontier":"fixture"}),
            json!({"kind":"unreadable","reason":"busy"}),
            json!({"kind":"ahead_pending","applied_frontier":"old","latest_known":"new"}),
        ] {
            fixture.verdict(verdict);
            assert!(!fixture.eligible().unwrap());
            assert!(fixture.child.try_wait().unwrap().is_none());
            assert_eq!(fixture.store.session("native").unwrap(), before);
            assert_eq!(
                fixture.store.session_attachment("native").unwrap(),
                Some(fixture.attachment.clone())
            );
        }
        fs::remove_file(&fixture.binary).unwrap();
        assert!(fixture.eligible().is_err());
        assert!(fixture.child.try_wait().unwrap().is_none());
    }

    #[test]
    fn recovery_observes_exit_preserves_session_and_fences_the_old_owner() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let mut fixture = Fixture::new();
        assert!(fixture.eligible().unwrap());
        let store = &fixture.store;
        let before = store.session("native").unwrap().unwrap();
        let history = store.session_history("native", 0, 100).unwrap();
        let adapter = store
            .reserve_steer_adapter("native", &fixture.attachment)
            .unwrap();
        store
            .with_provider_context(&adapter, "saved-native", true, || Ok(()))
            .unwrap();
        store
            .save_session_steer("native", "Fixture", "Uncertain direction")
            .unwrap();
        assert!(store
            .emit_session_steers(
                &adapter,
                "saved-native",
                "tool:one",
                |_, _, _| Ok(SteerContext {
                    text: "direction".into(),
                    excerpts: vec![]
                }),
                |_| Err(StoreError::InvalidData("broken pipe".into()))
            )
            .is_err());
        store
            .save_session_steer("native", "Fixture", "Pending direction")
            .unwrap();
        let receipts = store.session_steers("native", 0, 100).unwrap();
        stop_native(store, "native", &fixture.attachment).unwrap();
        fixture.child.wait().unwrap();
        assert_eq!(store.session("native").unwrap(), Some(before.clone()));
        assert!(store
            .session_history("native", 0, 100)
            .unwrap()
            .starts_with(&history));
        assert!(store
            .process(&fixture.attachment.agent_process_id)
            .unwrap()
            .unwrap()
            .completed_at
            .is_some());
        let next = crate::session_record::resume_session_agent_process(
            store,
            "native",
            &LfProcessId::new(),
        )
        .unwrap();
        assert_ne!(next.agent_process_id, fixture.attachment.agent_process_id);
        let mut command = Command::new("/bin/sleep");
        command.env_clear().arg("60");
        fixture.child =
            spawn_native(command, &(store.clone(), "native".into(), next.clone())).unwrap();
        // A stale installer and the former client's eventual close cannot
        // signal the replacement or retire an ordinary, unbound Session.
        assert!(stop_native(store, "native", &fixture.attachment).is_err());
        assert!(store
            .finish_session_attachment("native", &fixture.attachment, "completed", || panic!(
                "stale close"
            ))
            .is_err());
        assert!(fixture.child.try_wait().unwrap().is_none());
        assert_eq!(store.session("native").unwrap(), Some(before.clone()));
        assert_eq!(
            store
                .input_provider_session(&before.artifact_key)
                .unwrap()
                .unwrap()
                .agent_session
                .as_str(),
            "saved-native"
        );
        let after = store.session_steers("native", 0, 100).unwrap();
        assert_eq!(
            serde_json::to_value(&after.items).unwrap(),
            serde_json::to_value(receipts.items).unwrap()
        );
        assert_eq!(after.pending, 1);
        assert!(after.items.iter().any(|row| row.evidence == "uncertain"));
    }

    #[test]
    fn unknown_process_identity_and_changed_attachment_never_authorize_a_signal() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let mut fixture = Fixture::new();
        let sql = Connection::open(&fixture.database).unwrap();
        sql.execute(
            "UPDATE processes SET os_started_at=NULL WHERE id=?1",
            [&fixture.attachment.agent_process_id],
        )
        .unwrap();
        let before = fixture.store.session_attachment("native").unwrap();
        assert!(stop_native(&fixture.store, "native", &fixture.attachment).is_err());
        assert_eq!(fixture.store.session_attachment("native").unwrap(), before);
        assert!(fixture.child.try_wait().unwrap().is_none());
        let next = fixture
            .store
            .claim_session_attachment(
                "native",
                Some(&fixture.attachment),
                &LfProcessId::new(),
                false,
            )
            .unwrap();
        assert!(stop_native(&fixture.store, "native", &fixture.attachment).is_err());
        assert_eq!(
            fixture.store.session_attachment("native").unwrap(),
            Some(next)
        );
        assert!(fixture.child.try_wait().unwrap().is_none());
    }

    #[test]
    fn flow_and_structured_execution_are_not_restarted_as_native_conversations() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let mut fixture = Fixture::new();
        Connection::open(&fixture.database)
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=0 WHERE id='native'",
                [],
            )
            .unwrap();
        assert!(!fixture.eligible().unwrap());
        assert!(fixture.child.try_wait().unwrap().is_none());
        let sql = Connection::open(&fixture.database).unwrap();
        let flow = LfProcessId::new();
        let completed = LfProcessId::new();
        sql.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
            [&flow],
        )
        .unwrap();
        sql.execute("INSERT INTO processes(id,trace_id,started_at,completed_at,exit_code,outcome) VALUES(?1,?1,1,2,0,'succeeded')", [&completed]).unwrap();
        sql.execute(
            "INSERT INTO flow_processes(lf_process_id,flow,graph) VALUES(?1,'effects','{}')",
            [&flow],
        )
        .unwrap();
        for (node, step) in [&completed, &fixture.attachment.provider_lf_process_id]
            .into_iter()
            .enumerate()
        {
            sql.execute(
                "UPDATE processes SET parent_lf_process_id=?2 WHERE id=?1",
                params![step, flow],
            )
            .unwrap();
            sql.execute("INSERT INTO flow_process_steps(flow_lf_process_id,lf_process_id,node,iterations) VALUES(?1,?2,?3,'[]')", params![flow, step, node as u32]).unwrap();
        }
        sql.execute(
            "UPDATE agent_sessions SET interactive=1 WHERE id='native'",
            [],
        )
        .unwrap();
        sql.execute("INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,observed_at,payload) VALUES('native','captured',?1,?2,3,'{}')",
            params![crate::session_record::new_artifact_key(), fixture.attachment.provider_lf_process_id]).unwrap();
        sql.execute(
            "UPDATE agent_sessions SET current_capture=?1 WHERE id='native'",
            [sql.last_insert_rowid()],
        )
        .unwrap();
        let before = fixture.store.process(&completed).unwrap();
        assert_eq!(
            fixture
                .store
                .session("native")
                .unwrap()
                .unwrap()
                .flow_lf_process_id
                .as_deref(),
            Some(flow.as_str())
        );
        assert!(!fixture.eligible().unwrap());
        assert_eq!(fixture.store.process(&completed).unwrap(), before);
        assert!(fixture.child.try_wait().unwrap().is_none());
    }

    #[test]
    fn recovery_presents_saved_native_history_once_without_input() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let binary = loopflow_test_support::lf_binary();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let mut fixture = Fixture::new();
        let home = fixture._home.path();
        let provider = home.join("claude");
        let received = home.join("received");
        let viewed = home.join("viewed");
        let presentations = home.join("presentations");
        let terminal = home.join("fixture-terminal");
        fs::write(
            &terminal,
            format!(
                "#!/bin/sh\nprintf 'opened\\n' >> {}\nshift\nexec \"$@\" > {}\n",
                crate::os_process::shell_escape(presentations.to_str().unwrap()),
                crate::os_process::shell_escape(viewed.to_str().unwrap()),
            ),
        )
        .unwrap();
        fs::set_permissions(&terminal, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(
            home.join("config.yaml"),
            format!("session:\n  terminal: {}\n", terminal.display()),
        )
        .unwrap();
        fs::write(
            &provider,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > {0}.tmp\nmv {0}.tmp {0}\nprintf 'Recovered saved-native\\n'\nexec /bin/sleep 60\n",
                crate::os_process::shell_escape(received.to_str().unwrap()),
            ),
        )
        .unwrap();
        fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
        let before = fixture.store.session("native").unwrap().unwrap();
        let history = fixture.store.session_history("native", 0, 100).unwrap();
        let mut command = Command::new(binary);
        command
            .env_clear()
            .env("HOME", home)
            .env("LF_HOME", home)
            .env("LF_BIN", binary)
            .env("PATH", format!("{}:/usr/bin:/bin", home.display()))
            .current_dir(home)
            .stdin(Stdio::null())
            .args([
                "transport",
                "start",
                "--terminal",
                "--",
                binary,
                "session",
                "recover",
                "native",
                "--agent-process",
                fixture.attachment.agent_process_id.as_str(),
            ]);
        let output =
            crate::os_process::bounded_output(&mut command, Duration::from_secs(30)).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let owner = LfProcessId::parse(String::from_utf8_lossy(&output.stdout).trim()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while !fs::read_to_string(&viewed)
            .is_ok_and(|output| output.contains("Recovered saved-native"))
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(25));
        }
        let next = fixture.store.session_attachment("native").unwrap().unwrap();
        // A concurrent or repeated installer still names the original attachment.
        // Its acknowledged owner must neither reopen the viewer nor replay input.
        let repeated =
            crate::os_process::bounded_output(&mut command, Duration::from_secs(30)).unwrap();
        let repeated_owner =
            LfProcessId::parse(String::from_utf8_lossy(&repeated.stdout).trim()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while fixture
            .store
            .process(&repeated_owner)
            .unwrap()
            .is_none_or(|process| process.completed_at.is_none())
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(25));
        }
        // Clean up before assertions, including a failing launch. Only the
        // replacement belongs to this fixture; never retry its input.
        if next.agent_process_id != fixture.attachment.agent_process_id {
            stop_native(&fixture.store, "native", &next).unwrap();
        }
        assert!(repeated.status.success());
        assert!(fixture
            .store
            .process(&repeated_owner)
            .unwrap()
            .unwrap()
            .completed_at
            .is_some());
        let args = fs::read_to_string(&received).unwrap_or_else(|error| {
            panic!(
                "{error}: {}",
                fs::read_to_string(home.join("logs/transport.log")).unwrap_or_default()
            )
        });
        assert!(args.contains("--resume\nsaved-native\n"), "{args}");
        assert!(!args.contains("--prompt"), "{args}");
        assert!(fs::read_to_string(&viewed)
            .unwrap()
            .contains("Recovered saved-native"));
        assert_eq!(fs::read_to_string(&presentations).unwrap(), "opened\n");
        assert_ne!(next.agent_process_id, fixture.attachment.agent_process_id);
        fixture.child.wait().unwrap();
        assert_eq!(fixture.store.session("native").unwrap(), Some(before));
        assert!(fixture
            .store
            .session_history("native", 0, 100)
            .unwrap()
            .starts_with(&history));
        let transport: String = Connection::open(&fixture.database)
            .unwrap()
            .query_row(
                "SELECT transport_lf_process_id FROM agent_sessions WHERE id='native'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(transport, owner.as_str());
    }
}
