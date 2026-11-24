//! Control connections to other Machines. Reads and saves borrow an idle pipe.
//! A subscription owns its pipe until it ends or the Task moves.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures_util::StreamExt;
use loopflow_execution::engine::ErrorCode;
use loopflow_execution::machine::client::{self, Connection, RequestError, Response};
use loopflow_execution::machine::ssh::Pipe;
use loopflow_execution::machine::wire::Sender;
use loopflow_model::durable::{Machine, MachineId};
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, ChildStdout};
use tokio_util::codec::{FramedRead, LinesCodec};

use super::service::lock;
use super::wire::{
    ClientMessage, Hello, ServerMessage, MACHINE_CLIENT, MAX_MESSAGE, MIN_REVISION, REVISION,
};
use crate::lf::commands::{read::Read, write::Write};

const MAX_IDLE: usize = 4;
const IDLE_TIME: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
pub(super) struct PeerPool {
    peers: Mutex<HashMap<MachineId, Peer>>,
}

#[derive(Debug)]
struct Peer {
    route: String,
    idle: Arc<Mutex<Vec<Idle>>>,
}

#[derive(Debug)]
struct Idle {
    since: Instant,
    connection: Connection,
}

impl PeerPool {
    pub fn query(&self, machine: &Machine, sender: Sender, read: &Read) -> Result<Response> {
        self.request(machine, sender, read, None, ErrorCode::Unavailable)
    }

    pub fn save(
        &self,
        machine: &Machine,
        sender: Sender,
        change: &Write,
        request_id: Option<String>,
    ) -> Result<Response> {
        self.request(
            machine,
            sender,
            change,
            request_id,
            ErrorCode::OutcomeUnknown,
        )
    }

    fn request(
        &self,
        machine: &Machine,
        sender: Sender,
        operation: &impl serde::Serialize,
        request_id: Option<String>,
        disconnected: ErrorCode,
    ) -> Result<Response> {
        let mut peers = lock(&self.peers);
        let old = if peers
            .get(&machine.id)
            .is_some_and(|peer| peer.route != machine.route)
        {
            peers.remove(&machine.id)
        } else {
            None
        };
        let peer = peers.entry(machine.id.clone()).or_insert_with(|| Peer {
            route: machine.route.clone(),
            idle: Arc::default(),
        });
        let idle = peer.idle.clone();
        drop(peers);
        drop(old);

        let mut connections = lock(&idle);
        let expired = take_expired(&mut connections);
        let warm = connections.pop();
        drop(connections);
        drop(expired);

        let send = |connection: &mut Connection| {
            connection.request_as(
                operation,
                Some(sender.clone()),
                request_id.clone(),
                disconnected,
            )
        };
        let reused = warm.is_some();
        let mut connection = match warm {
            Some(warm) => warm.connection,
            None => Connection::open_peer(machine, None)?,
        };
        let mut answer = send(&mut connection);
        let retry = request_id.is_some() || (reused && disconnected == ErrorCode::Unavailable);
        if retry && matches!(answer, Err(RequestError::Disconnected(_))) {
            drop(connection);
            connection = match Connection::open_peer(machine, None) {
                Ok(fresh) => fresh,
                Err(_) if disconnected == ErrorCode::OutcomeUnknown => {
                    return answer.map_err(RequestError::into_error)
                }
                Err(error) => return Err(error),
            };
            answer = send(&mut connection);
        }
        if matches!(answer, Ok(_) | Err(RequestError::Refused(_))) {
            let mut connections = lock(&idle);
            if connections.len() < MAX_IDLE {
                connections.push(Idle {
                    since: Instant::now(),
                    connection,
                });
            }
        }
        answer.map_err(RequestError::into_error)
    }

    fn prune(&self) {
        let mut expired = Vec::new();
        lock(&self.peers).retain(|_, peer| {
            let mut idle = lock(&peer.idle);
            expired.extend(take_expired(&mut idle));
            !idle.is_empty() || Arc::strong_count(&peer.idle) > 1
        });
        // Dropping a pipe reaps its SSH process, outside both locks.
        drop(expired);
    }

    pub async fn expiring(self: Arc<Self>) {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let pool = self.clone();
            tokio::task::spawn_blocking(move || pool.prune())
                .await
                .expect("peer cleanup finished");
        }
    }
}

fn take_expired(idle: &mut Vec<Idle>) -> Vec<Idle> {
    let expired = idle.partition_point(|entry| entry.since.elapsed() >= IDLE_TIME);
    idle.drain(..expired).collect()
}

/// A dedicated subscription pipe. Dropping it cancels a pending read or handshake.
pub(super) struct PeerSubscription {
    child: Child,
    output: FramedRead<ChildStdout, LinesCodec>,
}

impl PeerSubscription {
    pub async fn open(
        machine: &Machine,
        sender: Sender,
        read: Read,
        select: Option<Vec<String>>,
    ) -> Result<Self> {
        let connect = async {
            let mut child = tokio::process::Command::from(Pipe::command(machine)?)
                .kill_on_drop(true)
                .spawn()?;
            let output = FramedRead::new(
                child.stdout.take().expect("piped SSH stdout"),
                LinesCodec::new_with_max_length(MAX_MESSAGE),
            );
            let mut peer = Self { child, output };
            peer.send(ClientMessage::Hello(Hello {
                revision: REVISION,
                min_revision: MIN_REVISION,
                client: client::identity(MACHINE_CLIENT, "lf"),
            }))
            .await?;
            anyhow::ensure!(
                matches!(peer.receive().await?, ServerMessage::Welcome(_)),
                "expected peer welcome"
            );
            anyhow::Ok(peer)
        };
        let mut peer = tokio::time::timeout(client::HANDSHAKE, connect).await??;
        peer.send(ClientMessage::Subscribe {
            id: "task".into(),
            operation: read,
            select,
            resume: None,
            sender: Some(sender),
        })
        .await?;
        anyhow::ensure!(
            matches!(
                tokio::time::timeout(client::ANSWER, peer.receive()).await??,
                ServerMessage::Response { .. }
            ),
            "expected subscription acknowledgement"
        );
        Ok(peer)
    }

    async fn send(&mut self, message: ClientMessage) -> Result<()> {
        let mut line = serde_json::to_vec(&message)?;
        line.push(b'\n');
        anyhow::ensure!(
            line.len() <= MAX_MESSAGE,
            "subscription exceeds peer message limit"
        );
        self.child
            .stdin
            .as_mut()
            .expect("piped SSH stdin")
            .write_all(&line)
            .await?;
        Ok(())
    }

    pub async fn checkpoint(&mut self, checkpoint: u64) -> Result<()> {
        self.send(ClientMessage::Checkpoint {
            id: "task".into(),
            checkpoint,
        })
        .await
    }

    pub async fn receive(&mut self) -> Result<ServerMessage> {
        let line = self
            .output
            .next()
            .await
            .context("peer closed the subscription")??;
        match serde_json::from_str(&line)? {
            ServerMessage::Error {
                code,
                message,
                data,
                ..
            } => Err(loopflow_execution::engine::Error {
                code,
                message,
                data,
            }
            .into()),
            message => Ok(message),
        }
    }
}
