use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use super::ActiveRunReader;
use crate::durable::RunId;
use crate::run_record::active::DiscoveryState;
use crate::run_record::{resolve_manifest, write_provider_client, CaptureHandle, RunSpec};
use crate::store::{open_store, SharedStore, StorageConfig};

struct Client(Child);
impl Client {
    fn start() -> Self {
        Self(
            Command::new("/bin/cat")
                .env_remove("LF_RUN_DIR")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        )
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn store(home: &Path) -> SharedStore {
    Arc::new(
        open_store(&StorageConfig::sqlite(home.join("loopflow.db")))
            .await
            .unwrap(),
    )
}

fn prepare(home: &Path) -> RunId {
    CaptureHandle::prepare_at(
        home,
        RunSpec {
            harness: "cat".into(),
            model: None,
            surface: "tui".into(),
            cwd: home.to_owned(),
            repo: None,
            worktree: None,
            skill: None,
            subjects: Vec::new(),
            flow: crate::run_record::RunFlowMembership::Independent,
        },
        None,
    )
    .unwrap()
}

async fn visible(reader: &mut ActiveRunReader, store: &SharedStore, ids: &[RunId]) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = reader.observe(store, None).await;
        let mut actual = snapshot
            .runs
            .iter()
            .map(|run| run.id.as_str())
            .collect::<Vec<_>>();
        let mut expected = ids.iter().map(RunId::as_str).collect::<Vec<_>>();
        actual.sort();
        expected.sort();
        if snapshot.discovery == DiscoveryState::Ready
            && snapshot.gaps.is_empty()
            && actual == expected
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "did not converge: {snapshot:?}; expected {ids:?}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test]
async fn native_feed_discovers_old_resumes_replacement_and_removal() {
    let home = tempfile::tempdir().unwrap();
    let store = store(home.path()).await;
    let old = prepare(home.path());
    let (dir, mut manifest) = resolve_manifest(home.path(), old.as_str()).unwrap();
    manifest.created_at = time::OffsetDateTime::UNIX_EPOCH;
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let first = Client::start();
    write_provider_client(&dir, first.0.id()).unwrap();
    let cancel = CancellationToken::new();
    let mut reader = ActiveRunReader::start(home.path(), true, cancel.clone()).unwrap();
    visible(&mut reader, &store, std::slice::from_ref(&old)).await;
    for _ in 0..3 {
        visible(&mut reader, &store, std::slice::from_ref(&old)).await;
        assert_eq!(reader.cost.directories, 0, "warm read enumerated history");
        assert_eq!(reader.cost.rescans, 0);
    }
    drop(first);
    visible(&mut reader, &store, &[]).await;
    assert!(reader.candidates.is_empty());
    let mut resumed = Client::start();
    write_provider_client(&dir, resumed.0.id()).unwrap();
    visible(&mut reader, &store, std::slice::from_ref(&old)).await;
    let fresh = prepare(home.path());
    let (fresh_dir, _) = resolve_manifest(home.path(), fresh.as_str()).unwrap();
    let second = Client::start();
    write_provider_client(&fresh_dir, second.0.id()).unwrap();
    visible(&mut reader, &store, &[old.clone(), fresh.clone()]).await;

    // A final receipt is corrupt, then atomically replaced with its original.
    let path = fresh_dir
        .join("provider-clients")
        .join(format!("{}.json", second.0.id()));
    let bytes = fs::read(&path).unwrap();
    fs::write(&path, b"{").unwrap();
    let incomplete = reader.observe(&store, None).await;
    assert!(!incomplete.gaps.is_empty());
    let staging = path.with_extension("staging");
    fs::write(&staging, bytes).unwrap();
    fs::rename(staging, &path).unwrap();
    visible(&mut reader, &store, &[old.clone(), fresh]).await;
    fs::remove_file(&path).unwrap();
    visible(&mut reader, &store, std::slice::from_ref(&old)).await;

    reader.pending.invalidate(); // deterministic loss at the real feed boundary
    let recovered = reader.observe(&store, None).await;
    assert_eq!(reader.cost.rescans, 1);
    let cold = crate::run_record::active::snapshot(home.path(), &store, None).await;
    assert_eq!(recovered.discovery, DiscoveryState::Ready);
    assert_eq!(recovered.runs, cold.runs);
    assert_eq!(recovered.gaps, cold.gaps);
    let saved = home.path().join("saved-runs");
    fs::rename(home.path().join("runs"), &saved).unwrap();
    visible(&mut reader, &store, &[]).await;
    fs::rename(saved, home.path().join("runs")).unwrap();
    visible(&mut reader, &store, std::slice::from_ref(&old)).await;
    cancel.cancel();
    assert_eq!(
        reader.observe(&store, None).await.discovery,
        DiscoveryState::Unavailable
    );
    drop(reader);
    assert!(
        resumed.0.try_wait().unwrap().is_none(),
        "reader cancellation stopped a provider"
    );
}

#[tokio::test]
async fn missing_roots_and_replaced_home_never_become_false_empty() {
    let parent = tempfile::tempdir().unwrap();
    let home = parent.path().join("new-home");
    let store = store(parent.path()).await;
    let mut reader = ActiveRunReader::start(&home, true, CancellationToken::new()).unwrap();
    visible(&mut reader, &store, &[]).await;
    let id = prepare(&home);
    let (dir, _) = resolve_manifest(&home, id.as_str()).unwrap();
    let client = Client::start();
    write_provider_client(&dir, client.0.id()).unwrap();
    visible(&mut reader, &store, &[id]).await;
    fs::rename(&home, parent.path().join("old-home")).unwrap();
    fs::create_dir(&home).unwrap();
    let replaced = reader.observe(&store, None).await;
    assert_eq!(replaced.discovery, DiscoveryState::Unavailable);
    assert!(!replaced.gaps.is_empty());
}

#[tokio::test]
async fn retained_ownerless_capture_follows_loss_of_native_history() {
    let home = tempfile::tempdir().unwrap();
    let store = store(home.path()).await;
    let id = prepare(home.path());
    let (dir, _) = resolve_manifest(home.path(), id.as_str()).unwrap();
    let _binding = crate::run_record::active::RunBindingGuard::publish_at(
        home.path(),
        super::RunBinding {
            run_id: id.clone(),
            owner: None,
        },
    )
    .unwrap();
    let client = Client::start();
    write_provider_client(&dir, client.0.id()).unwrap();
    let mut reader = ActiveRunReader::start(home.path(), true, CancellationToken::new()).unwrap();
    visible(&mut reader, &store, std::slice::from_ref(&id)).await;
    let receipt = dir
        .join("provider-clients")
        .join(format!("{}.json", client.0.id()));
    let bytes = fs::read(&receipt).unwrap();
    fs::remove_file(&receipt).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let observed = reader.observe(&store, None).await;
        if observed.discovery == DiscoveryState::Ready {
            let cold = crate::run_record::active::snapshot(home.path(), &store, None).await;
            assert_eq!(observed.runs, cold.runs);
            assert_eq!(
                observed.gaps, cold.gaps,
                "retained discovery lost an ownership dependency"
            );
            assert!(!observed.gaps.is_empty());
            break;
        }
        assert!(Instant::now() < deadline, "{observed:?}");
    }
    fs::write(&receipt, &bytes).unwrap();
    visible(&mut reader, &store, std::slice::from_ref(&id)).await;
    drop(client);
    visible(&mut reader, &store, &[]).await;
    visible(&mut reader, &store, &[]).await;
    assert_eq!(
        reader.cost.directories, 0,
        "native history was rediscovered on a warm read"
    );
    assert_eq!(
        reader.cost.receipts, 2,
        "only the retained capture is reread; the dead native receipt is not"
    );
    fs::write(&receipt, b"{").unwrap();
    let corrupt = reader.observe(&store, None).await;
    assert!(!corrupt.gaps.is_empty());
    fs::write(&receipt, bytes).unwrap();
    visible(&mut reader, &store, &[]).await;
}

#[tokio::test]
async fn manifest_failure_does_not_outlive_its_native_client() {
    let home = tempfile::tempdir().unwrap();
    let store = store(home.path()).await;
    let id = prepare(home.path());
    let (dir, _) = resolve_manifest(home.path(), id.as_str()).unwrap();
    let client = Client::start();
    write_provider_client(&dir, client.0.id()).unwrap();
    let mut reader = ActiveRunReader::start(home.path(), true, CancellationToken::new()).unwrap();
    visible(&mut reader, &store, std::slice::from_ref(&id)).await;
    let manifest = dir.join("manifest.json");
    let bytes = fs::read(&manifest).unwrap();
    fs::write(&manifest, b"{").unwrap();
    assert!(!reader.observe(&store, None).await.gaps.is_empty());
    drop(client);
    fs::write(&manifest, bytes).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let observed = reader.observe(&store, None).await;
        if observed.discovery == DiscoveryState::Ready {
            let cold = crate::run_record::active::snapshot(home.path(), &store, None).await;
            assert_eq!(observed.runs, cold.runs);
            assert_eq!(
                observed.gaps, cold.gaps,
                "stale manifest failure survived repair and client exit"
            );
            assert!(observed.gaps.is_empty());
            break;
        }
        assert!(Instant::now() < deadline, "{observed:?}");
    }
}

#[tokio::test]
#[ignore = "explicit discovery cost collection: creates 100,000 historical Runs"]
async fn discovery_cost_matrix() {
    let home = tempfile::tempdir().unwrap();
    let store = store(home.path()).await;
    let live_id = prepare(home.path());
    let (live_dir, mut manifest) = resolve_manifest(home.path(), live_id.as_str()).unwrap();
    let client = Client::start();
    write_provider_client(&live_dir, client.0.id()).unwrap();
    let _binding = crate::run_record::active::RunBindingGuard::publish_at(
        home.path(),
        super::RunBinding {
            run_id: live_id.clone(),
            owner: None,
        },
    )
    .unwrap();
    manifest.created_at = time::OffsetDateTime::UNIX_EPOCH;
    let stale = crate::run_record::ProviderClientRef {
        schema_version: 1,
        pid: 4_000_000,
        terminal_id: None,
        started_at: time::OffsetDateTime::UNIX_EPOCH,
    };
    let native = serde_json::to_vec(&stale).unwrap();
    let owner = crate::durable::TaskWorkerOwner {
        trace_id: crate::id::TraceId::new(),
        exec_id: crate::id::ExecId::new(),
        pid: stale.pid,
        started_at: 0,
    };
    let exec = serde_json::to_vec(&crate::journal::ExecProcessReceipt {
        schema_version: 1,
        trace_id: owner.trace_id.to_string(),
        exec_id: owner.exec_id.to_string(),
        pid: owner.pid,
        started_at: owner.started_at,
    })
    .unwrap();
    fs::create_dir_all(home.path().join("runtime/exec-processes")).unwrap();
    fs::create_dir_all(home.path().join("run-bindings")).unwrap();
    let mut previous = 0;
    for population in [100, 10_000, 100_000] {
        for index in previous..population {
            let id = RunId::parse(&format!("run_{index:032x}")).unwrap();
            let dir = crate::run_record::record_dir(home.path(), &id).unwrap();
            fs::create_dir_all(dir.join("provider-clients")).unwrap();
            manifest.run_id = id.clone();
            fs::write(
                dir.join("manifest.json"),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            fs::write(dir.join("provider-clients/dead.json"), &native).unwrap();
            fs::write(
                home.path()
                    .join(format!("runtime/exec-processes/{index}.json")),
                &exec,
            )
            .unwrap();
            let binding = super::RunBinding {
                run_id: id,
                owner: Some(owner.clone()),
            };
            fs::write(
                home.path().join(format!("run-bindings/{index}.json")),
                serde_json::to_vec(&binding).unwrap(),
            )
            .unwrap();
        }
        previous = population;
        for mode in ["one_shot", "cold"] {
            let start = Measurement::start();
            let mut reader =
                ActiveRunReader::start(home.path(), mode == "cold", CancellationToken::new())
                    .unwrap();
            let snapshot = reader.observe(&store, None).await;
            report(population, mode, 0, start, &reader);
            assert_eq!(snapshot.discovery, DiscoveryState::Ready, "{snapshot:?}");
            assert!(snapshot.gaps.is_empty(), "{:?}", snapshot.gaps);
            assert_eq!(snapshot.runs.len(), 1);
            if mode == "one_shot" {
                continue;
            }
            for sample in 0..20 {
                let start = Measurement::start();
                let snapshot = reader.observe(&store, None).await;
                report(population, "warm", sample, start, &reader);
                assert_eq!(snapshot.discovery, DiscoveryState::Ready);
                assert!(snapshot.gaps.is_empty(), "{:?}", snapshot.gaps);
                assert_eq!(snapshot.runs[0].id, live_id);
                assert_eq!(reader.cost.directories, 0);
                assert_eq!(reader.cost.rescans, 0);
                assert_eq!(
                    reader.cost.receipts, 4,
                    "only the live receipt and ownerless capture are reread before/after projection"
                );
                assert_eq!(reader.cost.retained, 2);
            }
            let old = RunId::parse("run_00000000000000000000000000000000").unwrap();
            let dir = crate::run_record::record_dir(home.path(), &old).unwrap();
            let resumed = Client::start();
            let start = Measurement::start();
            write_provider_client(&dir, resumed.0.id()).unwrap();
            let mut attempt = 0;
            loop {
                let snapshot = reader.observe(&store, None).await;
                report(population, "resume", attempt, start, &reader);
                assert_eq!(
                    reader.cost.rescans, 0,
                    "publication triggered full history discovery"
                );
                if snapshot.discovery == DiscoveryState::Ready {
                    assert!(snapshot.gaps.is_empty(), "{snapshot:?}");
                    assert_eq!(snapshot.runs.len(), 2);
                    assert!(snapshot.runs.iter().any(|r| r.id == old));
                    break;
                }
                assert!(start.elapsed() < Duration::from_secs(5), "{snapshot:?}");
                attempt += 1;
            }
            reader.pending.invalidate();
            let start = Measurement::start();
            let snapshot = reader.observe(&store, None).await;
            report(population, "loss_recovery", 0, start, &reader);
            assert_eq!(snapshot.discovery, DiscoveryState::Ready);
            assert_eq!(snapshot.runs.len(), 2);
            assert_eq!(reader.cost.rescans, 1);
            if population == 100_000 {
                // Publish after process sampling while a real long scan is
                // in flight. Preserve the timing so this cannot pass merely
                // by publishing before subscription or after enumeration.
                let racing_id = RunId::parse("run_00000000000000000000000000000001").unwrap();
                let racing_dir = crate::run_record::record_dir(home.path(), &racing_id).unwrap();
                reader.invalidate();
                let start = Measurement::start();
                let (started, scanning) = std::sync::mpsc::sync_channel(1);
                reader.scan_started = Some(started);
                let publisher = std::thread::spawn(move || {
                    scanning.recv_timeout(Duration::from_secs(5)).unwrap();
                    let child = Client::start();
                    write_provider_client(&racing_dir, child.0.id()).unwrap();
                    (child, Instant::now())
                });
                let during = reader.observe(&store, None).await;
                let end = Instant::now();
                let (_child, publication) = publisher.join().unwrap();
                assert!(publication > start.instant && publication < end);
                report(population, "cold_race", 0, start, &reader);
                assert_eq!(during.discovery, DiscoveryState::Scanning, "{during:?}");
                assert!(during.runs.is_empty());
                visible(&mut reader, &store, &[live_id.clone(), old, racing_id]).await;
                reader.invalidate();
                let cancel = reader.cancel.clone();
                let (started, scanning) = std::sync::mpsc::sync_channel(1);
                reader.scan_started = Some(started);
                let interrupt = std::thread::spawn(move || {
                    scanning.recv_timeout(Duration::from_secs(5)).unwrap();
                    cancel.cancel();
                });
                let start = Measurement::start();
                let cancelled = reader.observe(&store, None).await;
                interrupt.join().unwrap();
                report(population, "cancel_scan", 0, start, &reader);
                assert_eq!(cancelled.discovery, DiscoveryState::Unavailable);
                assert!(
                    reader.cost.directories > 0,
                    "cancellation must interrupt traversal"
                );
                assert!(start.elapsed() < Duration::from_secs(5));
            }
        }
    }
}

fn report(
    population: usize,
    mode: &str,
    sample: usize,
    start: Measurement,
    reader: &ActiveRunReader,
) {
    let usage = usage();
    // SAFETY: proc_pidinfo receives the correct sized, initialized taskinfo
    // buffer for this process; its return value establishes successful fill.
    let rss = unsafe {
        let mut task: libc::proc_taskinfo = std::mem::zeroed();
        let size = std::mem::size_of_val(&task) as i32;
        (libc::proc_pidinfo(
            std::process::id() as i32,
            libc::PROC_PIDTASKINFO,
            0,
            (&mut task as *mut libc::proc_taskinfo).cast(),
            size,
        ) == size)
            .then_some(task.pti_resident_size)
    };
    eprintln!(
        "DISCOVERY_COST {}",
        serde_json::json!({
            "population": population, "mode": mode, "sample": sample,
            "wall_ms": start.elapsed().as_secs_f64() * 1000.0,
            "cpu_user_us": usage.ru_utime.tv_sec * 1_000_000 + usage.ru_utime.tv_usec as i64 - start.user_us,
            "cpu_system_us": usage.ru_stime.tv_sec * 1_000_000 + usage.ru_stime.tv_usec as i64 - start.system_us,
            "resident_bytes": rss,
            "process_peak_rss_bytes": usage.ru_maxrss,
            "cost": reader.cost,
        })
    );
}

#[derive(Clone, Copy)]
struct Measurement {
    instant: Instant,
    user_us: i64,
    system_us: i64,
}
impl Measurement {
    fn start() -> Self {
        let usage = usage();
        Self {
            instant: Instant::now(),
            user_us: usage.ru_utime.tv_sec * 1_000_000 + usage.ru_utime.tv_usec as i64,
            system_us: usage.ru_stime.tv_sec * 1_000_000 + usage.ru_stime.tv_usec as i64,
        }
    }
    fn elapsed(self) -> Duration {
        self.instant.elapsed()
    }
}
fn usage() -> libc::rusage {
    // SAFETY: getrusage writes one initialized rusage value to this pointer.
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        assert_eq!(libc::getrusage(libc::RUSAGE_SELF, &mut usage), 0);
        usage
    }
}
