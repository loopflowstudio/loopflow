use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialResponse {
    pub token: String,
    pub login: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStartResponse {
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub user_code: Option<String>,
    pub expires_in: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct CredentialSocketClient {
    socket_path: PathBuf,
}

#[derive(Debug, Error)]
pub enum CredentialSocketError {
    #[error("credential socket unavailable at {path}: {source}")]
    SocketUnavailable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("credential not found for provider {provider}")]
    NotFound { provider: String },
    #[error("credential socket returned HTTP {status} for {path}: {body}")]
    UnexpectedStatus {
        status: u16,
        path: String,
        body: String,
    },
    #[error("credential socket returned an invalid HTTP response: {0}")]
    InvalidResponse(String),
    #[error("credential socket response parse failed: {0}")]
    Parse(String),
}

#[derive(Debug)]
struct SocketHttpResponse {
    status: u16,
    body: Vec<u8>,
}

impl CredentialSocketClient {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    pub fn get_credential_blocking(
        &self,
        provider: &str,
    ) -> Result<CredentialResponse, CredentialSocketError> {
        let path = format!("/credentials/{provider}");
        let response = self.request_blocking("GET", path.as_str(), None)?;
        match response.status {
            200 => Self::decode_json(response.body.as_slice()),
            404 => Err(CredentialSocketError::NotFound {
                provider: provider.to_string(),
            }),
            status => Err(CredentialSocketError::UnexpectedStatus {
                status,
                path,
                body: String::from_utf8_lossy(&response.body).trim().to_string(),
            }),
        }
    }

    /// Fetch credential for a provider from the Concerto-hosted Unix socket.
    pub async fn get_credential(
        &self,
        provider: &str,
    ) -> Result<CredentialResponse, CredentialSocketError> {
        let provider_owned = provider.to_string();
        let client = self.clone();
        tokio::task::spawn_blocking(move || client.get_credential_blocking(provider_owned.as_str()))
            .await
            .map_err(|err| {
                CredentialSocketError::InvalidResponse(format!("credential task failed: {err}"))
            })?
    }

    pub async fn start_auth(
        &self,
        provider: &str,
    ) -> Result<AuthStartResponse, CredentialSocketError> {
        let path = format!("/auth/{provider}/start");
        let response = self
            .request("POST", path.as_str(), Some(b"{}".to_vec()))
            .await?;
        match response.status {
            200 | 201 => Self::decode_json(response.body.as_slice()),
            status => Err(CredentialSocketError::UnexpectedStatus {
                status,
                path,
                body: String::from_utf8_lossy(&response.body).trim().to_string(),
            }),
        }
    }

    pub async fn disconnect(&self, provider: &str) -> Result<(), CredentialSocketError> {
        let path = format!("/credentials/{provider}");
        let response = self.request("DELETE", path.as_str(), None).await?;
        match response.status {
            200 | 204 | 404 => Ok(()),
            status => Err(CredentialSocketError::UnexpectedStatus {
                status,
                path,
                body: String::from_utf8_lossy(&response.body).trim().to_string(),
            }),
        }
    }

    /// Check if the socket is reachable.
    pub async fn health(&self) -> bool {
        match self.request("GET", "/health", None).await {
            Ok(response) => response.status == 200,
            Err(_) => false,
        }
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<SocketHttpResponse, CredentialSocketError> {
        let client = self.clone();
        let method = method.to_string();
        let path = path.to_string();
        tokio::task::spawn_blocking(move || client.request_blocking(method.as_str(), &path, body))
            .await
            .map_err(|err| {
                CredentialSocketError::InvalidResponse(format!("socket request task failed: {err}"))
            })?
    }

    fn request_blocking(
        &self,
        method: &str,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<SocketHttpResponse, CredentialSocketError> {
        let mut stream = UnixStream::connect(&self.socket_path).map_err(|err| {
            CredentialSocketError::SocketUnavailable {
                path: self.socket_path.clone(),
                source: err,
            }
        })?;

        let payload = body.unwrap_or_default();
        let request = if payload.is_empty() {
            format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        } else {
            format!(
                "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                payload.len()
            )
        };

        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|err| CredentialSocketError::InvalidResponse(err.to_string()))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|err| CredentialSocketError::InvalidResponse(err.to_string()))?;

        stream.write_all(request.as_bytes()).map_err(|err| {
            CredentialSocketError::SocketUnavailable {
                path: self.socket_path.clone(),
                source: err,
            }
        })?;
        if !payload.is_empty() {
            stream.write_all(payload.as_slice()).map_err(|err| {
                CredentialSocketError::SocketUnavailable {
                    path: self.socket_path.clone(),
                    source: err,
                }
            })?;
        }

        let _ = stream.shutdown(Shutdown::Write);

        let mut response_bytes = Vec::new();
        stream.read_to_end(&mut response_bytes).map_err(|err| {
            CredentialSocketError::SocketUnavailable {
                path: self.socket_path.clone(),
                source: err,
            }
        })?;

        parse_http_response(response_bytes.as_slice())
    }

    fn decode_json<T>(bytes: &[u8]) -> Result<T, CredentialSocketError>
    where
        T: DeserializeOwned,
    {
        serde_json::from_slice(bytes).map_err(|err| CredentialSocketError::Parse(err.to_string()))
    }
}

fn parse_http_response(raw: &[u8]) -> Result<SocketHttpResponse, CredentialSocketError> {
    let Some(header_end) = raw.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Err(CredentialSocketError::InvalidResponse(
            "missing HTTP header delimiter".to_string(),
        ));
    };

    let headers = &raw[..header_end];
    let body = raw[(header_end + 4)..].to_vec();
    let headers_text = String::from_utf8_lossy(headers);
    let mut lines = headers_text.lines();
    let status_line = lines.next().ok_or_else(|| {
        CredentialSocketError::InvalidResponse("missing HTTP status line".to_string())
    })?;

    let status = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| CredentialSocketError::InvalidResponse("missing HTTP status".to_string()))?
        .parse::<u16>()
        .map_err(|err| CredentialSocketError::InvalidResponse(err.to_string()))?;

    Ok(SocketHttpResponse { status, body })
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::thread;

    use tempfile::tempdir;

    use super::*;

    fn start_socket_server(socket_path: PathBuf, response: String) -> thread::JoinHandle<()> {
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path).expect("bind unix listener");

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept unix connection");
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request);
            stream
                .write_all(response.as_bytes())
                .expect("write HTTP response");
        })
    }

    #[test]
    fn get_credential_blocking_reads_json_response() {
        let temp = tempdir().expect("tempdir");
        let socket_path = temp.path().join("credentials.sock");
        let body = r#"{"token":"abc123","login":"jack","expires_at":null}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let server = start_socket_server(socket_path.clone(), response);

        let client = CredentialSocketClient::new(socket_path.clone());
        let credential = client
            .get_credential_blocking("github")
            .expect("credential response");

        assert_eq!(credential.token, "abc123");
        assert_eq!(credential.login, Some("jack".to_string()));
        assert_eq!(credential.expires_at, None);

        server.join().expect("server join");
    }

    #[test]
    fn get_credential_blocking_maps_not_found_status() {
        let temp = tempdir().expect("tempdir");
        let socket_path = temp.path().join("credentials.sock");
        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_string();
        let server = start_socket_server(socket_path.clone(), response);

        let client = CredentialSocketClient::new(socket_path.clone());
        let err = client
            .get_credential_blocking("claude")
            .expect_err("missing credential should fail");

        match err {
            CredentialSocketError::NotFound { provider } => assert_eq!(provider, "claude"),
            _ => panic!("expected not-found error"),
        }

        server.join().expect("server join");
    }

    #[tokio::test]
    async fn health_returns_true_for_http_200() {
        let temp = tempdir().expect("tempdir");
        let socket_path = temp.path().join("health.sock");
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_string();
        let server = start_socket_server(socket_path.clone(), response);

        let client = CredentialSocketClient::new(socket_path.clone());
        assert!(client.health().await);

        server.join().expect("server join");
    }
}
