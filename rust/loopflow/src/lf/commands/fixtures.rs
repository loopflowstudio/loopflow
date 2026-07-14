//! Test fixtures shared by the command modules that talk to a wave: a temp
//! store, a wave row, and a booted HTTP surface. They live here rather than in
//! `loopflow-test-support` because they are made of loopflow's own types, and
//! that crate must not depend back on this one.

use std::path::Path;
use std::sync::Arc;

use crate::lfd::id::LfdId;
use crate::lfd::types::{Wave, WaveStatus};
use crate::lfdb::{open_store, SharedStore, StorageConfig};
use crate::wave::runtime::{InboxItem, WaveRuntime};
use crate::wave::server;

pub(crate) async fn temp_store(dir: &Path) -> SharedStore {
    Arc::new(
        open_store(&StorageConfig::sqlite(dir.join("lfd.db")))
            .await
            .expect("open sqlite store"),
    )
}

pub(crate) fn make_wave(name: &str, repo: &Path, parent: Option<&LfdId>) -> Wave {
    Wave {
        id: LfdId::new(),
        name: name.to_string(),
        goal: "ship-roadmap".to_string(),
        metrics: Vec::new(),
        repo: repo.display().to_string(),
        status: WaveStatus::Idle,
        iteration: 0,
        cycle_start_iteration: 0,
        direction: Vec::new(),
        area: Vec::new(),
        paused: false,
        created_at: Some(time::OffsetDateTime::now_utc()),
        workers: 1,
        parent_wave_id: parent.cloned(),
    }
}

/// Boot the HTTP surface over a runtime (the wave/mod.rs harness pattern).
/// The inbox receiver is subscribed before serving, so a caller can assert on
/// ops the door delivers.
pub(crate) async fn boot_server(
    origin: &Path,
    wave: &str,
) -> (
    String,
    Arc<WaveRuntime>,
    tokio::sync::broadcast::Receiver<InboxItem>,
) {
    let runtime = WaveRuntime::open(wave.to_string(), origin.to_path_buf()).expect("open runtime");
    let inbox_rx = runtime.subscribe_inbox();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = server::router(
        runtime.clone(),
        server::ResidentDoor::new("test-token"),
        None,
        None,
        server::ShutdownDoor::new(),
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    (addr.to_string(), runtime, inbox_rx)
}
