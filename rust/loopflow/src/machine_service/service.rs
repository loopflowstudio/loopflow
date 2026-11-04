use std::fs::OpenOptions;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use fs2::FileExt;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Notify, Semaphore};

use super::wire::{
    ErrorCode, Operation, Reply, Request, Response, MAX_CONNECTIONS, MAX_FRAME, MAX_REQUEST,
    VERSION,
};
use crate::lf::commands::{read::Read, write::Write};

pub(super) async fn run(remote_port: Option<u16>) -> Result<()> {
    let path = super::socket_path()?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(path.with_extension("lock"))?;
    lock.try_lock_exclusive()
        .context("Machine service is already running")?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    let executable_sha256 = Arc::new(
        crate::installation::ArtifactIdentity::capture(
            crate::installation::ArtifactRole::Cli,
            &std::env::current_exe()?,
        )?
        .sha256,
    );
    let instance = Arc::new(uuid::Uuid::new_v4().to_string());
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    // Both carriers dispatch the same operations after their own admission.
    // Remote access never connects to the private socket as a proxy Unix user.
    let remote = async {
        match remote_port {
            Some(port) => {
                super::remote::run(
                    port,
                    instance.clone(),
                    executable_sha256.clone(),
                    slots.clone(),
                )
                .await
            }
            None => std::future::pending::<Result<()>>().await,
        }
    };
    tracing::info!(%instance, socket = %path.display(), "Machine service ready");
    tokio::select! {
        result = remote => result,
        result = local(listener, instance.clone(), executable_sha256.clone(), slots.clone()) => result,
    }
}

async fn local(
    listener: UnixListener,
    instance: Arc<String>,
    executable_sha256: Arc<String>,
    slots: Arc<Semaphore>,
) -> Result<()> {
    loop {
        let (socket, _) = listener.accept().await?;
        // Unix admission is deliberately unavailable to remote proxies.
        // SAFETY: geteuid has no preconditions.
        if socket.peer_cred()?.uid() != unsafe { libc::geteuid() } {
            continue;
        }
        let Ok(permit) = slots.clone().try_acquire_owned() else {
            tracing::warn!(%instance, "Machine connection capacity reached");
            continue;
        };
        tracing::debug!(
            active = MAX_CONNECTIONS - slots.available_permits(),
            "Machine connection admitted"
        );
        let instance = instance.clone();
        let executable_sha256 = executable_sha256.clone();
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(error) = connection(socket, &instance, &executable_sha256).await {
                tracing::warn!(%instance, %error, "Machine connection ended");
            }
        });
    }
}

pub(super) async fn connection(
    mut socket: impl AsyncRead + AsyncWrite + Unpin,
    instance: &str,
    executable_sha256: &str,
) -> Result<()> {
    let bytes =
        tokio::time::timeout(Duration::from_secs(5), receive(&mut socket, MAX_REQUEST)).await??;
    let request: Request = match serde_json::from_slice(&bytes) {
        Ok(request) => request,
        Err(error) => {
            return respond(
                &mut socket,
                "",
                instance,
                Reply::Error {
                    code: ErrorCode::Invalid,
                    message: error.to_string(),
                },
            )
            .await
        }
    };
    let id = &request.id;
    if request.version != VERSION {
        return respond(
            &mut socket,
            id,
            instance,
            Reply::Error {
                code: ErrorCode::Incompatible,
                message: format!(
                    "Machine protocol {VERSION} required; update the client or service"
                ),
            },
        )
        .await;
    }
    anyhow::ensure!(
        !id.is_empty() && id.len() <= 128,
        "request ID must contain 1–128 bytes"
    );
    let started = Instant::now();
    let result = operation(
        &mut socket,
        id,
        instance,
        executable_sha256,
        request.operation,
    )
    .await;
    tracing::debug!(request = %id, %instance, elapsed_ms = started.elapsed().as_millis(), ok = result.is_ok(), "Machine request ended");
    if let Err(error) = result {
        tracing::warn!(request = %id, %instance, %error, "Machine request failed");
        respond(
            &mut socket,
            id,
            instance,
            Reply::Error {
                code: ErrorCode::Unavailable,
                message: error.to_string(),
            },
        )
        .await?;
    }
    Ok(())
}

async fn operation(
    socket: &mut (impl AsyncRead + AsyncWrite + Unpin),
    id: &str,
    instance: &str,
    executable_sha256: &str,
    operation: Operation,
) -> Result<()> {
    let reply = match operation {
        Operation::SessionOwner { session } => {
            let owner = tokio::task::spawn_blocking(move || -> Result<_> {
                let registry = crate::store::read_existing_registry()?
                    .context("Machine registry is unavailable")?;
                Ok(registry.sqlite.session_transport(session.as_str())?)
            })
            .await??;
            Reply::SessionOwner { owner }
        }
        Operation::Bootstrap => {
            let machine = tokio::task::spawn_blocking(|| Read::MachineId.read()).await??;
            Reply::Bootstrap {
                version: VERSION,
                build: crate::build_info::BUILD_VERSION.to_owned(),
                executable_sha256: executable_sha256.into(),
                machine: serde_json::from_str(&machine)?,
                capabilities: vec![
                    "roadmap".into(),
                    "machine_id".into(),
                    "portfolio".into(),
                    "repository_path".into(),
                    "session_rename".into(),
                    "owner_bridge".into(),
                    "session_owner".into(),
                    "desktop_sessions".into(),
                    "view_plan".into(),
                ],
                max_frame: MAX_FRAME,
                max_connections: MAX_CONNECTIONS,
            }
        }
        Operation::Read { query } if supported_read(&query) => Reply::Read {
            output: tokio::task::spawn_blocking(move || query.read()).await??,
        },
        Operation::Subscribe { query, generation } if matches!(query, Read::Roadmap { .. }) => {
            return subscribe(socket, id, instance, query, generation).await;
        }
        Operation::Command { change } if matches!(change, Write::SessionRename { .. }) => {
            command(change, id, instance)
                .await
                .unwrap_or_else(|error| Reply::Error {
                    code: ErrorCode::OutcomeUnknown,
                    message: format!("{error}; inspect Session state before retrying"),
                })
        }
        Operation::Read { .. } | Operation::Subscribe { .. } | Operation::Command { .. } => {
            Reply::Error {
                code: ErrorCode::Unsupported,
                message: "Operation is outside this service's advertised capabilities".into(),
            }
        }
        Operation::Attach { owner } => {
            let mut destination = UnixStream::connect(crate::transport::endpoint(&owner)?)
                .await
                .context(
                    "Session owner is unavailable; no provider restarted and no input replayed",
                )?;
            respond(socket, id, instance, Reply::Bridged).await?;
            // Tokio uses bounded copy buffers. Closing either side releases the
            // owner claim; no service state can restore it on another connection.
            if let Err(error) = tokio::io::copy_bidirectional(socket, &mut destination).await {
                // Once bridged, this connection speaks only the owner protocol.
                // Close it on failure; a JSON error would corrupt that stream.
                tracing::debug!(%error, "Machine owner bridge disconnected");
            }
            return Ok(());
        }
    };
    respond(socket, id, instance, reply).await
}

fn supported_read(query: &Read) -> bool {
    matches!(
        query,
        Read::Roadmap { .. }
            | Read::DesktopSessions { .. }
            | Read::MachineId
            | Read::Portfolio
            | Read::RepositoryPath { .. }
            | Read::ViewPlan { .. }
    )
}

async fn subscribe(
    socket: &mut (impl AsyncRead + AsyncWrite + Unpin),
    id: &str,
    instance: &str,
    query: Read,
    generation: u64,
) -> Result<()> {
    let notify = Arc::new(Notify::new());
    let wake = notify.clone();
    let subscription = Arc::new(
        tokio::task::spawn_blocking(move || query.subscribe(move || wake.notify_one())).await??,
    );
    // Keep the in-flight read future across wakes; cancellation cannot discard a
    // partial frame then interpret its payload as the next length prefix.
    let (mut reader, mut writer) = tokio::io::split(socket);
    let mut input = Box::pin(receive(&mut reader, MAX_REQUEST));
    loop {
        tokio::select! {
            bytes = &mut input => {
                let bytes = bytes?;
                // The shared watch owns checkpoint parsing and its final error
                // reading, including argument validation and request limits.
                subscription.request(&bytes);
                drop(input);
                input = Box::pin(receive(&mut reader, MAX_REQUEST));
            }
            () = notify.notified() => {
                let pull = subscription.clone();
                let frame = tokio::task::spawn_blocking(move || pull.pull()).await?;
                if let Some(frame) = frame {
                    respond(&mut writer, id, instance, Reply::Reading {
                        generation, frame: serde_json::from_slice(&frame)?,
                    }).await?;
                }
                if subscription.ended() { return Ok(()); }
            }
        }
    }
}

async fn command(change: Write, id: &str, instance: &str) -> Result<Reply> {
    let executable = crate::os_process::resolve_lf_binary()?;
    let mut command = tokio::process::Command::new(&executable);
    // A service has no caller Session/Task. Never lend its launcher's identity
    // to requests, or change process-global environment while serving clients.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LF_") {
            command.env_remove(key);
        }
    }
    let trace_id = crate::id::TraceId::default();
    command.env("LF_TRACE_ID", trace_id.as_str());
    command.env("LF_HOME", crate::store::lf_home_dir());
    command.env("LF_BIN", executable);
    command
        .args(change.argv())
        .current_dir(change.directory())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn()?;
    tracing::info!(request = %id, %instance, pid = child.id(), %trace_id, "Machine command started");
    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    let (output, errors, status) = tokio::join!(
        read_command_output(stdout),
        read_command_output(stderr),
        child.wait()
    );
    let status = status?;
    tracing::info!(request = %id, %instance, code = status.code(), %trace_id, "Machine command exited");
    Ok(Reply::Command {
        trace_id,
        output: output?,
        stderr: errors?,
        code: status.code().unwrap_or(1),
    })
}

async fn read_command_output(stream: impl AsyncRead + Unpin) -> Result<String> {
    let mut bytes = Vec::new();
    stream
        .take((MAX_FRAME / 2) as u64)
        .read_to_end(&mut bytes)
        .await?;
    anyhow::ensure!(
        bytes.len() < MAX_FRAME / 2,
        "command output exceeded its limit"
    );
    Ok(String::from_utf8(bytes)?)
}

pub(super) async fn receive(
    reader: &mut (impl AsyncRead + Unpin),
    limit: usize,
) -> Result<Vec<u8>> {
    let length = reader.read_u32().await? as usize;
    anyhow::ensure!(length <= limit, "Machine frame exceeds {limit} bytes");
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).await?;
    Ok(bytes)
}

async fn respond(
    writer: &mut (impl AsyncWrite + Unpin),
    id: &str,
    instance: &str,
    result: Reply,
) -> Result<()> {
    let bytes = serde_json::to_vec(&Response {
        id: id.into(),
        instance: instance.into(),
        result,
    })?;
    anyhow::ensure!(
        bytes.len() <= MAX_FRAME,
        "Machine reading exceeds frame limit"
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        writer.write_u32(bytes.len() as u32).await?;
        writer.write_all(&bytes).await
    })
    .await??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{connection, receive};
    use crate::machine_service::wire::{
        ErrorCode, Operation, Reply, Request, Response, MAX_FRAME, VERSION,
    };
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;

    #[tokio::test]
    async fn incompatible_client_is_rejected_before_private_work_is_read() {
        let (mut client, server) = UnixStream::pair().unwrap();
        let worker =
            tokio::spawn(async move { connection(server, "incarnation", "fixture").await });
        let bytes = serde_json::to_vec(&Request {
            id: "request-1".into(),
            version: VERSION + 1,
            operation: Operation::Bootstrap,
        })
        .unwrap();
        client.write_u32(bytes.len() as u32).await.unwrap();
        client.write_all(&bytes).await.unwrap();
        let reply: Response =
            serde_json::from_slice(&receive(&mut client, MAX_FRAME).await.unwrap()).unwrap();
        assert_eq!(reply.id, "request-1");
        assert_eq!(reply.instance, "incarnation");
        assert!(matches!(
            reply.result,
            Reply::Error {
                code: ErrorCode::Incompatible,
                ..
            }
        ));
        worker.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn oversized_frame_is_rejected_before_allocating_its_payload() {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        client.write_u32(u32::MAX).await.unwrap();
        assert!(receive(&mut server, MAX_FRAME)
            .await
            .unwrap_err()
            .to_string()
            .contains("exceeds"));
    }
}
