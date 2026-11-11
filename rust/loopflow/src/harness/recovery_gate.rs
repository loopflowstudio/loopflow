//! A native submission holds this connection while the existing owner prepares
//! recovery. Closing it cancels installation, not an in-flight refresh grant.
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::session_recovery::SessionRecovery;

const PREFIX: &str = "<lf-recovery:";
const SUFFIX: &str =
    "> Continue the preceding request that was rejected before work by the usage limit.";
const WAIT: Duration = Duration::from_secs(40);

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Envelope {
    session: String,
    thread: String,
    failed: String,
    attempt: String,
    socket: PathBuf,
}

#[derive(Debug)]
pub(super) struct RecoveryGate {
    _directory: tempfile::TempDir,
    listener: UnixListener,
    envelope: Envelope,
    expires: SystemTime,
}

impl RecoveryGate {
    pub(super) fn new(session: &str, recovery: &SessionRecovery) -> Result<Self> {
        // Unix socket addresses must fit even when LF_HOME is a long checkout path.
        let directory = tempfile::Builder::new()
            .prefix("lf-recovery-")
            .tempdir_in("/tmp")?;
        let socket = directory.path().join("gate");
        let listener = UnixListener::bind(&socket)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            _directory: directory,
            listener,
            envelope: Envelope {
                session: session.into(),
                thread: recovery.thread.to_string(),
                failed: recovery.failed_turn.clone(),
                attempt: uuid::Uuid::new_v4().to_string(),
                socket,
            },
            expires: SystemTime::now() + WAIT,
        })
    }

    pub(super) fn attempt(&self) -> &str {
        &self.envelope.attempt
    }

    pub(super) fn prompt(&self) -> String {
        format!(
            "{PREFIX}{}{SUFFIX}",
            serde_json::to_string(&self.envelope).expect("envelope is serializable")
        )
    }

    /// The listener is consumed once, including when native cancellation requeues input.
    pub(super) fn accept(self) -> Result<Admission> {
        let mut socket = loop {
            ensure!(
                SystemTime::now() < self.expires,
                "Native recovery admission expired"
            );
            match self.listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => return Err(error.into()),
            }
        };
        socket.set_read_timeout(Some(Duration::from_secs(2)))?;
        let mut line = String::new();
        BufReader::new((&mut socket).take(1024 * 1024)).read_line(&mut line)?;
        let input: Value = serde_json::from_str(&line)?;
        ensure!(
            input["prompt"].as_str() == Some(self.prompt().as_str()),
            "Recovery envelope changed"
        );
        ensure!(
            input["session_id"] == self.envelope.thread,
            "Recovery thread changed"
        );
        ensure!(
            input["hook_event_name"] == "UserPromptSubmit",
            "Not native prompt admission"
        );
        socket.set_nonblocking(true)?;
        drop(self.listener);
        Ok(Admission {
            socket,
            input,
            expires: SystemTime::now() + WAIT,
            _directory: self._directory,
        })
    }
}

#[derive(Debug)]
pub(super) struct Admission {
    socket: UnixStream,
    pub(super) input: Value,
    expires: SystemTime,
    _directory: tempfile::TempDir,
}

impl Admission {
    pub(super) fn transcript(&self) -> Result<Vec<Value>> {
        let path = self.input["transcript_path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Native history unavailable"))?;
        read_transcript(Path::new(path))
    }

    pub(super) fn check(&self) -> Result<()> {
        ensure!(
            SystemTime::now() < self.expires,
            "Recovery preparation expired"
        );
        let mut byte = 0u8;
        // SAFETY: the socket owns this descriptor; byte is writable for one byte.
        let read = unsafe {
            libc::recv(
                self.socket.as_raw_fd(),
                (&mut byte as *mut u8).cast(),
                1,
                libc::MSG_PEEK | libc::MSG_DONTWAIT,
            )
        };
        if read == -1 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::WouldBlock {
            return Ok(());
        }
        bail!("Native recovery admission was cancelled")
    }

    pub(super) fn finish(mut self) -> Result<()> {
        self.check()?;
        self.socket.write_all(b"{}\n")?;
        Ok(())
    }
}

pub(super) fn matches_input(item: &Value, session: &str, recovery: &SessionRecovery) -> bool {
    if item["type"] != "userMessage" {
        return false;
    }
    let Some(content) = item["content"].as_array() else {
        return false;
    };
    let [text] = content.as_slice() else {
        return false;
    };
    let Some(encoded) = text["text"]
        .as_str()
        .and_then(|text| text.strip_prefix(PREFIX))
        .and_then(|text| text.strip_suffix(SUFFIX))
    else {
        return false;
    };
    let Ok(envelope) = serde_json::from_str::<Envelope>(encoded) else {
        return false;
    };
    envelope.session == session
        && envelope.thread == recovery.thread.as_str()
        && envelope.failed == recovery.failed_turn
        && Some(&envelope.attempt) == recovery.request.as_ref()
}

/// Called only by the recovery-only wrapper. Ordinary input needs no lf store.
pub(crate) fn hook(input: &Value) -> Result<()> {
    let Some(prompt) = input["prompt"].as_str() else {
        return Ok(());
    };
    let Some(encoded) = prompt
        .strip_prefix(PREFIX)
        .and_then(|p| p.strip_suffix(SUFFIX))
    else {
        return Ok(());
    };
    let envelope: Envelope = serde_json::from_str(encoded)?;
    let mut socket = UnixStream::connect(envelope.socket)?;
    socket.set_read_timeout(Some(WAIT + Duration::from_secs(2)))?;
    socket.set_write_timeout(Some(Duration::from_secs(2)))?;
    writeln!(socket, "{input}")?;
    let mut response = String::new();
    BufReader::new(socket).read_line(&mut response)?;
    ensure!(response == "{}\n", "Recovery admission did not complete");
    print!("{response}");
    Ok(())
}

pub(crate) fn hook_command(home: &std::path::Path) -> Result<String> {
    let quote = |path: &std::path::Path| crate::os_process::shell_escape(&path.to_string_lossy());
    Ok(command(&format!(
        "env LF_HOME={} {} __context-block --moment",
        quote(home),
        quote(&crate::os_process::resolve_lf_binary()?)
    )))
}

/// The shell survives an lf failure and Codex's native fail-open timeout. It
/// recognizes only the reserved envelope at the start of a prompt JSON field.
fn command(command: &str) -> String {
    format!(
        r#"payload=$(/bin/cat); case "$payload" in *'"prompt":"<lf-recovery:'*|*'"prompt": "<lf-recovery:'*)
result=$(printf '%s' "$payload" | {command} recovery & gate=$!;
(sleeper=; stopping=0; trap 'stopping=1; [ -z "$sleeper" ] || kill "$sleeper" 2>/dev/null' TERM INT;
sleep 45 & sleeper=$!; [ "$stopping" = 0 ] || kill "$sleeper" 2>/dev/null;
wait "$sleeper"; [ "$stopping" = 0 ] && kill -TERM "$gate" 2>/dev/null) & timer=$!;
wait "$gate"; gate_status=$?; kill "$timer" 2>/dev/null; wait "$timer" 2>/dev/null; exit "$gate_status");
if [ "$?" = 0 ] && [ "$result" = '{{}}' ]; then printf '%s\n' "$result"; else printf '%s\n' '{{"decision":"block","reason":"Recovery did not finish; continue from the conversation"}}'; fi;; *) exit 0;; esac"#
    )
}

pub(super) fn read_transcript(path: &Path) -> Result<Vec<Value>> {
    std::fs::read_to_string(path)?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

/// The native hook has reserved submission. The retained failed input, completed
/// rejection and current model must still be the ones the observer inspected.
pub(super) fn codex_current(admission: &Admission, recovery: &SessionRecovery) -> Result<String> {
    let turn = admission.input["turn_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Native retry has no turn ID"))?;
    ensure!(
        turn != recovery.failed_turn,
        "Retry names the rejected turn"
    );
    let rows = admission.transcript()?;
    let user = rows
        .iter()
        .rev()
        .find(|row| {
            row["type"] == "response_item"
                && row["payload"]["type"] == "message"
                && row["payload"]["role"] == "user"
        })
        .ok_or_else(|| anyhow::anyhow!("Rejected input unavailable"))?;
    ensure!(
        user.pointer("/payload/internal_chat_message_metadata_passthrough/turn_id")
            .and_then(Value::as_str)
            == Some(&recovery.failed_turn),
        "Newer input superseded recovery"
    );
    let completed = rows
        .iter()
        .rev()
        .find(|row| row["type"] == "event_msg" && row["payload"]["type"] == "task_complete")
        .ok_or_else(|| anyhow::anyhow!("Native rejection unavailable"))?;
    ensure!(
        completed["payload"]["turn_id"] == recovery.failed_turn
            && completed["payload"]["error"]["codex_error_info"] == "usage_limit_exceeded",
        "Newer work superseded recovery"
    );
    let context = rows
        .iter()
        .rev()
        .find(|row| row["type"] == "turn_context")
        .ok_or_else(|| anyhow::anyhow!("Native model unavailable"))?;
    ensure!(
        context["payload"]["turn_id"] == turn
            && context["payload"]["model"].as_str() == recovery.model.as_deref(),
        "Native model or turn changed"
    );
    Ok(turn.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{RecoveryGate, SUFFIX};
    use crate::session_recovery::{RecoveryState, SessionRecovery};
    use serde_json::json;
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    fn gate() -> RecoveryGate {
        RecoveryGate::new(
            "session",
            &SessionRecovery {
                agent_process_id: crate::id::AgentProcessId::new(),
                thread: "thread".into(),
                failed_turn: "failed".into(),
                model: None,
                account: None,
                state: RecoveryState::WaitingNative,
                reason: String::new(),
                retry_at: None,
                request: None,
                retry_turn: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn cancellation_revokes_installation_and_duplicate_admission() {
        let gate = gate();
        let mut client = UnixStream::connect(&gate.envelope.socket).unwrap();
        writeln!(client, "{}", json!({"prompt":gate.prompt(),"session_id":"thread","hook_event_name":"UserPromptSubmit"})).unwrap();
        let admission = gate.accept().unwrap();
        admission.check().unwrap();
        let path = admission._directory.path().join("gate");
        drop(client);
        assert!(admission.check().is_err());
        assert!(admission.finish().is_err());
        assert!(UnixStream::connect(path).is_err());
    }

    #[test]
    fn broken_lf_blocks_only_the_reserved_recovery_prompt() {
        use std::process::{Command, Stdio};
        for (prompt, blocked) in [
            ("Explain recovery to me".to_owned(), false),
            (gate().prompt(), true),
        ] {
            let mut child = Command::new("/bin/sh")
                .args(["-c", &super::command("false")])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            write!(child.stdin.take().unwrap(), "{}", json!({"prompt":prompt})).unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(output.status.success());
            if blocked {
                let decision: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(decision["decision"], "block");
            } else {
                assert!(output.stdout.is_empty());
            }
        }
    }

    #[test]
    fn completion_requires_the_exact_admitted_input() {
        let gate = gate();
        let mut recovery = SessionRecovery {
            agent_process_id: crate::id::AgentProcessId::new(),
            thread: "thread".into(),
            failed_turn: "failed".into(),
            model: None,
            account: None,
            state: RecoveryState::Recovering,
            reason: String::new(),
            retry_at: None,
            request: Some(gate.attempt().into()),
            retry_turn: Some("native".into()),
        };
        let input = json!({"type":"userMessage","content":[{"type":"text","text":gate.prompt()}]});
        assert!(super::matches_input(&input, "session", &recovery));
        assert!(!super::matches_input(&input, "other-session", &recovery));
        recovery.request = Some("earlier-attempt".into());
        assert!(!super::matches_input(&input, "session", &recovery));
        assert!(!super::matches_input(
            &json!({"type":"agentMessage","text":"completed"}),
            "session",
            &recovery
        ));
    }

    #[test]
    fn wrong_thread_cannot_admit_an_attempt() {
        let gate = gate();
        let mut client = UnixStream::connect(&gate.envelope.socket).unwrap();
        writeln!(client, "{}", json!({"prompt":gate.prompt(),"session_id":"other","hook_event_name":"UserPromptSubmit"})).unwrap();
        assert!(gate.accept().is_err());
        super::hook(&json!({"prompt":format!("Explain <lf-recovery:example{SUFFIX}")})).unwrap();
    }
}
