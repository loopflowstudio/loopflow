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
    ErrorCode, Operation, Reply, Request, Response, MAX_CONNECTIONS, MAX_FRAME, VERSION,
};
use crate::lf::commands::{read::Read, write::Write};

pub(super) async fn run() -> Result<()> {
    let path = super::socket_path()?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(path.with_extension("lock"))?;
    lock.try_lock_exclusive()
        .context("Loopflow server is already running")?;
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
    tracing::info!(%instance, socket = %path.display(), "Loopflow server ready");
    loop {
        let (socket, _) = listener.accept().await?;
        // Unix admission is deliberately unavailable to remote proxies.
        // SAFETY: geteuid has no preconditions.
        if socket.peer_cred()?.uid() != unsafe { libc::geteuid() } {
            continue;
        }
        let Ok(permit) = slots.clone().try_acquire_owned() else {
            tracing::warn!(%instance, "Server connection capacity reached");
            continue;
        };
        tracing::debug!(
            active = MAX_CONNECTIONS - slots.available_permits(),
            "Server connection admitted"
        );
        let instance = instance.clone();
        let executable_sha256 = executable_sha256.clone();
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(error) = connection(socket, &instance, &executable_sha256).await {
                tracing::warn!(%instance, %error, "Server connection ended");
            }
        });
    }
}

async fn connection(mut socket: UnixStream, instance: &str, executable_sha256: &str) -> Result<()> {
    let bytes = tokio::time::timeout(Duration::from_secs(5), receive(&mut socket)).await??;
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
                    "Server protocol {VERSION} required; update the client or service"
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
    tracing::debug!(request = %id, %instance, elapsed_ms = started.elapsed().as_millis(), ok = result.is_ok(), "Server request ended");
    if let Err(error) = result {
        tracing::warn!(request = %id, %instance, %error, "Server request failed");
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
    socket: &mut UnixStream,
    id: &str,
    instance: &str,
    executable_sha256: &str,
    operation: Operation,
) -> Result<()> {
    let reply = match operation {
        Operation::Status => Reply::Status {
            version: VERSION,
            executable_sha256: executable_sha256.into(),
        },
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
                    "reads".into(),
                    "subscriptions".into(),
                    "commands".into(),
                    "configuration".into(),
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
        Operation::Read { query } => Reply::Read {
            output: tokio::task::spawn_blocking(move || {
                crate::journal::request::isolate(|| query.read())
            })
            .await??,
        },
        Operation::Subscribe { query, generation } => {
            return subscribe(socket, id, instance, query, generation).await;
        }
        Operation::Configuration { request } => {
            let mut target = serde_json::to_value(&request)?;
            target
                .as_object_mut()
                .expect("configuration request is an object")
                .remove("value");
            execute_command(target, id, instance, move |output, _errors| {
                use std::io::Write;
                writeln!(output, "{}", crate::config::api::run(request)?)?;
                Ok(())
            })
            .await
            .unwrap_or_else(command_outcome_unknown)
        }
        Operation::Command { change } => command(change, id, instance)
            .await
            .unwrap_or_else(command_outcome_unknown),
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
                tracing::debug!(%error, "Server owner bridge disconnected");
            }
            return Ok(());
        }
    };
    respond(socket, id, instance, reply).await
}

async fn subscribe(
    socket: &mut UnixStream,
    id: &str,
    instance: &str,
    query: Read,
    generation: u64,
) -> Result<()> {
    let notify = Arc::new(Notify::new());
    let wake = notify.clone();
    let subscription = Arc::new(
        tokio::task::spawn_blocking(move || {
            crate::journal::request::isolate(|| query.subscribe(move || wake.notify_one()))
        })
        .await??,
    );
    // Keep the in-flight read future across wakes; cancellation cannot discard a
    // partial frame then interpret its payload as the next length prefix.
    let (mut reader, mut writer) = socket.split();
    let mut input = Box::pin(receive(&mut reader));
    loop {
        tokio::select! {
            bytes = &mut input => {
                let bytes = bytes?;
                // The shared watch owns checkpoint parsing and its final error
                // reading, including argument validation and request limits.
                subscription.request(&bytes);
                drop(input);
                input = Box::pin(receive(&mut reader));
            }
            () = notify.notified() => {
                let pull = subscription.clone();
                let frame = tokio::task::spawn_blocking(move || crate::journal::request::isolate(|| pull.pull())).await?;
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

fn command_outcome_unknown(error: anyhow::Error) -> Reply {
    Reply::Error {
        code: ErrorCode::OutcomeUnknown,
        message: format!("{error}; inspect the affected work before retrying"),
    }
}

async fn command(change: Write, id: &str, instance: &str) -> Result<Reply> {
    use clap::Parser;
    let mut target = serde_json::to_value(&change)?;
    if let Some(fields) = target.as_object_mut() {
        for field in ["content", "notes", "plan", "layout", "slots", "expression"] {
            fields.remove(field);
        }
    }
    execute_command(target, id, instance, move |output, errors| {
        let args: Vec<_> = std::iter::once("lf".to_owned())
            .chain(change.argv())
            .collect();
        let cli = match crate::lf::Cli::try_parse_from(&args).and_then(crate::lf::Cli::checked) {
            Ok(cli) => cli,
            Err(error) => {
                use std::io::Write;
                let stream = if error.use_stderr() {
                    &mut *errors
                } else {
                    &mut *output
                };
                write!(stream, "{error}")?;
                return Err(crate::process::CommandExit(
                    u8::try_from(error.exit_code()).expect("Clap exit code fits a byte"),
                )
                .into());
            }
        };
        anyhow::ensure!(
            crate::lf::commands::mutation::accepts(&cli),
            "unsupported mutation"
        );
        let mut input = change.input().unwrap_or_default().as_bytes();
        crate::lf::commands::mutation::run(
            &cli,
            &args,
            change.directory(),
            &mut crate::lf::commands::mutation::CommandIo {
                input: &mut input,
                output,
                error: errors,
            },
        )
    })
    .await
}

async fn execute_command(
    target: serde_json::Value,
    id: &str,
    instance: &str,
    execute: impl FnOnce(&mut Vec<u8>, &mut Vec<u8>) -> Result<()> + Send + 'static,
) -> Result<Reply> {
    let id = id.to_owned();
    let instance = instance.to_owned();
    // Once admitted, disconnection releases the viewer; effects finish without replay.
    tokio::task::spawn_blocking(move || {
        let trace_id = crate::id::TraceId::default();
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let result =
            crate::journal::request::run(&trace_id, &target, || execute(&mut output, &mut errors));
        let code = i32::from(crate::journal::command_exit_code(&result));
        if let Err(error) = result {
            use std::io::Write;
            if !error.is::<crate::process::CommandExit>() {
                writeln!(errors, "Error: {error:?}")?;
            }
        }
        tracing::info!(request = %id, %instance, code, %trace_id, "Server command finished");
        anyhow::ensure!(
            output.len() < MAX_FRAME / 2 && errors.len() < MAX_FRAME / 2,
            "command output exceeded its limit"
        );
        Ok(Reply::Command {
            trace_id,
            output: String::from_utf8(output)?,
            stderr: String::from_utf8(errors)?,
            code,
        })
    })
    .await?
}

pub(super) async fn receive(reader: &mut (impl AsyncRead + Unpin)) -> Result<Vec<u8>> {
    let length = reader.read_u32().await? as usize;
    anyhow::ensure!(
        length <= MAX_FRAME,
        "Server frame exceeds {MAX_FRAME} bytes"
    );
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
        "Server reading exceeds frame limit"
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
    use crate::server::wire::{ErrorCode, Operation, Reply, Request, Response, VERSION};
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
        let reply: Response = serde_json::from_slice(&receive(&mut client).await.unwrap()).unwrap();
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
        assert!(receive(&mut server)
            .await
            .unwrap_err()
            .to_string()
            .contains("exceeds"));
    }
}
