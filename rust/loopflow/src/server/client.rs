use super::wire::{Operation, Reply, Request, Response, MAX_FRAME, MAX_REQUEST, VERSION};
use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

#[derive(Debug, serde::Serialize)]
pub struct Status {
    pub socket: std::path::PathBuf,
    pub ready: bool,
    pub instance: Option<String>,
    pub error: Option<String>,
}

pub fn status() -> Result<Status> {
    let socket = super::socket_path()?;
    let (instance, error) = match connect(Operation::Bootstrap) {
        Ok((_, response)) => {
            let error = match response.result {
                Reply::Bootstrap {
                    version,
                    executable_sha256,
                    ..
                } => {
                    let expected = crate::installation::ArtifactIdentity::capture(
                        crate::installation::ArtifactRole::Cli,
                        &std::env::current_exe()?,
                    )?
                    .sha256;
                    (version != VERSION || executable_sha256 != expected).then(||
                        "Loopflow server uses another executable or protocol; obtain the selected installation".into()
                    )
                }
                Reply::Error { code, message } => Some(format!("{code:?}: {message}")),
                _ => Some("Server did not answer the readiness request".into()),
            };
            (Some(response.instance), error)
        }
        Err(error) => (None, Some(error.to_string())),
    };
    Ok(Status {
        socket,
        ready: error.is_none(),
        instance,
        error,
    })
}

/// The service answered the exact-owner request, but could not bridge it.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct BridgeRefused(String);

pub fn connect(operation: Operation) -> Result<(UnixStream, Response)> {
    let mut socket = UnixStream::connect(super::socket_path()?).context(
        "Loopflow server unavailable; check `lf server status` and the selected installation",
    )?;
    socket.set_read_timeout(Some(Duration::from_secs(10)))?;
    socket.set_write_timeout(Some(Duration::from_secs(5)))?;
    let id = uuid::Uuid::new_v4().to_string();
    let request = Request {
        version: VERSION,
        id: id.clone(),
        operation,
    };
    let bytes = serde_json::to_vec(&request)?;
    anyhow::ensure!(bytes.len() <= MAX_REQUEST, "Server request is too large");
    socket.write_all(&(bytes.len() as u32).to_be_bytes())?;
    socket.write_all(&bytes)?;
    let response = receive(&mut socket)?;
    anyhow::ensure!(
        response.id == id,
        "Server response does not answer this request"
    );
    Ok((socket, response))
}

pub fn receive(socket: &mut UnixStream) -> Result<Response> {
    let mut header = [0; 4];
    socket.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    anyhow::ensure!(length <= MAX_FRAME, "Server response is too large");
    let mut bytes = vec![0; length];
    socket.read_exact(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn bridge(owner: &crate::id::LfProcessId) -> Result<UnixStream> {
    // Restored terminals can open before Desktop's first Work reading obtains
    // the helper. Setup delegates through the same selected installation.
    let setup = crate::installation::server::readiness_command()?.output()?;
    anyhow::ensure!(
        setup.status.success(),
        "Loopflow server unavailable: {}",
        String::from_utf8_lossy(&setup.stderr)
    );
    bridge_existing(owner)
}

/// Recovery never registers a service or changes installation policy.
pub(crate) fn bridge_existing(owner: &crate::id::LfProcessId) -> Result<UnixStream> {
    let (socket, response) = connect(Operation::Attach {
        owner: owner.clone(),
    })?;
    if let Reply::Error { code, message } = &response.result {
        return Err(BridgeRefused(format!("{code:?}: {message}")).into());
    }
    anyhow::ensure!(
        matches!(response.result, Reply::Bridged),
        "Server did not open the owner bridge"
    );
    socket.set_read_timeout(None)?;
    Ok(socket)
}
