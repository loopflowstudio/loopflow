use super::wire::{Operation, Reply, Request, Response, MAX_FRAME, MAX_REQUEST, VERSION};
use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// The service answered the exact-owner request, but could not bridge it.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct BridgeRefused(String);

pub fn connect(operation: Operation) -> Result<(UnixStream, Response)> {
    let mut socket = UnixStream::connect(super::socket_path()?)
        .context("Machine service unavailable; run `lf machine service ensure`")?;
    // Initial Tailscale certificate issuance can outlast an ordinary read.
    let timeout = if matches!(operation, Operation::ConfigureRemote { .. }) {
        60
    } else {
        10
    };
    socket.set_read_timeout(Some(Duration::from_secs(timeout)))?;
    socket.set_write_timeout(Some(Duration::from_secs(5)))?;
    let id = uuid::Uuid::new_v4().to_string();
    let request = Request {
        version: VERSION,
        id: id.clone(),
        operation,
    };
    let bytes = serde_json::to_vec(&request)?;
    anyhow::ensure!(bytes.len() <= MAX_REQUEST, "Machine request is too large");
    socket.write_all(&(bytes.len() as u32).to_be_bytes())?;
    socket.write_all(&bytes)?;
    let response = receive(&mut socket)?;
    anyhow::ensure!(
        response.id == id,
        "Machine response does not answer this request"
    );
    Ok((socket, response))
}

pub fn receive(socket: &mut UnixStream) -> Result<Response> {
    let mut header = [0; 4];
    socket.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    anyhow::ensure!(length <= MAX_FRAME, "Machine response is too large");
    let mut bytes = vec![0; length];
    socket.read_exact(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn bridge(owner: &crate::id::LfProcessId) -> Result<UnixStream> {
    // Restored terminals can open before Desktop's first Work reading obtains
    // the helper. Setup delegates through the same selected installation.
    let setup = std::process::Command::new(crate::os_process::resolve_lf_binary()?)
        .args(["machine", "service", "ensure"])
        .stdin(std::process::Stdio::null())
        .output()?;
    anyhow::ensure!(
        setup.status.success(),
        "Machine service unavailable: {}",
        String::from_utf8_lossy(&setup.stderr)
    );
    let (socket, response) = connect(Operation::Attach {
        owner: owner.clone(),
    })?;
    if let Reply::Error { code, message } = &response.result {
        return Err(BridgeRefused(format!("{code:?}: {message}")).into());
    }
    anyhow::ensure!(
        matches!(response.result, Reply::Bridged),
        "Machine did not open the owner bridge"
    );
    socket.set_read_timeout(None)?;
    Ok(socket)
}
