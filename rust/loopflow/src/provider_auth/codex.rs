//! Codex owns OAuth and persistence; Loopflow owns opening the returned URL.

use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use super::{AuthCompletion, AuthError, AuthFlowHandle, AuthFlowResponse, Provider};
use crate::engine::process::ProcessGroupGuard;

const LOGIN_TIMEOUT: Duration = Duration::from_secs(10 * 60);

struct Connection {
    // Both guards remain owned through completion, cancellation and early errors.
    _child: Child,
    _process_group: ProcessGroupGuard,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Connection {
    async fn start(command: &mut Command) -> Result<Self, AuthError> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
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
        let mut connection = Self {
            _child: child,
            _process_group: process_group,
            stdin,
            stdout,
        };
        connection
            .request(
                1,
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
        self.stdin.write_all(&bytes).await.map_err(io_error)?;
        self.stdin.flush().await.map_err(io_error)
    }

    async fn receive(&mut self) -> Result<Value, AuthError> {
        let mut line = String::new();
        if self.stdout.read_line(&mut line).await.map_err(io_error)? == 0 {
            return Err(failed(
                "Codex app-server disconnected before auth completed",
            ));
        }
        // Never include raw protocol data: errors and notifications can contain secrets.
        serde_json::from_str(&line).map_err(|_| failed("invalid Codex app-server auth response"))
    }

    async fn request(&mut self, id: i64, method: &str, params: Value) -> Result<Value, AuthError> {
        tokio::time::timeout(Duration::from_secs(15), async {
            self.send(json!({"id": id, "method": method, "params": params}))
                .await?;
            loop {
                let message = self.receive().await?;
                if message["id"].as_i64() != Some(id) {
                    continue;
                }
                if message.get("error").is_some() {
                    return Err(failed("Codex app-server rejected the auth request"));
                }
                return message
                    .get("result")
                    .cloned()
                    .ok_or_else(|| failed("Codex app-server auth response has no result"));
            }
        })
        .await
        .map_err(|_| failed("timed out waiting for Codex app-server auth response"))?
    }

    async fn complete_login(mut self, login_id: String) -> Result<(), AuthError> {
        tokio::time::timeout(LOGIN_TIMEOUT, async {
            loop {
                let message = self.receive().await?;
                if message["method"] != "account/login/completed"
                    || message["params"]["loginId"].as_str() != Some(login_id.as_str())
                {
                    continue;
                }
                return if message["params"]["success"] == true {
                    Ok(())
                } else {
                    Err(failed("Codex login failed; retry lf auth connect codex"))
                };
            }
        })
        .await
        .map_err(|_| failed("timed out waiting for Codex login completion"))?
    }
}

pub(super) async fn refresh(command: &mut Command) -> Result<Value, AuthError> {
    Connection::start(command)
        .await?
        .request(2, "account/read", json!({"refreshToken": true}))
        .await
}

pub(super) async fn start_login(command: &mut Command) -> Result<AuthFlowHandle, AuthError> {
    let mut connection = Connection::start(command).await?;
    // Unlike `codex login`, this endpoint sets open_browser=false inside Codex.
    let result = connection
        .request(2, "account/login/start", json!({"type": "chatgpt"}))
        .await?;
    let login_id = result["loginId"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| failed("Codex login response has no login ID"))?
        .to_string();
    let url = result["authUrl"]
        .as_str()
        .ok_or_else(|| failed("Codex login response has no authorization URL"))?;
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| failed("Codex login response has an invalid authorization URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(failed(
            "Codex login response has an invalid authorization URL",
        ));
    }
    let response = AuthFlowResponse {
        completion: AuthCompletion::Browser { manual_uri: None },
        provider: Provider::Codex,
        verification_uri: super::strip_query(url).to_string(),
        verification_uri_complete: Some(url.to_string()),
        user_code: None,
        expires_in: Some(LOGIN_TIMEOUT.as_secs()),
    };
    Ok(AuthFlowHandle::new(
        response,
        tokio::spawn(connection.complete_login(login_id)),
    ))
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

#[cfg(all(test, unix))]
mod tests {
    use super::{start_login, AuthCompletion, LOGIN_TIMEOUT};
    use std::fs;
    use std::path::Path;
    use std::time::Duration;
    use tokio::process::Command;

    fn server(root: &Path, after_start: &str) -> Command {
        let script = root.join("server.sh");
        fs::write(&script, format!(r#"
printf '%s' "$$" > "$1"
read -r initialize
echo '{{"id":1,"result":{{}}}}'
read -r initialized
read -r login
case "$login" in *account/login/start*) ;; *) exit 90;; esac
echo '{{"id":2,"result":{{"type":"chatgpt","loginId":"this-login","authUrl":"https://example.com/authorize?state=private-state"}}}}'
{after_start}
"#)).unwrap();
        let mut command = Command::new("/bin/sh");
        command.arg(script).arg(root.join("pid"));
        command
    }

    #[tokio::test]
    async fn login_requires_its_matching_completion_and_hides_provider_errors() {
        for (completion, expected) in [
            (
                r#"echo '{"method":"account/login/completed","params":{"loginId":"this-login","success":true}}'"#,
                None,
            ),
            (
                r#"echo '{"method":"account/login/completed","params":{"loginId":"another-login","success":true}}'
echo '{"method":"account/updated","params":{}}'
echo '{"method":"account/login/completed","params":{"loginId":"this-login","success":false,"error":"private-token"}}'"#,
                Some("Codex login failed"),
            ),
            ("exit 0", Some("disconnected")),
            ("echo private-token", Some("invalid Codex")),
        ] {
            let root = tempfile::tempdir().unwrap();
            let handle = start_login(&mut server(root.path(), completion))
                .await
                .unwrap();
            assert_eq!(
                handle.response.verification_uri,
                "https://example.com/authorize"
            );
            assert_eq!(
                handle.response.verification_uri_complete.as_deref(),
                Some("https://example.com/authorize?state=private-state")
            );
            assert!(matches!(
                handle.response.completion,
                AuthCompletion::Browser { .. }
            ));
            assert!(!handle.supports_authorization_code());
            let result = handle.wait().await;
            if let Some(expected) = expected {
                let error = result.unwrap_err().to_string();
                assert!(error.contains(expected), "{error}");
                assert!(!error.contains("private-token"));
                assert!(!error.contains("private-state"));
            } else {
                result.unwrap();
            }
        }
    }

    async fn wait_for_exit(root: &Path) {
        let pid: i32 = fs::read_to_string(root.join("pid"))
            .unwrap()
            .parse()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                // SAFETY: signal 0 only checks existence of this fixture's child.
                if unsafe { libc::kill(pid, 0) } == -1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("auth child should terminate");
    }

    #[tokio::test]
    async fn dropping_login_stops_the_owned_app_server() {
        let root = tempfile::tempdir().unwrap();
        let handle = start_login(&mut server(root.path(), "read -r pending"))
            .await
            .unwrap();
        drop(handle);
        wait_for_exit(root.path()).await;
    }

    #[tokio::test]
    async fn login_timeout_stops_the_owned_app_server() {
        let root = tempfile::tempdir().unwrap();
        let handle = start_login(&mut server(root.path(), "read -r pending"))
            .await
            .unwrap();
        tokio::time::pause();
        tokio::task::yield_now().await;
        tokio::time::advance(LOGIN_TIMEOUT).await;
        let error = handle.wait().await.unwrap_err().to_string();
        tokio::time::resume();
        assert!(error.contains("timed out"), "{error}");
        wait_for_exit(root.path()).await;
    }
}
