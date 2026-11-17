//! Short-lived stdio RPC connection for native Codex configuration reads.
use crate::os_process::ProcessGroupGuard;
use crate::provider_auth::{AuthError, Provider};
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
#[derive(Debug)]
pub(crate) struct Connection {
    // Both guards remain owned through completion, cancellation and early errors.
    _child: Child,
    _process_group: ProcessGroupGuard,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr: ChildStderr,
    next_request_id: i64,
}

impl Connection {
    pub(crate) async fn start(command: &mut Command) -> Result<Self, AuthError> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn().map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                AuthError::CommandUnavailable {
                    provider: Provider::Codex,
                    command: "codex".into(),
                }
            } else {
                AuthError::CommandSpawn {
                    provider: Provider::Codex,
                    source,
                }
            }
        })?;
        let process_group =
            ProcessGroupGuard::new(child.id().expect("newly spawned Codex has a process id"));
        let stdin = child.stdin.take().expect("Codex stdin is piped");
        let stdout = BufReader::new(child.stdout.take().expect("Codex stdout is piped"));
        let stderr = child.stderr.take().expect("Codex stderr is piped");
        let mut connection = Self {
            _child: child,
            _process_group: process_group,
            stdin,
            stdout,
            stderr,
            next_request_id: 1,
        };
        connection
            .request(
                "initialize",
                json!({"clientInfo": {
                    "name": "loopflow", "title": "loopflow", "version": env!("CARGO_PKG_VERSION")
                }}),
            )
            .await?;
        connection.send(json!({"method": "initialized"})).await?;
        Ok(connection)
    }

    async fn send(&mut self, message: Value) -> Result<(), AuthError> {
        let mut bytes = message.to_string().into_bytes();
        bytes.push(b'\n');
        if let Err(error) = self.stdin.write_all(&bytes).await {
            return Err(self
                .failure(&format!("Codex app-server write failed: {error}"))
                .await);
        }
        if let Err(error) = self.stdin.flush().await {
            return Err(self
                .failure(&format!("Codex app-server flush failed: {error}"))
                .await);
        }
        Ok(())
    }

    async fn receive(&mut self) -> Result<Value, AuthError> {
        let mut line = String::new();
        if self.stdout.read_line(&mut line).await.map_err(io_error)? == 0 {
            return Err(self
                .failure("Codex app-server disconnected during configuration read")
                .await);
        }
        // Never include raw protocol data: errors and notifications can contain secrets.
        serde_json::from_str(&line).map_err(|_| failed("invalid Codex app-server response"))
    }

    async fn failure(&mut self, message: &str) -> AuthError {
        let mut stderr = String::new();
        let _ = tokio::time::timeout(
            Duration::from_secs(1),
            self.stderr.read_to_string(&mut stderr),
        )
        .await;
        failed(&format!(
            "{message}{}",
            crate::os_process::exit_reason(&stderr)
        ))
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value, AuthError> {
        let message = self.request_raw(method, params).await?;
        if message.get("error").is_some() {
            return Err(failed("Codex app-server rejected the request"));
        }
        message
            .get("result")
            .cloned()
            .ok_or_else(|| failed("Codex app-server response has no result"))
    }

    pub(crate) async fn request_raw(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, AuthError> {
        let id = self.next_request_id;
        self.next_request_id += 1;
        tokio::time::timeout(Duration::from_secs(15), async {
            self.send(json!({"id": id, "method": method, "params": params}))
                .await?;
            loop {
                let message = self.receive().await?;
                if message["id"].as_i64() == Some(id) {
                    return Ok(message);
                }
            }
        })
        .await
        .map_err(|_| failed("timed out waiting for Codex app-server response"))?
    }
}

fn failed(message: &str) -> AuthError {
    AuthError::CommandFailed {
        provider: Provider::Codex,
        message: message.into(),
    }
}

fn io_error(source: std::io::Error) -> AuthError {
    AuthError::CommandIo {
        provider: Provider::Codex,
        source,
    }
}
