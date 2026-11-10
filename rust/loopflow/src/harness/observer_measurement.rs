//! Frozen, opt-in baseline for the actual native and Codex loops. Run alone:
//! cargo test --release -p loopflow --lib observer_baseline -- --ignored --nocapture --test-threads=1
//! OBSERVER_KIND=native|codex OBSERVER_COUNT=1|16 OBSERVER_ACTIVE=0|1
//! OBSERVER_PROFILE=1 adds SQL attribution, excluded from reported samples.
use super::{observe, CodexObserver};
use crate::durable::TaskId;
use crate::harness::task_updates;
use crate::id::LfProcessId;
use crate::performance::observer_measurement as meter;
use crate::process::SessionAttachment;
use crate::steer::SteerAdapter;
use crate::store::sqlite::SqliteStore;
use crate::work::task::comment::{TaskComment, TaskCommentAuthor};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tokio::net::{UnixListener, UnixStream};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};

type Provider = WebSocketStream<UnixStream>;
const WINDOW: Duration = Duration::from_millis(4200);
const MARKER_ID: &str = "00000000-0000-4000-8000-000000000001";
const MARKER: &str = "observer baseline marker";

struct FixtureClient(std::process::Child);
impl Drop for FixtureClient {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn attachment(store: &SqliteStore, session: &str, pid: u32) -> (SessionAttachment, SteerAdapter) {
    store.test_session(session, &crate::session_record::new_artifact_key());
    let process = LfProcessId::new();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
            [process.as_str()],
        )
        .unwrap();
    let attachment = store
        .claim_session_attachment(session, None, &process, true)
        .unwrap();
    let adapter = store.reserve_steer_adapter(session, &attachment).unwrap();
    let born = crate::journal::process_started_at(pid).unwrap().unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE processes SET pid=?2,os_started_at=?3 WHERE id=?1",
            rusqlite::params![adapter.agent_process_id, pid, born],
        )
        .unwrap();
    assert_eq!(
        store
            .agent_process(&adapter.agent_process_id)
            .unwrap()
            .unwrap()
            .pid,
        Some(pid)
    );
    (attachment, adapter)
}

async fn receive(socket: &mut Provider) -> Value {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(15), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if let Message::Text(text) = message {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

async fn send(socket: &mut Provider, value: Value) {
    socket
        .send(Message::Text(value.to_string().into()))
        .await
        .unwrap();
}

fn wait_for(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !ready() {
        assert!(Instant::now() < deadline, "observer did not make progress");
        thread::sleep(Duration::from_millis(20));
    }
}

fn descriptors() -> usize {
    std::fs::read_dir("/dev/fd").unwrap().count()
}

fn thread_count() -> usize {
    let output = Command::new("ps")
        .args(["-M", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().lines().count() - 1
}

fn run_codex(
    home: &Path,
    session: String,
    attachment: SessionAttachment,
    pid: u32,
    texts: Arc<Mutex<Vec<String>>>,
) -> (thread::JoinHandle<()>, tokio::sync::oneshot::Sender<()>) {
    let database = home.join("loopflow.db");
    let endpoint = home.join(format!("{session}.sock"));
    let cwd = home.to_path_buf();
    let (stop, mut stopped) = tokio::sync::oneshot::channel();
    let (ready, started) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        // Same runtime shape as run_observer; one runtime per observation worker.
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let listener = UnixListener::bind(&endpoint).unwrap();
            let observer = CodexObserver {
                database, session, attachment, endpoint, thread: "native-thread".into(),
                account: None, cwd, client: (pid, crate::journal::process_started_at(pid).unwrap().unwrap()),
                input: None,
            };
            let watching = tokio::spawn(observe(observer, move || { ready.send(()).unwrap(); }));
            let (stream, _) = listener.accept().await.unwrap();
            let mut provider = accept_async(stream).await.unwrap();
            let initialize = receive(&mut provider).await;
            assert_eq!(initialize["method"], "initialize");
            send(&mut provider, json!({"id":initialize["id"],"result":{}})).await;
            assert_eq!(receive(&mut provider).await["method"], "initialized");
            let resume = receive(&mut provider).await;
            assert_eq!(resume["method"], "thread/resume");
            send(&mut provider, json!({"id":resume["id"],"result":{"thread":{"id":"native-thread","turns":[]}}})).await;
            loop {
                tokio::select! {
                    message = provider.next() => {
                        let Some(Ok(Message::Text(text))) = message else { panic!("provider closed before cleanup") };
                        let request: Value = serde_json::from_str(&text).unwrap();
                        assert_eq!(request["method"], "turn/start");
                        assert_eq!(request["params"]["threadId"], "native-thread");
                        assert_eq!(request["params"]["input"], json!([]));
                        assert_eq!(request["params"]["toolOutput"]["name"], "loopflow_task_update");
                        texts.lock().unwrap().push(request["params"]["toolOutput"]["output"].as_str().unwrap().to_owned());
                        send(&mut provider, json!({"id":request["id"],"result":{"turn":{"id":"report"}}})).await;
                        send(&mut provider, json!({"method":"turn/completed","params":{"threadId":"native-thread","turn":{"id":"report","status":"completed","items":[]}}})).await;
                    }
                    _ = &mut stopped => { break; }
                }
            }
            drop(provider);
            drop(listener);
            watching.await.unwrap().unwrap();
        });
    });
    started.recv_timeout(Duration::from_secs(20)).unwrap();
    (worker, stop)
}

#[test]
#[ignore = "manual real-clock observer baseline; run alone with isolated LF_HOME"]
fn observer_baseline() {
    let _environment = loopflow_test_support::env_lock().lock().unwrap();
    let kind = std::env::var("OBSERVER_KIND").unwrap_or_else(|_| "native".into());
    assert!(["native", "codex"].contains(&kind.as_str()));
    let count: usize = std::env::var("OBSERVER_COUNT")
        .unwrap_or_else(|_| "1".into())
        .parse()
        .unwrap();
    assert!([1, 16].contains(&count));
    let active = std::env::var("OBSERVER_ACTIVE").as_deref() == Ok("1");
    let profile = std::env::var("OBSERVER_PROFILE").as_deref() == Ok("1");
    let home = tempfile::tempdir_in("/tmp").unwrap();
    let previous_home = std::env::var_os("LF_HOME");
    std::env::set_var("LF_HOME", home.path());
    let cold_fd = descriptors();
    if kind == "codex" {
        // Tokio's process-wide signal pipe lives until process exit. Warm that
        // once so per-observer teardown is measured against the same globals.
        drop(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap(),
        );
    }
    let before_store_fd = descriptors();
    let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
    let task = TaskId::new();
    store
        .create_task(&crate::work::task::NewTask {
            repository_id: store.ensure_repository("/observer-baseline").unwrap(),
            id: task.clone(),
            project_id: None,
            title: "Observer baseline".into(),
            description: String::new(),
            due_date: None,
        })
        .unwrap();
    store.conn.lock().unwrap().execute("UPDATE tasks SET worktree='/observer-baseline',checkout_machine_id=(SELECT id FROM machines WHERE is_local=1) WHERE id=?1", [task.as_str()]).unwrap();
    for _ in 0..32 {
        store.test_graph(
            "history",
            "/observer-baseline",
            &[("gate", Some("succeeded"))],
            Some("succeeded"),
        );
    }
    store.test_session("unrelated", &crate::session_record::new_artifact_key());
    let mut client = FixtureClient(
        Command::new("/bin/sleep")
            .arg("120")
            .stdin(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let attachments: Vec<_> = (0..count)
        .map(|i| {
            let session = format!("observer-{i}");
            let (claim, adapter) = attachment(&store, &session, client.0.id());
            (session, claim, adapter)
        })
        .collect();
    let texts = Arc::new(Mutex::new(Vec::<String>::new()));
    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let before_fd = descriptors();
    let before_threads = thread_count();
    let initial = meter::snapshot();
    meter::start();
    let mut workers = Vec::new();
    let mut stops = Vec::new();
    for (session, claim, adapter) in &attachments {
        if kind == "codex" {
            let (worker, stop) = run_codex(
                home.path(),
                session.clone(),
                claim.clone(),
                client.0.id(),
                texts.clone(),
            );
            workers.push(worker);
            stops.push(stop);
        } else {
            let home = home.path().to_path_buf();
            let adapter = adapter.clone();
            let texts = texts.clone();
            let attempts = attempts.clone();
            workers.push(thread::spawn(move || {
                task_updates::watch(
                    &home,
                    &json!({"hook_event_name":"SessionStart"}),
                    adapter,
                    "native-thread",
                    |store, adapter, _, update| {
                        // One failed fixture delivery exercises the actual loop retry;
                        // no provider input, network, or Task sync is invoked.
                        if update.text.contains(MARKER)
                            && attempts.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0
                        {
                            return Err(crate::store::StoreError::InvalidData(
                                "fixture delivery retry".into(),
                            ));
                        }
                        texts.lock().unwrap().push(update.text.clone());
                        store.record_task_update(
                            &adapter.session,
                            &update.key,
                            "dispatched",
                            &update.observation,
                        )?;
                        store.record_task_update(
                            &adapter.session,
                            &update.key,
                            "accepted",
                            &update.observation,
                        )?;
                        Ok(())
                    },
                )
                .unwrap();
            }));
        }
    }
    wait_for(|| {
        attachments
            .iter()
            .all(|(session, _, _)| store.latest_task_update(session).unwrap().is_some())
    });
    // Native startup's Task sync sees no bound Tasks. Bind only after that
    // first iteration, then let the real loop settle and deliver the baseline.
    thread::sleep(Duration::from_millis(1100));
    for (session, _, _) in &attachments {
        store
            .bind_session(
                session,
                store.session(session).unwrap().unwrap().captured,
                &task,
            )
            .unwrap();
    }
    wait_for(|| texts.lock().unwrap().len() == count);
    thread::sleep(Duration::from_millis(2200));
    texts.lock().unwrap().clear();
    let setup = meter::snapshot().since(initial);
    let live_fd = descriptors();
    let live_threads = thread_count();
    eprintln!(
        "OBSERVER_READY pid={} kind={kind} count={count} active={active}",
        std::process::id()
    );
    meter::begin_window(profile);
    let before = meter::snapshot();
    let started = Instant::now();
    let mut writes = 0;
    for interval in 0..4 {
        if active {
            store.conn.lock().unwrap().execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('unrelated','observed',?1,1,'{}')", [format!("activity-{interval}")]).unwrap();
            writes += 1;
        }
        thread::sleep(Duration::from_secs(1));
    }
    thread::sleep(WINDOW - Duration::from_secs(4));
    let elapsed = started.elapsed();
    let measured = meter::snapshot().since(before);
    meter::end_window();
    assert!(measured.polls >= (count * 4) as u64, "{measured:?}");
    assert!(
        texts.lock().unwrap().is_empty(),
        "unrelated changes delivered an update"
    );
    let delivery_start = Instant::now();
    store
        .append_task_comment(
            &task,
            &TaskComment {
                id: MARKER_ID.into(),
                body: MARKER.into(),
                author: TaskCommentAuthor::Person { name: None },
                created_at: Some("2026-11-09T22:00:00Z".into()),
            },
        )
        .unwrap();
    wait_for(|| {
        texts
            .lock()
            .unwrap()
            .iter()
            .filter(|text| text.contains(MARKER))
            .count()
            == count
    });
    wait_for(|| {
        attachments.iter().all(|(session, _, _)| {
            store
                .session_history(session, 0, 0)
                .unwrap()
                .iter()
                .any(|event| {
                    event.payload["state"] == "accepted"
                        && event.payload.to_string().contains(MARKER_ID)
                })
        })
    });
    let delivery_ms = delivery_start.elapsed().as_secs_f64() * 1000.0;
    let output_digest = {
        let texts = texts.lock().unwrap();
        assert_eq!(texts.len(), count);
        let identifier = store.task(&task).unwrap().unwrap().identifier;
        let normalized = texts[0].replace(&identifier, "TASK");
        assert!(normalized.contains("Task TASK: No Task workflow selected."));
        assert!(normalized
            .contains("New comment from attribution unresolved: observer baseline marker"));
        assert!(texts
            .iter()
            .all(|text| text.replace(&identifier, "TASK") == normalized));
        format!("{:x}", Sha256::digest(normalized.as_bytes()))
    };
    // End only our fixture client. Both production loops observe its exit.
    client.0.kill().unwrap();
    client.0.wait().unwrap();
    for stop in stops {
        stop.send(()).unwrap();
    }
    for worker in workers {
        wait_for(|| worker.is_finished());
        worker.join().unwrap();
    }
    let cleanup = meter::snapshot();
    thread::sleep(Duration::from_millis(1100));
    let after_cleanup = meter::snapshot().since(cleanup);
    let total = meter::snapshot().since(initial);
    meter::stop();
    match previous_home {
        Some(value) => std::env::set_var("LF_HOME", value),
        None => std::env::remove_var("LF_HOME"),
    }
    assert_eq!(after_cleanup.statements, 0);
    assert_eq!(total.opens, total.closes);
    assert_eq!(measured.warnings, 0);
    assert_eq!(total.warnings, u64::from(kind == "native"));
    let after_fd = descriptors();
    let after_threads = thread_count();
    drop(store);
    let after_store_fd = descriptors();
    assert_eq!(after_store_fd, before_store_fd);
    assert_eq!(after_threads, before_threads);
    eprintln!(
        "OBSERVER_RESULT {}",
        json!({
            "kind":kind,"observers":count,"active":active,"profile":profile,
            "elapsed_ms":elapsed.as_secs_f64()*1000.0,"work":measured,"setup":setup,
            "unrelated_writes":writes,"unexpected_errors":0,"delivered":count,
            "injected_delivery_errors":usize::from(kind == "native"),
            "delivery_attempts":if kind == "native" {attempts.load(std::sync::atomic::Ordering::Relaxed)} else {count},
            "delivery_ms":delivery_ms,"checked_output":MARKER,"output_sha256":output_digest,"retained_graphs":32,
            "cold_fds":cold_fd,"fds":[before_fd,live_fd,after_fd],"fds_without_store":[before_store_fd,after_store_fd],"threads":[before_threads,live_threads,after_threads],
            "total_opens":total.opens,"total_closes":total.closes,"post_cleanup":after_cleanup,
        })
    );
}
