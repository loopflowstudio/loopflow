//! Idle control connections to other Machines. Each checkout is exclusive;
//! concurrent reads can open their own pipes.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use loopflow_execution::engine::ErrorCode;
use loopflow_execution::machine::client::{Connection, RequestError, Response};
use loopflow_execution::machine::wire::Sender;
use loopflow_model::durable::{Machine, MachineId};

use super::service::lock;
use crate::lf::commands::read::Read;

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
            connection.request_as(read, Some(sender.clone()), None, ErrorCode::Unavailable)
        };
        let reused = warm.is_some();
        let mut connection = match warm {
            Some(warm) => warm.connection,
            None => Connection::open_peer(machine, None)?,
        };
        let mut answer = send(&mut connection);
        if reused && matches!(answer, Err(RequestError::Disconnected(_))) {
            drop(connection);
            connection = Connection::open_peer(machine, None)?;
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
