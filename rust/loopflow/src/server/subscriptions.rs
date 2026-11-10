//! One production Subscription per exact demand, with independent delivery.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

use anyhow::Result;
use serde_json::Value;
use tokio::sync::{broadcast, watch, Notify};
use tokio::task::AbortHandle;

use super::outbox::{Materializing, Outbox, Retained};
use super::service::{deliver, lock, reading_checkpoint, unavailable};
use super::wire::{ServerMessage, SubscriptionEvent, MAX_MESSAGE};
use crate::engine::Error;
use crate::lf::commands::{read::Read, subscribe::Subscription};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    read: Vec<u8>,
    database: PathBuf,
    directory: PathBuf,
}

#[derive(Debug, Default)]
pub(super) struct Subscriptions(Mutex<HashMap<Key, Arc<Demand>>>);

#[derive(Debug, Default)]
struct Requested {
    generation: u64,
    closed: bool,
}

#[derive(Debug)]
struct Demand {
    requested: Arc<Mutex<Requested>>,
    wake: Arc<Notify>,
    latest: watch::Receiver<Latest>,
    events: broadcast::Sender<Arc<Events>>,
    cancelled: Arc<AtomicBool>,
    task: AbortHandle,
    consumers: AtomicUsize,
}

#[derive(Debug, Clone, Default)]
struct Latest {
    ready: bool,
    reading: Option<Arc<Reading>>,
    events: u64,
    ended: Option<Result<(), Arc<Error>>>,
}

#[derive(Debug)]
struct Events {
    items: Vec<SubscriptionEvent>,
    _retained: Arc<Retained>,
}

#[derive(Debug)]
struct Reading {
    generation: u64,
    frame: Value,
    identity: [u8; 32],
    _retained: Arc<Retained>,
}

#[derive(Debug, Clone, Copy)]
struct Pending {
    generation: u64,
    checkpoint: Option<u64>,
}

/// Held by connection admission, so cancel releases demand before ID reuse.
#[derive(Debug)]
pub(super) struct Member {
    registry: Arc<Subscriptions>,
    key: Key,
    demand: Arc<Demand>,
    pending: Arc<Mutex<Pending>>,
}

#[derive(Debug)]
pub(super) struct Delivery {
    latest: watch::Receiver<Latest>,
    pending: Arc<Mutex<Pending>>,
    events: broadcast::Receiver<Arc<Events>>,
    delivered_events: u64,
}

impl Subscriptions {
    pub(super) fn join(
        self: &Arc<Self>,
        id: &str,
        query: Read,
        select: Option<Vec<String>>,
    ) -> Result<(Member, Delivery)> {
        // Reads are isolated from request attribution. The service's process
        // environment is fixed; paths and all wire arguments distinguish demand.
        let key = Key {
            read: serde_json::to_vec(&(&query, &select))?,
            database: crate::store::database_path_from_env()?,
            directory: std::env::current_dir()?,
        };
        let mut entries = lock(&self.0);
        let demand = entries
            .entry(key.clone())
            .or_insert_with(|| {
                let requested = Arc::new(Mutex::new(Requested::default()));
                let wake = Arc::new(Notify::new());
                let cancelled = Arc::new(AtomicBool::new(false));
                let (latest, receiver) = watch::channel(Latest::default());
                let (events, _) = broadcast::channel(16);
                let task = tokio::spawn(
                    Producer {
                        registry: Arc::downgrade(self),
                        key: key.clone(),
                        id: id.to_owned(),
                        requested: requested.clone(),
                        wake: wake.clone(),
                        cancelled: cancelled.clone(),
                        latest,
                        events: events.clone(),
                    }
                    .run(query, select),
                );
                Arc::new(Demand {
                    requested,
                    wake,
                    latest: receiver,
                    events,
                    cancelled,
                    task: task.abort_handle(),
                    consumers: AtomicUsize::new(0),
                })
            })
            .clone();
        demand.consumers.fetch_add(1, Ordering::Relaxed);
        let mut requested = lock(&demand.requested);
        requested.generation += 1;
        let pending = Arc::new(Mutex::new(Pending {
            generation: requested.generation,
            checkpoint: None,
        }));
        drop(requested);
        demand.wake.notify_one();
        let delivery = {
            // Publication holds this watch lock while enqueueing occurrences.
            // A joining consumer starts at the current edge, without replay.
            let latest = demand.latest.borrow();
            Delivery {
                latest: demand.latest.clone(),
                pending: pending.clone(),
                events: demand.events.subscribe(),
                delivered_events: latest.events,
            }
        };
        Ok((
            Member {
                registry: self.clone(),
                key,
                demand,
                pending,
            },
            delivery,
        ))
    }

    fn retire(&self, key: &Key, cancelled: &Arc<AtomicBool>) {
        let mut entries = lock(&self.0);
        if entries
            .get(key)
            .is_some_and(|entry| Arc::ptr_eq(&entry.cancelled, cancelled))
        {
            entries.remove(key);
        }
    }
}

impl Member {
    pub(super) fn checkpoint(&self, checkpoint: u64) -> bool {
        let mut requested = lock(&self.demand.requested);
        if requested.closed {
            return false;
        }
        requested.generation += 1;
        *lock(&self.pending) = Pending {
            generation: requested.generation,
            checkpoint: Some(checkpoint),
        };
        self.demand.wake.notify_one();
        true
    }
}

impl Drop for Member {
    fn drop(&mut self) {
        let mut entries = lock(&self.registry.0);
        if self.demand.consumers.fetch_sub(1, Ordering::Relaxed) == 1 {
            self.demand.cancelled.store(true, Ordering::Release);
            self.demand.task.abort();
            if entries
                .get(&self.key)
                .is_some_and(|entry| Arc::ptr_eq(entry, &self.demand))
            {
                entries.remove(&self.key);
            }
        }
    }
}

impl Delivery {
    pub(super) async fn run(
        mut self,
        id: String,
        outgoing: Outbox,
        scope: Vec<u8>,
        resume: Option<String>,
    ) {
        #[cfg(test)]
        if let Some(mut pause) =
            super::service::subscription_measurement::take_pause(&id, "terminal-delivery")
        {
            self.latest
                .wait_for(|latest| latest.ended.is_some())
                .await
                .unwrap();
            tokio::task::spawn_blocking(move || pause.wait())
                .await
                .unwrap();
        }
        let result = self.deliver(&id, &outgoing, &scope, resume).await;
        if let Err(failure) = result {
            let _ = deliver(
                &outgoing,
                ServerMessage::error(Some(id), unavailable(failure)),
            )
            .await;
        }
    }

    async fn deliver(
        &mut self,
        id: &str,
        outgoing: &Outbox,
        scope: &[u8],
        mut resume: Option<String>,
    ) -> Result<()> {
        let mut acknowledged = false;
        let mut sent = None;
        let mut answered = 0;
        loop {
            // Release the watch borrow before waiting on this consumer's socket.
            let latest = self.latest.borrow_and_update().clone();
            if latest.ready && !acknowledged {
                deliver(
                    outgoing,
                    ServerMessage::Response {
                        id: id.into(),
                        trace_id: crate::id::TraceId::default(),
                        result: Value::Null,
                    },
                )
                .await?;
                acknowledged = true;
            }
            let pending = *lock(&self.pending);
            if let Some(reading) = latest.reading {
                let fresh = reading.generation >= pending.generation;
                let unseen_failure = latest.ended.as_ref().is_some_and(Result::is_err)
                    && reading.frame["unavailable"].is_string()
                    && sent != Some(reading.identity);
                if (fresh && (answered < pending.generation || sent != Some(reading.identity)))
                    || unseen_failure
                {
                    let reservation = outgoing.wait().await?;
                    let mut frame = reading.frame.clone();
                    frame["answers"] = if fresh {
                        pending.checkpoint.map(Value::from).unwrap_or(Value::Null)
                    } else {
                        Value::Null
                    };
                    let checkpoint = reading_checkpoint(scope, &frame)?;
                    let unchanged = resume.take().as_ref() == Some(&checkpoint)
                        && frame["answers"].is_null()
                        && frame["unavailable"].is_null();
                    if !unchanged {
                        outgoing
                            .deliver(
                                ServerMessage::Reading {
                                    id: id.into(),
                                    checkpoint,
                                    frame,
                                },
                                reservation,
                            )
                            .await?;
                    }
                    answered = pending.generation;
                    sent = Some(reading.identity);
                }
            }
            // A join during an admitted read still needs its own fresh baseline
            // before it can consume occurrences from the shared reader.
            if sent.is_none() && latest.ended.is_none() {
                self.latest.changed().await?;
                continue;
            }
            while self.delivered_events < latest.events {
                let events = self.events.try_recv().map_err(|_| {
                    anyhow::anyhow!(
                        "subscription fell behind its events; subscribe again for current state"
                    )
                })?;
                let reservation = outgoing.wait().await?;
                reservation.batch();
                for event in &events.items {
                    outgoing
                        .deliver(
                            ServerMessage::Event {
                                id: id.into(),
                                seq: 0,
                                event: event.clone(),
                            },
                            reservation.clone(),
                        )
                        .await?;
                }
                self.delivered_events += 1;
            }
            if let Some(ended) = latest.ended {
                return match ended {
                    Ok(()) => deliver(outgoing, ServerMessage::Ended { id: id.into() }).await,
                    Err(error) => Err(anyhow::anyhow!((*error).clone())),
                };
            }
            self.latest.changed().await?;
        }
    }
}

/// A queued job must be aborted, not detached, when its last consumer leaves.
#[derive(Debug)]
struct Queued(AbortHandle);
impl Drop for Queued {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Runtime and SQLite destruction must finish off the async workers, including
/// when an admitted setup finishes after cancellation.
#[derive(Debug)]
struct Held(Option<Subscription>);
impl std::ops::Deref for Held {
    type Target = Subscription;
    fn deref(&self) -> &Subscription {
        self.0
            .as_ref()
            .expect("subscription held until final release")
    }
}
impl Drop for Held {
    fn drop(&mut self) {
        let mut release = Release(self.0.take());
        tokio::task::spawn_blocking(move || drop(release.0.take()));
    }
}

// A shutting-down runtime rejects blocking jobs by dropping their closures on
// the caller's thread. Even that final release must run outside an async worker.
// Join only on this shutdown path, so service teardown also finishes destruction.
struct Release(Option<Subscription>);
impl Drop for Release {
    fn drop(&mut self) {
        if let Some(subscription) = self.0.take() {
            std::thread::spawn(move || drop(subscription))
                .join()
                .expect("subscription cleanup thread does not panic");
        }
    }
}

#[derive(Debug)]
struct Producer {
    registry: Weak<Subscriptions>,
    key: Key,
    id: String,
    requested: Arc<Mutex<Requested>>,
    wake: Arc<Notify>,
    cancelled: Arc<AtomicBool>,
    latest: watch::Sender<Latest>,
    events: broadcast::Sender<Arc<Events>>,
}

impl Producer {
    async fn run(self, query: Read, select: Option<Vec<String>>) {
        let result = self.production(query, select).await;
        if let Some(registry) = self.registry.upgrade() {
            registry.retire(&self.key, &self.cancelled);
        }
        lock(&self.requested).closed = true;
        self.latest.send_modify(|latest| {
            latest.ended = Some(result.map_err(|error| Arc::new(unavailable(error))));
        });
    }

    // Join and terminal retirement hold the registry before the generation.
    // A request admitted before this cut gets a trailing read; after it, a join
    // gets a new Subscription and an old checkpoint is no longer admitted.
    fn finish(&self, generation: u64) -> bool {
        let registry = self.registry.upgrade();
        let mut entries = registry.as_ref().map(|registry| lock(&registry.0));
        let mut requested = lock(&self.requested);
        if requested.generation != generation {
            return false;
        }
        requested.closed = true;
        if let Some(entries) = &mut entries {
            if entries
                .get(&self.key)
                .is_some_and(|entry| Arc::ptr_eq(&entry.cancelled, &self.cancelled))
            {
                entries.remove(&self.key);
            }
        }
        true
    }

    async fn production(&self, query: Read, select: Option<Vec<String>>) -> Result<()> {
        let Self {
            id: _id,
            requested,
            wake,
            cancelled,
            latest,
            ..
        } = self;
        let ends = query.clone();
        #[cfg(test)]
        let mut pause = super::service::subscription_measurement::take_pause(_id, "setup");
        let notify = wake.clone();
        let cancel = cancelled.clone();
        let headroom = Materializing::reserve().await?;
        let setup = tokio::task::spawn_blocking(move || {
            let _headroom = headroom;
            if cancel.load(Ordering::Acquire) {
                return Ok(None);
            }
            #[cfg(test)]
            if let Some(pause) = &mut pause {
                pause.wait();
            }
            crate::journal::request::isolate(|| {
                query.subscribe(select, move || notify.notify_one())
            })
            .map(|subscription| Some(Arc::new(Held(Some(subscription)))))
        });
        let queued = Queued(setup.abort_handle());
        let Some(subscription) = setup.await?? else {
            return Ok(());
        };
        drop(queued);
        latest.send_modify(|latest| latest.ready = true);
        let mut applied = 0;
        loop {
            if lock(requested).generation <= applied {
                wake.notified().await;
            }
            if cancelled.load(Ordering::Acquire) {
                return Ok(());
            }
            let generation = lock(requested).generation;
            let force = generation > applied;
            let pull = subscription.clone();
            let cancel = cancelled.clone();
            #[cfg(test)]
            let mut pause = super::service::subscription_measurement::take_pause(_id, "pull");
            let headroom = Materializing::reserve().await?;
            let job = tokio::task::spawn_blocking(move || {
                if cancel.load(Ordering::Acquire) {
                    return (Default::default(), headroom);
                }
                #[cfg(test)]
                if let Some(pause) = &mut pause {
                    pause.wait();
                }
                let update = crate::journal::request::isolate(|| {
                    if force {
                        pull.checkpoint(generation);
                    }
                    pull.pull()
                });
                (update, headroom)
            });
            let queued = Queued(job.abort_handle());
            let (update, headroom) = job.await?;
            drop(queued);
            applied = generation;
            if cancelled.load(Ordering::Acquire) {
                return Ok(());
            }
            let mut reason = None;
            let mut last = false;
            let mut bytes = update.frame.as_ref().map_or(0, Vec::len);
            for event in &update.events {
                bytes += serde_json::to_vec(event)?.len() + 1024;
                anyhow::ensure!(
                    bytes <= MAX_MESSAGE,
                    "subscription event batch exceeds retained bytes; subscribe again for current state"
                );
            }
            let retained = Arc::new(headroom.retain(bytes)?);
            let mut reading = None;
            if let Some(frame) = update.frame {
                let mut frame: Value = serde_json::from_slice(&frame)?;
                reason = frame["unavailable"].as_str().map(str::to_owned);
                last = ends.ends_after(&frame["body"]);
                // Internal generations never appear in a consumer's envelope.
                let identity =
                    crate::lf::commands::subscribe::identity(reason.as_deref(), frame.get("body"))?;
                frame["answers"] = Value::Null;
                reading = Some(Arc::new(Reading {
                    generation,
                    frame,
                    identity,
                    _retained: retained.clone(),
                }));
            }
            if reading.is_some() || !update.events.is_empty() {
                latest.send_modify(|latest| {
                    if let Some(reading) = reading {
                        latest.reading = Some(reading);
                    }
                    if !update.events.is_empty() {
                        let _ = self.events.send(Arc::new(Events {
                            items: update.events,
                            _retained: retained,
                        }));
                        latest.events += 1;
                    }
                });
            }
            if subscription.ended() {
                anyhow::bail!(
                    "{}",
                    reason.unwrap_or_else(|| "the subscription ended".into())
                );
            }
            if last && self.finish(applied) {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{atomic::Ordering, Arc};
    use std::time::Duration;

    use serde_json::Value;
    use tokio::sync::mpsc;
    use tokio::task::JoinHandle;

    use super::{Delivery, Member, Subscriptions};
    use crate::lf::commands::read::Read;
    use crate::performance::subscription_measurement as meter;
    use crate::server::outbox::Outbox;
    use crate::server::service::subscription_measurement::pause;
    use crate::server::wire::ServerMessage;
    use crate::store::sqlite::SqliteStore;

    async fn receive(messages: &mut mpsc::Receiver<crate::server::outbox::Frame>) -> ServerMessage {
        let frame = messages.recv().await.unwrap();
        let message = serde_json::from_str(&frame.payload.into_line(&mut 0).unwrap()).unwrap();
        drop(frame.reservation);
        let _ = frame.delivered.send(());
        message
    }

    struct Consumer {
        member: Member,
        messages: mpsc::Receiver<super::super::outbox::Frame>,
        delivery: JoinHandle<()>,
    }
    impl Consumer {
        fn new(
            registry: &Arc<Subscriptions>,
            id: &str,
            read: Read,
            select: Option<Vec<String>>,
        ) -> Self {
            let (member, delivery) = registry.join(id, read, select).unwrap();
            Self::from_join(member, delivery, id)
        }
        fn from_join(member: Member, delivery: Delivery, id: &str) -> Self {
            let (outgoing, messages) = Outbox::new();
            let delivery = tokio::spawn(delivery.run(id.into(), outgoing, Vec::new(), None));
            Self {
                member,
                messages,
                delivery,
            }
        }
        async fn next(&mut self) -> ServerMessage {
            tokio::time::timeout(Duration::from_secs(10), receive(&mut self.messages))
                .await
                .unwrap()
        }
        async fn initial(&mut self) -> Value {
            assert!(matches!(self.next().await, ServerMessage::Response { .. }));
            self.reading().await
        }
        async fn reading(&mut self) -> Value {
            let ServerMessage::Reading { frame, .. } = self.next().await else {
                panic!("expected reading")
            };
            frame
        }
    }
    impl Drop for Consumer {
        fn drop(&mut self) {
            self.delivery.abort();
        }
    }

    fn with_store<F: std::future::Future<Output = ()>>(body: impl FnOnce(SqliteStore) -> F) {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(2)
            .enable_all()
            .build()
            .unwrap();
        meter::ENABLED.store(true, Ordering::Relaxed);
        let before = meter::snapshot();
        runtime.block_on(body(store));
        // Joins every admitted blocking job and destructor before checking release,
        // rather than treating SQLITE_TRACE_CLOSE as completed destruction.
        drop(runtime);
        let work = meter::snapshot().since(before);
        meter::ENABLED.store(false, Ordering::Relaxed);
        assert_eq!(stage(&work, "runtime_build"), stage(&work, "runtime_drop"));
        assert_eq!(work.opens, work.closes);
    }

    fn stage(work: &meter::Snapshot, name: &str) -> u64 {
        work.stages.get(name).map_or(0, |count| count.count)
    }

    fn sessions() -> Read {
        Read::SessionList {
            directory: "/".into(),
            headless: true,
            after: None,
            limit: None,
        }
    }

    #[tokio::test]
    async fn events_survive_coalescing_or_end_a_lagging_consumer() {
        use super::{Events, Latest, Pending, Retained};
        use crate::server::wire::SubscriptionEvent;
        use crate::work::task::{TaskId, TaskPrId};
        use std::sync::Mutex;

        for count in [2, 17] {
            let (latest, receiver) = tokio::sync::watch::channel(Latest::default());
            let (events, pending_events) = tokio::sync::broadcast::channel(16);
            let delivery = Delivery {
                latest: receiver,
                pending: Arc::new(Mutex::new(Pending {
                    generation: 1,
                    checkpoint: None,
                })),
                events: pending_events,
                delivered_events: 0,
            };
            // Several publications happen before this consumer can deliver.
            for number in 1..=count {
                latest.send_modify(|latest| {
                    events
                        .send(Arc::new(Events {
                            items: vec![SubscriptionEvent::TaskShipped {
                                task_id: TaskId::parse("task_11111111111111111111111111111111")
                                    .unwrap(),
                                pr_id: TaskPrId::new(),
                                number,
                                url: format!("https://example.com/pull/{number}"),
                                merge_commit: number.to_string(),
                            }],
                            _retained: Arc::new(Retained::empty()),
                        }))
                        .unwrap();
                    latest.ready = true;
                    latest.events += 1;
                });
            }
            latest.send_modify(|latest| latest.ended = Some(Ok(())));
            let (outgoing, mut messages) = Outbox::new();
            let task = tokio::spawn(delivery.run("events".into(), outgoing, Vec::new(), None));
            assert!(matches!(
                receive(&mut messages).await,
                ServerMessage::Response { .. }
            ));
            if count == 2 {
                for expected in 1..=count {
                    assert!(matches!(
                        receive(&mut messages).await,
                        ServerMessage::Event {
                            event: SubscriptionEvent::TaskShipped { number, .. }, ..
                        } if number == expected
                    ));
                }
                assert!(matches!(
                    receive(&mut messages).await,
                    ServerMessage::Ended { .. }
                ));
            } else {
                assert!(matches!(
                    receive(&mut messages).await,
                    ServerMessage::Error { .. }
                ));
            }
            task.await.unwrap();
        }
    }

    #[tokio::test]
    async fn timestamp_only_refresh_does_not_send_unsolicited_sibling_readings() {
        use super::{Latest, Pending, Reading};
        use std::sync::Mutex;
        let (latest, receiver) = tokio::sync::watch::channel(Latest::default());
        let (outgoing, mut messages) = Outbox::new();
        let delivery = Delivery {
            latest: receiver,
            events: tokio::sync::broadcast::channel(16).1,
            delivered_events: 0,
            pending: Arc::new(Mutex::new(Pending {
                generation: 1,
                checkpoint: None,
            })),
        };
        let task = tokio::spawn(delivery.run("quiet".into(), outgoing, Vec::new(), None));
        let reading = |generation, at| {
            let frame = serde_json::json!({"answers": null, "unavailable": null, "body": {
                "generated_at": at, "evidence_age_secs": at,
                "condition": { "observed_at": at, "state": "ready" }, "title": "same"
            }});
            let identity =
                crate::lf::commands::subscribe::identity(None, frame.get("body")).unwrap();
            Arc::new(Reading {
                generation,
                frame,
                identity,
                _retained: Arc::new(super::Retained::empty()),
            })
        };
        latest.send_replace(Latest {
            ready: true,
            events: 0,
            reading: Some(reading(1, 10)),
            ended: None,
        });
        assert!(matches!(
            receive(&mut messages).await,
            ServerMessage::Response { .. }
        ));
        assert!(matches!(
            receive(&mut messages).await,
            ServerMessage::Reading { .. }
        ));
        latest.send_replace(Latest {
            ready: true,
            events: 0,
            reading: Some(reading(2, 20)),
            ended: Some(Ok(())),
        });
        // End is an ordered barrier proving the changed timestamp was processed.
        assert!(matches!(
            receive(&mut messages).await,
            ServerMessage::Ended { .. }
        ));
        task.await.unwrap();
    }

    #[tokio::test]
    async fn a_terminal_failure_sends_its_last_reading_without_answering_a_later_checkpoint() {
        use super::{Latest, Pending, Reading};
        use std::sync::Mutex;
        let frame =
            serde_json::json!({"answers": null, "unavailable": "snapshot failed", "body": null});
        let identity =
            crate::lf::commands::subscribe::identity(Some("snapshot failed"), None).unwrap();
        let (latest, receiver) = tokio::sync::watch::channel(Latest {
            ready: true,
            events: 0,
            reading: Some(Arc::new(Reading {
                generation: 1,
                frame,
                identity,
                _retained: Arc::new(super::Retained::empty()),
            })),
            ended: Some(Err(Arc::new(crate::engine::Error::new(
                crate::engine::ErrorCode::Unavailable,
                "snapshot failed",
            )))),
        });
        let (outgoing, mut messages) = Outbox::new();
        let delivery = Delivery {
            latest: receiver,
            events: tokio::sync::broadcast::channel(16).1,
            delivered_events: 0,
            pending: Arc::new(Mutex::new(Pending {
                generation: 2,
                checkpoint: Some(9),
            })),
        };
        let task = tokio::spawn(delivery.run("failed".into(), outgoing, Vec::new(), None));
        assert!(matches!(
            receive(&mut messages).await,
            ServerMessage::Response { .. }
        ));
        let ServerMessage::Reading { frame, .. } = receive(&mut messages).await else {
            panic!("missing final failure reading")
        };
        assert!(frame["answers"].is_null());
        assert_eq!(frame["unavailable"], "snapshot failed");
        assert!(matches!(
            receive(&mut messages).await,
            ServerMessage::Error { .. }
        ));
        task.await.unwrap();
        drop(latest);
    }

    #[tokio::test]
    async fn a_published_failure_is_not_repeated_when_the_stream_ends() {
        use super::{Latest, Pending, Reading};
        use std::sync::Mutex;

        let reason = Some("snapshot failed".to_owned());
        let identity = crate::lf::commands::subscribe::identity(reason.as_deref(), None).unwrap();
        let (latest, receiver) = tokio::sync::watch::channel(Latest {
            ready: true,
            events: 0,
            reading: Some(Arc::new(Reading {
                generation: 1,
                frame: serde_json::json!({
                    "answers": null, "unavailable": reason, "body": null
                }),
                identity,
                _retained: Arc::new(super::Retained::empty()),
            })),
            ended: None,
        });
        let pending = Arc::new(Mutex::new(Pending {
            generation: 1,
            checkpoint: None,
        }));
        let (outgoing, mut messages) = Outbox::new();
        let task = tokio::spawn(
            Delivery {
                latest: receiver,
                events: tokio::sync::broadcast::channel(16).1,
                delivered_events: 0,
                pending: pending.clone(),
            }
            .run("failed".into(), outgoing, Vec::new(), None),
        );
        assert!(matches!(
            receive(&mut messages).await,
            ServerMessage::Response { .. }
        ));
        assert!(matches!(
            receive(&mut messages).await,
            ServerMessage::Reading { .. }
        ));

        // The failure was already delivered when a later checkpoint arrived.
        *pending.lock().unwrap() = Pending {
            generation: 2,
            checkpoint: Some(9),
        };
        latest.send_modify(|value| {
            value.ended = Some(Err(Arc::new(crate::engine::Error::new(
                crate::engine::ErrorCode::Unavailable,
                "snapshot failed",
            ))));
        });
        assert!(matches!(
            receive(&mut messages).await,
            ServerMessage::Error { .. }
        ));
        task.await.unwrap();
    }

    #[test]
    fn identical_demand_shares_setup_and_coalesces_independent_checkpoints() {
        with_store(|_store| {
            Box::pin(async {
                let registry = Arc::new(Subscriptions::default());
                let setup = pause("first", "setup");
                let before = meter::snapshot();
                let mut first = Consumer::new(&registry, "first", Read::ProcessList, None);
                setup.entered.await.unwrap();
                let mut second = Consumer::new(&registry, "second", Read::ProcessList, None);
                let mut quiet = Consumer::new(&registry, "quiet", Read::ProcessList, None);
                setup.release.send(()).unwrap();
                first.initial().await;
                second.initial().await;
                quiet.initial().await;
                let work = meter::snapshot().since(before);
                assert_eq!(stage(&work, "runtime_build"), 1);
                assert_eq!(stage(&work, "projection"), 1);

                let before = meter::snapshot();
                let pull = pause("first", "pull");
                assert!(first.member.checkpoint(900));
                pull.entered.await.unwrap();
                for id in (0..100).rev() {
                    assert!(first.member.checkpoint(id));
                }
                assert!(second.member.checkpoint(0));
                pull.release.send(()).unwrap();
                assert_eq!(first.reading().await["answers"], 0);
                assert_eq!(second.reading().await["answers"], 0);
                assert_eq!(stage(&meter::snapshot().since(before), "projection"), 2);
                // A sibling's checkpoint must not send an unchanged reading here.
                assert!(
                    tokio::time::timeout(Duration::from_millis(100), quiet.messages.recv())
                        .await
                        .is_err()
                );
                drop(first);
                assert!(second.member.checkpoint(7));
                assert_eq!(second.reading().await["answers"], 7);
                drop(second);
                drop(quiet);
                assert!(super::lock(&registry.0).is_empty());
            })
        });
    }

    #[test]
    fn production_sqlite_pulls_progress_with_saturated_outer_pool() {
        with_store(|store| {
            Box::pin(async move {
                store.test_session("visible", &crate::session_record::new_artifact_key());
                let registry = Arc::new(Subscriptions::default());
                let mut first = Consumer::new(&registry, "saturated", sessions(), None);
                first.initial().await;
                let (entered, began) = tokio::sync::oneshot::channel();
                let (release, wait) = std::sync::mpsc::channel();
                let held = tokio::task::spawn_blocking(move || {
                    entered.send(()).unwrap();
                    wait.recv_timeout(Duration::from_secs(15)).unwrap();
                });
                began.await.unwrap();
                let pull = pause("saturated", "pull");
                first.member.checkpoint(1);
                pull.entered.await.unwrap();
                // Both outer slots are held. The production pull must execute its
                // nested run_sqlite on Reader's independent execution capacity.
                pull.release.send(()).unwrap();
                let progressed =
                    tokio::time::timeout(Duration::from_secs(5), receive(&mut first.messages))
                        .await;
                release.send(()).unwrap();
                held.await.unwrap();
                let message = progressed.expect("production pull deadlocked on the outer pool");
                let ServerMessage::Reading { frame, .. } = message else {
                    panic!("expected reading")
                };
                assert_eq!(frame["answers"], 1);
                assert!(frame["body"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["id"] == "visible"));
                let mut sibling =
                    Consumer::new(&registry, "sibling", sessions(), Some(vec!["id".into()]));
                let selected = sibling.initial().await;
                assert!(selected["body"].as_array().unwrap().iter().all(|row| row
                    .as_object()
                    .unwrap()
                    .len()
                    == 1));
            })
        });
    }

    #[test]
    fn reader_cleanup_finishes_even_when_the_outer_runtime_rejects_jobs() {
        with_store(|_store| {
            Box::pin(async {
                let before = meter::snapshot();
                let (held, stopped) = tokio::task::spawn_blocking(|| {
                    let runtime = tokio::runtime::Runtime::new().unwrap();
                    let stopped = runtime.handle().clone();
                    drop(runtime);
                    let subscription = sessions().subscribe(None, || {}).unwrap();
                    assert!(subscription.pull().frame.is_some());
                    (super::Held(Some(subscription)), stopped)
                })
                .await
                .unwrap();
                // Still inside an async task, but Tokio can no longer admit the
                // destructor to this pool. Drop must complete without a runtime panic.
                let entered = stopped.enter();
                drop(held);
                drop(entered);
                let work = meter::snapshot().since(before);
                assert_eq!(stage(&work, "runtime_build"), 1);
                assert_eq!(stage(&work, "runtime_drop"), 1);
                assert_eq!(work.opens, work.closes);
            })
        });
    }

    #[test]
    fn cancelled_queued_pull_does_no_projection_and_releases_its_reader() {
        with_store(|_store| {
            Box::pin(async {
                let registry = Arc::new(Subscriptions::default());
                let mut first = Consumer::new(&registry, "queued-pull", sessions(), None);
                first.initial().await;
                let mut workers = Vec::new();
                let mut releases = Vec::new();
                for _ in 0..2 {
                    let (entered, began) = tokio::sync::oneshot::channel();
                    let (release, wait) = std::sync::mpsc::channel();
                    releases.push(release);
                    workers.push(tokio::task::spawn_blocking(move || {
                        entered.send(()).unwrap();
                        wait.recv_timeout(Duration::from_secs(15)).unwrap();
                    }));
                    began.await.unwrap();
                }
                let before = meter::snapshot();
                let held = pause("queued-pull", "pull");
                first.member.checkpoint(1);
                held.queued.await.unwrap();
                drop(first);
                assert!(super::lock(&registry.0).is_empty());
                let _ = held.release.send(());
                for release in releases {
                    release.send(()).unwrap();
                }
                for worker in workers {
                    worker.await.unwrap();
                }
                held.finished.await.unwrap();
                assert_eq!(stage(&meter::snapshot().since(before), "projection"), 0);
                let mut fresh = Consumer::new(&registry, "queued-pull", sessions(), None);
                fresh.initial().await;
            })
        });
    }

    #[test]
    fn distinct_arguments_selection_and_service_lifetimes_do_not_alias() {
        with_store(|_store| {
            Box::pin(async {
                let registry = Arc::new(Subscriptions::default());
                let other_service = Arc::new(Subscriptions::default());
                let before = meter::snapshot();
                let mut first = Consumer::new(&registry, "one", sessions(), None);
                let mut different_arguments = Consumer::new(
                    &registry,
                    "two",
                    Read::SessionList {
                        directory: "/".into(),
                        headless: false,
                        after: None,
                        limit: None,
                    },
                    None,
                );
                let mut different_select =
                    Consumer::new(&registry, "three", sessions(), Some(vec!["title".into()]));
                let mut other = Consumer::new(&other_service, "one", sessions(), None);
                for consumer in [
                    &mut first,
                    &mut different_arguments,
                    &mut different_select,
                    &mut other,
                ] {
                    consumer.initial().await;
                }
                assert_eq!(stage(&meter::snapshot().since(before), "runtime_build"), 4);
                drop(first);
                drop(different_arguments);
                drop(different_select);
                assert!(super::lock(&registry.0).is_empty());
                other.member.checkpoint(1);
                assert_eq!(other.reading().await["answers"], 1);
            })
        });
    }

    #[test]
    fn cancelling_identical_consumers_and_reusing_a_key_never_joins_old_work() {
        with_store(|_store| {
            Box::pin(async {
                let registry = Arc::new(Subscriptions::default());
                for stage_name in ["setup", "pull"] {
                    let held = pause("old", stage_name);
                    let first = Consumer::new(&registry, "old", sessions(), None);
                    held.entered.await.unwrap();
                    let mut sibling = Consumer::new(&registry, "sibling", sessions(), None);
                    drop(first);
                    held.release.send(()).unwrap();
                    sibling.initial().await;
                    assert!(sibling.member.checkpoint(4));
                    assert_eq!(sibling.reading().await["answers"], 4);
                    drop(sibling);
                    assert!(super::lock(&registry.0).is_empty());

                    let held = pause("old", stage_name);
                    let first = Consumer::new(&registry, "old", sessions(), None);
                    held.entered.await.unwrap();
                    drop(first);
                    let mut replacement = Consumer::new(&registry, "old", sessions(), None);
                    replacement.initial().await;
                    held.release.send(()).unwrap();
                    held.finished.await.unwrap();
                    replacement.member.checkpoint(2);
                    assert_eq!(replacement.reading().await["answers"], 2);
                    drop(replacement);
                }
            })
        });
    }

    #[test]
    fn slow_delivery_keeps_latest_state_without_blocking_a_sibling() {
        with_store(|_store| {
            Box::pin(async {
                let registry = Arc::new(Subscriptions::default());
                let mut slow = Consumer::new(&registry, "slow", sessions(), None);
                let mut fast = Consumer::new(&registry, "fast", sessions(), None);
                fast.initial().await;
                // Slow has a one-message output queue, with at most one send held
                // by its delivery task and one shared latest reading behind that.
                for checkpoint in 0..32 {
                    slow.member.checkpoint(checkpoint);
                    fast.member.checkpoint(checkpoint);
                    assert_eq!(fast.reading().await["answers"], checkpoint);
                }
                assert_eq!(slow.messages.len(), 1);
                assert!(matches!(slow.next().await, ServerMessage::Response { .. }));
                loop {
                    if slow.reading().await["answers"] == 31 {
                        break;
                    }
                }
                drop(slow);
                fast.member.checkpoint(0);
                assert_eq!(fast.reading().await["answers"], 0);
            })
        });
    }

    #[test]
    fn failed_read_recovers_and_terminal_joins_receive_fresh_final_readings() {
        with_store(|store| {
            Box::pin(async move {
                let registry = Arc::new(Subscriptions::default());
                let id = crate::durable::TaskId::new();
                let read = Read::TaskWait {
                    issue: id.to_string(),
                    until: crate::ops::task::TaskWaitUntil::Terminal,
                };
                let mut first = Consumer::new(
                    &registry,
                    "terminal",
                    read.clone(),
                    Some(vec!["task.title".into()]),
                );
                assert!(first.initial().await["unavailable"].is_string());
                let repo = loopflow_test_support::TestRepo::new();
                store
                    .create_task(&crate::work::task::NewTask {
                        repository_id: store
                            .ensure_repository(repo.path().to_str().unwrap())
                            .unwrap(),
                        id: id.clone(),
                        project_id: None,
                        title: "Recovered".into(),
                        description: String::new(),
                        due_date: None,
                    })
                    .unwrap();
                store
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE tasks SET planning_completed=1 WHERE id=?1",
                        [id.as_str()],
                    )
                    .unwrap();
                let held = pause("terminal", "pull");
                first.member.checkpoint(8);
                held.entered.await.unwrap();
                // Join and another checkpoint while a terminal read is in flight.
                let mut joined = Consumer::new(
                    &registry,
                    "joined",
                    read.clone(),
                    Some(vec!["task.title".into()]),
                );
                first.member.checkpoint(0);
                held.release.send(()).unwrap();
                let recovered = first.reading().await;
                assert!(recovered["unavailable"].is_null(), "{recovered}");
                assert_eq!(recovered["answers"], 0);
                assert_eq!(
                    recovered["body"]["reached"], true,
                    "ending fields survive select"
                );
                assert_eq!(joined.initial().await["body"]["reached"], true);
                assert!(matches!(first.next().await, ServerMessage::Ended { .. }));
                assert!(matches!(joined.next().await, ServerMessage::Ended { .. }));
                assert!(!first.member.checkpoint(9));
                assert!(super::lock(&registry.0).is_empty());
                let mut replacement =
                    Consumer::new(&registry, "terminal", read, Some(vec!["task.title".into()]));
                drop(first);
                drop(joined);
                assert_eq!(replacement.initial().await["body"]["reached"], true);
                assert!(matches!(
                    replacement.next().await,
                    ServerMessage::Ended { .. }
                ));
            })
        });
    }

    #[test]
    fn setup_errors_do_not_poison_the_next_admission() {
        with_store(|_store| {
            Box::pin(async {
                let registry = Arc::new(Subscriptions::default());
                for _ in 0..4 {
                    let mut refused = Consumer::new(&registry, "reuse", Read::MachineSelf, None);
                    assert!(matches!(refused.next().await, ServerMessage::Error { .. }));
                    assert!(super::lock(&registry.0).is_empty());
                    let mut valid = Consumer::new(&registry, "reuse", sessions(), None);
                    valid.initial().await;
                    drop(refused);
                    valid.member.checkpoint(1);
                    assert_eq!(valid.reading().await["answers"], 1);
                    drop(valid);
                    assert!(super::lock(&registry.0).is_empty());
                }
            })
        });
    }
}
