//! Isolated server for scripts/benchmarks/subscription_baseline.py. Run only
//! through that driver; the control socket is fixture-only, never a wire API.
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener as ControlListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::net::UnixListener;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use super::{Clients, Connection, Tickets};
use crate::durable::TaskId;
use crate::id::LfProcessId;
use crate::performance::subscription_measurement as meter;
use crate::process::SessionAttachment;
use crate::server::wire::MAX_CONNECTIONS;
use crate::store::sqlite::SqliteStore;

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn attach(store: &SqliteStore, session: &str, pid: u32) -> SessionAttachment {
    let owner = LfProcessId::new();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
            [owner.as_str()],
        )
        .unwrap();
    let attachment = store
        .claim_session_attachment(session, None, &owner, true)
        .unwrap();
    let born = crate::journal::process_started_at(pid).unwrap().unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE processes SET pid=?2,os_started_at=?3 WHERE id=?1",
            rusqlite::params![attachment.agent_process_id, pid, born],
        )
        .unwrap();
    attachment
}

fn seed(home: &Path, pid: u32) {
    let repo = home.join("repo");
    std::fs::create_dir(&repo).unwrap();
    assert!(Command::new("git")
        .args(["init", "--quiet"])
        .arg(&repo)
        .status()
        .unwrap()
        .success());
    let repo = repo.canonicalize().unwrap();
    let store = SqliteStore::open_ephemeral(&home.join("loopflow.db")).unwrap();
    let repository_id = store.ensure_repository(repo.to_str().unwrap()).unwrap();
    for i in 0..64 {
        store
            .create_task(&crate::work::task::NewTask {
                repository_id: repository_id.clone(),
                id: TaskId::new(),
                project_id: None,
                title: format!("Subscription fixture {i}"),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
    }
    for _ in 0..32 {
        store.test_graph(
            "history",
            repo.to_str().unwrap(),
            &[("gate", Some("succeeded"))],
            Some("succeeded"),
        );
    }
    for i in 0..128 {
        let id = format!("session-{i:03}");
        store.test_session(&id, &crate::session_record::new_artifact_key());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET repo=?2,cwd=?2 WHERE id=?1",
                rusqlite::params![
                    id,
                    if i == 127 {
                        home.to_str().unwrap()
                    } else {
                        repo.to_str().unwrap()
                    }
                ],
            )
            .unwrap();
    }
    let related = attach(&store, "session-000", pid);
    let unrelated = attach(&store, "session-127", pid);
    std::fs::write(
        home.join("attachments.json"),
        serde_json::to_vec(&json!([related, unrelated])).unwrap(),
    )
    .unwrap();
}

fn read_writer(reader: &mut BufReader<std::process::ChildStdout>) -> Value {
    loop {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).unwrap() > 0, "writer exited");
        if let Some((_, value)) = line.split_once("WRITER ") {
            return serde_json::from_str(value).unwrap();
        }
    }
}

#[test]
#[ignore = "private subprocess of the subscription benchmark"]
fn subscription_writer() {
    let home = std::env::var("SUBSCRIPTION_HOME").unwrap();
    std::env::set_var("LF_HOME", &home);
    let home = Path::new(&home);
    let store = SqliteStore::open_ephemeral(&home.join("loopflow.db")).unwrap();
    let attachments: Vec<SessionAttachment> =
        serde_json::from_slice(&std::fs::read(home.join("attachments.json")).unwrap()).unwrap();
    let session = store.session("session-127").unwrap().unwrap();
    println!("WRITER {}", json!({"ready":true}));
    let mut count = 0;
    let mut cpu = 0.0;
    for line in std::io::stdin().lock().lines() {
        let command = line.unwrap();
        if command == "stop" {
            break;
        }
        if command == "title" {
            store
                .rename_session(
                    "session-001",
                    "Changed by fixture",
                    crate::session::TitleSource::Human,
                )
                .unwrap();
            println!("WRITER {}", json!({"title":true}));
            continue;
        }
        let before = meter::snapshot();
        count += 1;
        let now = time::OffsetDateTime::now_utc();
        store
            .record_capture_fact(
                &session,
                session.artifact_key.as_ref().unwrap(),
                count,
                now,
                &crate::session_record::CaptureFact::ProviderAttemptStarted {
                    provider: "fixture".into(),
                    model: None,
                    account_id: None,
                    attempt_key: format!("fixture-{count}"),
                },
            )
            .unwrap();
        store
            .record_session_activity(
                &session.id,
                &attachments[1],
                &crate::session::SessionActivity {
                    observed_at: now.unix_timestamp(),
                    open_tools: count as usize % 2,
                    pending_input: 0,
                    yielded: count % 2 == 0,
                    preview: None,
                },
            )
            .unwrap();
        let delta = meter::snapshot().since(before);
        cpu += delta.user_ms + delta.system_ms;
        let receipts: i64 = store.conn.lock().unwrap().query_row(
            "SELECT count(*) FROM session_events WHERE session_id='session-127' AND kind='observed'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(receipts as u64, count);
        println!(
            "WRITER {}",
            json!({"writes":count, "receipts":receipts, "cpu_ms":cpu})
        );
    }
}

#[test]
#[ignore = "manual real-clock server subscription baseline; use its Python driver"]
fn subscription_server() {
    let home = std::env::var("SUBSCRIPTION_HOME").unwrap();
    std::env::set_var("LF_HOME", &home);
    let home = Path::new(&home);
    let mut provider = ChildGuard(
        Command::new("/bin/sleep")
            .arg("1800")
            .stdin(Stdio::null())
            .spawn()
            .unwrap(),
    );
    seed(home, provider.0.id());
    let mut writer = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "server::service::subscription_measurement::subscription_writer",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut writer_input = writer.0.stdin.take().unwrap();
    let mut writer_output = BufReader::new(writer.0.stdout.take().unwrap());
    assert_eq!(read_writer(&mut writer_output)["ready"], true);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let path = home.join("server.sock");
    let serving = runtime.spawn(async move {
        let listener = UnixListener::bind(path).unwrap();
        let clients = Clients::default();
        let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
        let tickets = Arc::new(Tickets::default());
        let mut connections = JoinSet::new();
        tokio::pin!(stopped);
        loop {
            tokio::select! {
                socket = listener.accept() => {
                    let (socket, _) = socket.unwrap();
                    let clients = clients.clone();
                    let slots = slots.clone();
                    let tickets = tickets.clone();
                    connections.spawn(async move {
                        Connection { instance: "subscription-fixture", executable_sha256: "frozen-by-runner",
                            clients: &clients, slots: &slots, tickets: &tickets, uid: 501 }.serve(socket).await
                    });
                }
                Some(result) = connections.join_next(), if !connections.is_empty() => { result.unwrap().unwrap(); }
                _ = &mut stopped => break,
            }
        }
        connections.abort_all();
        while connections.join_next().await.is_some() {}
    });
    meter::ENABLED.store(true, Relaxed);
    let control = ControlListener::bind(home.join("control.sock")).unwrap();
    let (mut output, _) = control.accept().unwrap();
    let input = BufReader::new(output.try_clone().unwrap());
    let mut phases = std::collections::HashMap::new();
    for line in input.lines() {
        let command: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let answer = match command["action"].as_str().unwrap() {
            "begin" => {
                phases.insert(
                    command["name"].as_str().unwrap().to_owned(),
                    meter::snapshot(),
                );
                json!(true)
            }
            "end" => serde_json::to_value(
                meter::snapshot().since(phases.remove(command["name"].as_str().unwrap()).unwrap()),
            )
            .unwrap(),
            "snapshot" => serde_json::to_value(meter::snapshot()).unwrap(),
            "resources" => {
                let threads = Command::new("ps")
                    .args(["-M", "-p", &std::process::id().to_string()])
                    .output()
                    .unwrap();
                json!({"fds":std::fs::read_dir("/dev/fd").unwrap().count(),
                    "threads":String::from_utf8(threads.stdout).unwrap().lines().count() - 1})
            }
            "title" => {
                writeln!(writer_input, "title").unwrap();
                read_writer(&mut writer_output)
            }
            "write" => {
                writeln!(writer_input, "write").unwrap();
                read_writer(&mut writer_output)
            }
            "exit" => {
                provider.0.kill().unwrap();
                provider.0.wait().unwrap();
                json!(true)
            }
            "stop" => break,
            other => panic!("unknown control {other}"),
        };
        writeln!(output, "{answer}").unwrap();
    }
    let _ = stop.send(());
    runtime.block_on(serving).unwrap();
    drop(runtime);
    writeln!(writer_input, "stop").unwrap();
    assert!(writer.0.wait().unwrap().success());
    // The observer's CHECK clock owns final follower removal.
    std::thread::sleep(Duration::from_millis(2200));
    std::fs::write(
        home.join("final.json"),
        serde_json::to_vec(&meter::snapshot()).unwrap(),
    )
    .unwrap();
}

/// This reproduces the proposed borrowed Handle's wait cycle without leaving
/// blocked threads behind. Both outer workers first enqueue real Store work;
/// escape channels then release them so every queued job can be joined.
#[test]
fn borrowed_executor_saturation_requires_releasing_outer_pulls() {
    let home = tempfile::tempdir().unwrap();
    let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (queued, mut observed) = tokio::sync::mpsc::unbounded_channel();
        let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let full = Arc::new(std::sync::Barrier::new(2));
        let (finished, mut finishes) = tokio::sync::mpsc::unbounded_channel();
        let mut releases = Vec::new();
        let mut pulls = Vec::new();
        for _ in 0..2 {
            let (release, escaped) = tokio::sync::oneshot::channel();
            releases.push(release);
            let queued = queued.clone();
            let store = store.clone();
            let completed = completed.clone();
            let handle = tokio::runtime::Handle::current();
            let full = full.clone();
            let finished = finished.clone();
            pulls.push(tokio::task::spawn_blocking(move || {
                full.wait();
                handle.block_on(async {
                tokio::select! {
                    biased;
                    result = crate::store::run_sqlite(&store, move |store| {
                        let result = store.revisions();
                        completed.fetch_add(1, Relaxed);
                        finished.send(()).unwrap();
                        result
                    }) => { result.unwrap(); panic!("nested work ran while both outer slots were held") }
                    _ = async { queued.send(()).unwrap(); escaped.await.unwrap(); } => {}
                }
            })}));
        }
        for _ in 0..2 { observed.recv().await.unwrap(); }
        assert_eq!(completed.load(Relaxed), 0);
        let sibling = tokio::task::spawn_blocking(|| 42);
        assert!(!sibling.is_finished(), "both pool slots belong to the outer pulls");
        for release in releases { release.send(()).unwrap(); }
        for pull in pulls { pull.await.unwrap(); }
        assert_eq!(tokio::time::timeout(Duration::from_secs(5), sibling).await.unwrap().unwrap(), 42);
        for _ in 0..2 {
            tokio::time::timeout(Duration::from_secs(5), finishes.recv()).await.unwrap().unwrap();
        }
        assert_eq!(completed.load(Relaxed), 2);
    });
}

#[test]
fn independent_current_thread_readers_progress_with_saturated_outer_pool() {
    let home = tempfile::tempdir().unwrap();
    let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (ready, mut observed) = tokio::sync::mpsc::unbounded_channel();
        let mut releases = Vec::new();
        let mut pulls = Vec::new();
        for _ in 0..2 {
            let store = store.clone();
            let ready = ready.clone();
            let (release, escaped) = std::sync::mpsc::channel();
            releases.push(release);
            pulls.push(tokio::task::spawn_blocking(move || {
                let reader = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .max_blocking_threads(1)
                    .build()
                    .unwrap();
                let revisions = reader
                    .block_on(crate::store::run_sqlite(&store, |store| store.revisions()))
                    .unwrap();
                ready.send(revisions).unwrap();
                escaped.recv_timeout(Duration::from_secs(10)).unwrap();
            }));
        }
        for _ in 0..2 {
            tokio::time::timeout(Duration::from_secs(5), observed.recv())
                .await
                .unwrap()
                .unwrap();
        }
        let sibling = tokio::task::spawn_blocking(|| 42);
        assert!(!sibling.is_finished());
        for release in releases {
            release.send(()).unwrap();
        }
        for pull in pulls {
            pull.await.unwrap();
        }
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), sibling)
                .await
                .unwrap()
                .unwrap(),
            42
        );
    });
}

// Deterministic pauses are inactive in timing runs and ordinary tests.
type Pauses = std::collections::HashMap<(String, &'static str), Pause>;
static PAUSES: std::sync::LazyLock<std::sync::Mutex<Pauses>> =
    std::sync::LazyLock::new(Default::default);
pub(super) struct Pause {
    queued: Option<tokio::sync::oneshot::Sender<()>>,
    entered: Option<tokio::sync::oneshot::Sender<()>>,
    release: std::sync::mpsc::Receiver<()>,
    finished: Option<tokio::sync::oneshot::Sender<()>>,
}
impl Pause {
    pub(super) fn wait(&mut self) {
        let _ = self.entered.take().unwrap().send(());
        self.release.recv_timeout(Duration::from_secs(30)).unwrap();
    }
}
impl Drop for Pause {
    fn drop(&mut self) {
        if let Some(finished) = self.finished.take() {
            let _ = finished.send(());
        }
    }
}
pub(super) fn take_pause(id: &str, stage: &'static str) -> Option<Pause> {
    let mut pause = PAUSES.lock().unwrap().remove(&(id.to_owned(), stage))?;
    let _ = pause.queued.take().unwrap().send(());
    Some(pause)
}
pub(super) struct Paused {
    pub queued: tokio::sync::oneshot::Receiver<()>,
    pub entered: tokio::sync::oneshot::Receiver<()>,
    pub release: std::sync::mpsc::Sender<()>,
    pub finished: tokio::sync::oneshot::Receiver<()>,
}
pub(super) fn pause(id: &str, stage: &'static str) -> Paused {
    let (queued, queue) = tokio::sync::oneshot::channel();
    let (entered, enter) = tokio::sync::oneshot::channel();
    let (release, wait) = std::sync::mpsc::channel();
    let (finished, finish) = tokio::sync::oneshot::channel();
    assert!(PAUSES
        .lock()
        .unwrap()
        .insert(
            (id.into(), stage),
            Pause {
                queued: Some(queued),
                entered: Some(entered),
                release: wait,
                finished: Some(finished),
            }
        )
        .is_none());
    Paused {
        queued: queue,
        entered: enter,
        release,
        finished: finish,
    }
}
