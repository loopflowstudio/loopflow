//! Reserve before materializing; release only after delivery or worker completion.
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::Result;
use tokio::sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore};

use super::wire::{ServerMessage, MAX_MESSAGE};

pub(super) const MAX_OPEN: usize = 256;
pub(super) const MAX_WRITES: usize = 16;
pub(super) const MAX_RETAINED: usize = 64 * 1024 * 1024;
const PROCESS_RETAINED: usize = 256 * 1024 * 1024;
const SHARED_RETAINED: usize = PROCESS_RETAINED / 2;
static PROCESS: OnceLock<Arc<Semaphore>> = OnceLock::new();
static SHARED: OnceLock<Arc<Semaphore>> = OnceLock::new();

fn process_budget() -> Arc<Semaphore> {
    PROCESS
        .get_or_init(|| Arc::new(Semaphore::new(PROCESS_RETAINED)))
        .clone()
}

/// A shared reader reserves the same process headroom as a client response.
#[derive(Debug)]
pub(super) struct Materializing(OwnedSemaphorePermit);

/// Charge the latest shared reading until its last delivery releases it.
/// At most half the process budget can be retained between reads, leaving
/// room for refreshes, errors and delivery even when the cache is full.
#[derive(Debug)]
pub(super) struct Retained {
    _process: OwnedSemaphorePermit,
    _shared: OwnedSemaphorePermit,
}

impl Materializing {
    pub async fn reserve() -> Result<Self> {
        Ok(Self(
            process_budget()
                .acquire_many_owned(MAX_MESSAGE as u32)
                .await?,
        ))
    }

    pub fn retain(mut self, bytes: usize) -> Result<Retained> {
        anyhow::ensure!(bytes <= MAX_MESSAGE, "shared reading exceeds max_message");
        let shared = SHARED
            .get_or_init(|| Arc::new(Semaphore::new(SHARED_RETAINED)))
            .clone()
            .try_acquire_many_owned(bytes as u32)
            .map_err(|_| {
                crate::engine::Error::new(
                    crate::engine::ErrorCode::Busy,
                    "shared subscription retained capacity reached; retry shortly",
                )
            })?;
        let release = self.0.num_permits() - bytes;
        drop(self.0.split(release));
        Ok(Retained {
            _process: self.0,
            _shared: shared,
        })
    }
}

#[cfg(test)]
impl Retained {
    pub fn empty() -> Self {
        let zero = || {
            Arc::new(Semaphore::new(0))
                .try_acquire_many_owned(0)
                .unwrap()
        };
        Self {
            _process: zero(),
            _shared: zero(),
        }
    }
}

#[derive(Clone)]
pub(super) struct Outbox {
    pub sender: mpsc::Sender<Frame>,
    local: Arc<Semaphore>,
    process: Arc<Semaphore>,
}

pub(super) struct Frame {
    pub line: String,
    pub delivered: oneshot::Sender<()>,
    pub id: String,
    pub task: Option<tokio::task::Id>,
    pub complete: bool,
    pub response: bool,
    // The reservation survives queueing and the socket's flush.
    pub reservation: Arc<Reservation>,
}

type RequestSlots = (OwnedSemaphorePermit, Option<OwnedSemaphorePermit>);

pub(super) struct Reservation {
    slots: Mutex<Option<RequestSlots>>,
    permits: Mutex<(OwnedSemaphorePermit, OwnedSemaphorePermit)>,
}

impl Reservation {
    pub fn hold(&self, slots: RequestSlots) {
        *self.slots.lock().expect("reservation lock poisoned") = Some(slots);
    }
    fn shrink(&self, bytes: usize) {
        let mut permits = self.permits.lock().expect("reservation lock poisoned");
        let release = permits.0.num_permits() - bytes;
        drop(permits.0.split(release));
        drop(permits.1.split(release));
    }
}

impl Outbox {
    pub fn new() -> (Self, mpsc::Receiver<Frame>) {
        Self::with_process(process_budget())
    }

    fn with_process(process: Arc<Semaphore>) -> (Self, mpsc::Receiver<Frame>) {
        let (sender, receiver) = mpsc::channel(MAX_OPEN);
        (
            Self {
                sender,
                local: Arc::new(Semaphore::new(MAX_RETAINED)),
                process,
            },
            receiver,
        )
    }

    pub fn retained(&self) -> usize {
        MAX_RETAINED - self.local.available_permits()
    }

    pub fn reserve(&self) -> Option<Arc<Reservation>> {
        let local = self
            .local
            .clone()
            .try_acquire_many_owned(MAX_MESSAGE as u32)
            .ok()?;
        let process = self
            .process
            .clone()
            .try_acquire_many_owned(MAX_MESSAGE as u32)
            .ok()?;
        Some(Arc::new(Reservation {
            slots: Mutex::new(None),
            permits: Mutex::new((local, process)),
        }))
    }

    pub async fn wait(&self) -> Result<Arc<Reservation>> {
        let local = self
            .local
            .clone()
            .acquire_many_owned(MAX_MESSAGE as u32)
            .await?;
        let process = self
            .process
            .clone()
            .acquire_many_owned(MAX_MESSAGE as u32)
            .await?;
        Ok(Arc::new(Reservation {
            slots: Mutex::new(None),
            permits: Mutex::new((local, process)),
        }))
    }

    pub async fn deliver(
        &self,
        message: ServerMessage,
        reservation: Arc<Reservation>,
    ) -> Result<()> {
        let (id, complete, response) = match &message {
            ServerMessage::Response { id, .. } => (id.clone(), false, true),
            ServerMessage::Reading { id, .. } => (id.clone(), false, false),
            ServerMessage::Ended { id } => (id.clone(), true, false),
            ServerMessage::Error { id: Some(id), .. } => (id.clone(), true, false),
            _ => unreachable!("only request frames enter an outbox"),
        };
        let mut bytes = Limited(Vec::new());
        serde_json::to_writer(&mut bytes, &message)?;
        // Free the materialized value before releasing its producer headroom.
        drop(message);
        reservation.shrink(bytes.0.len());
        let line = String::from_utf8(bytes.0).expect("JSON is UTF-8");
        let (delivered, taken) = oneshot::channel();
        self.sender
            .send(Frame {
                line,
                delivered,
                reservation,
                id,
                complete,
                response,
                task: tokio::task::try_id(),
            })
            .await
            .map_err(|_| anyhow::anyhow!("connection closed"))?;
        // A subscription cannot materialize another reading until this one leaves.
        taken
            .await
            .map_err(|_| anyhow::anyhow!("connection closed"))
    }
}

struct Limited(Vec<u8>);

impl std::io::Write for Limited {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_MESSAGE - self.0.len() {
            return Err(std::io::Error::other("server message exceeds max_message"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::Semaphore;

    use super::{Materializing, Outbox, MAX_RETAINED, PROCESS_RETAINED};
    use crate::server::wire::{ServerMessage, MAX_MESSAGE};

    #[test]
    fn shared_reading_retains_its_bytes_until_the_last_consumer_releases_it() {
        let process = Arc::new(Semaphore::new(MAX_MESSAGE));
        let reading = Materializing(
            process
                .clone()
                .try_acquire_many_owned(MAX_MESSAGE as u32)
                .unwrap(),
        );
        assert_eq!(process.available_permits(), 0);
        let producer = Arc::new(reading.retain(1024).unwrap());
        let consumer = producer.clone();
        assert_eq!(process.available_permits(), MAX_MESSAGE - 1024);
        drop(producer);
        assert_eq!(process.available_permits(), MAX_MESSAGE - 1024);
        drop(consumer);
        assert_eq!(process.available_permits(), MAX_MESSAGE);
        let oversized = Materializing(
            process
                .clone()
                .try_acquire_many_owned(MAX_MESSAGE as u32)
                .unwrap(),
        );
        assert!(oversized.retain(MAX_MESSAGE + 1).is_err());
        assert_eq!(process.available_permits(), MAX_MESSAGE);
    }

    #[tokio::test]
    async fn producers_and_queued_frames_share_connection_and_process_budgets() {
        let process = Arc::new(Semaphore::new(PROCESS_RETAINED));
        let mut clients = Vec::new();
        let mut held = Vec::new();
        for _ in 0..PROCESS_RETAINED / MAX_RETAINED {
            let (outbox, receiver) = Outbox::with_process(process.clone());
            held.push(outbox.reserve().unwrap());
            held.push(outbox.reserve().unwrap());
            assert!(outbox.reserve().is_none());
            assert_eq!(outbox.retained(), MAX_RETAINED);
            clients.push((outbox, receiver));
        }
        let (other, _) = Outbox::with_process(process.clone());
        assert!(
            other.reserve().is_none(),
            "another connection shares the process cap"
        );
        assert_eq!(
            other.retained(),
            0,
            "failed admission releases its local reservation"
        );
        let (outbox, receiver) = &mut clients[0];
        let reservation = held.remove(0);
        let producer = outbox.clone();
        let delivering = tokio::spawn(async move {
            producer
                .deliver(
                    ServerMessage::Response {
                        id: "large".into(),
                        trace_id: crate::id::TraceId::default(),
                        result: serde_json::Value::String("x".repeat(MAX_MESSAGE / 2)),
                    },
                    reservation,
                )
                .await
                .unwrap();
        });
        let frame = receiver.recv().await.unwrap();
        assert_eq!(outbox.retained(), MAX_MESSAGE + frame.line.len());
        frame.delivered.send(()).unwrap();
        drop(frame.reservation);
        delivering.await.unwrap();
        assert_eq!(outbox.retained(), MAX_MESSAGE);
        assert!(other.reserve().is_some(), "delivery frees process capacity");
        drop(held);
        assert_eq!(outbox.retained(), 0);
    }

    #[tokio::test]
    async fn a_disconnected_waiter_cannot_release_a_running_workers_reservation() {
        let (outbox, receiver) = Outbox::with_process(Arc::new(Semaphore::new(PROCESS_RETAINED)));
        let open = Arc::new(Semaphore::new(1));
        let writes = Arc::new(Semaphore::new(1));
        let waiting = outbox.reserve().unwrap();
        waiting.hold((
            open.clone().try_acquire_owned().unwrap(),
            Some(writes.clone().try_acquire_owned().unwrap()),
        ));
        let worker = waiting.clone();
        drop(receiver);
        drop(waiting);
        assert_eq!(outbox.retained(), MAX_MESSAGE);
        assert_eq!(open.available_permits(), 0);
        assert_eq!(writes.available_permits(), 0);
        drop(worker);
        assert_eq!(outbox.retained(), 0);
        assert_eq!(open.available_permits(), 1);
        assert_eq!(writes.available_permits(), 1);
    }
}
