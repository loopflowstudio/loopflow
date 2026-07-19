//! One machine-local keeper process serving every Wave placed on a Home.

use std::collections::HashMap;
use std::future::pending;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::durable::{HomeId, WorkRef};
use crate::engine::process::{resolve_lf_binary, start_lf_session, tmux_session_slug};
use crate::id::WaveId;
use crate::store::{lf_home_dir, open_existing_store, SharedStore};
use crate::wave::{self, registry};

pub(crate) const SUBCOMMAND: &str = "__home-resident";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HomeResidentStatus {
    home_id: HomeId,
    endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StartRequest {
    wave_ids: Vec<WaveId>,
}

#[derive(Clone)]
struct ResidentState {
    home_id: HomeId,
    endpoint: String,
    store: SharedStore,
    waves: Arc<Mutex<HashMap<WaveId, tokio::task::JoinHandle<()>>>>,
}

pub fn run(home_id: &HomeId) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(serve(home_id.clone()))
}

async fn serve(home_id: HomeId) -> Result<()> {
    let store = open_existing_store()
        .await
        .ok_or_else(|| anyhow!("the Home resident needs an initialized local store"))?;
    let local = store.local_home().await?;
    if local.id != home_id {
        return Err(anyhow!(
            "refusing Home resident start: expected {home_id}, local Home is {}",
            local.id
        ));
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = listener.local_addr()?.to_string();
    let state = ResidentState {
        home_id: home_id.clone(),
        endpoint: endpoint.clone(),
        store: Arc::new(store),
        waves: Arc::new(Mutex::new(HashMap::new())),
    };
    write_endpoint(&home_id, &endpoint)?;
    let cleanup_id = home_id.clone();
    let cleanup_endpoint = endpoint.clone();
    crate::engine::agent::register_interrupt_cleanup(move || {
        remove_endpoint(&cleanup_id, &cleanup_endpoint);
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/start", post(start))
        .with_state(state.clone());
    println!("lf Home resident · {home_id} · http://{endpoint}");
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await;

    let wave_ids = state.waves.lock().await.keys().cloned().collect::<Vec<_>>();
    for wave_id in wave_ids {
        let Ok(Some(wave)) = state.store.get_wave(&wave_id).await else {
            continue;
        };
        if let Err(error) = wave::request_stop(Path::new(wave.repo()), wave.name()).await {
            tracing::warn!(wave = wave.name(), %error, "failed to stop Wave during Home shutdown");
        }
    }
    let mut waves = state.waves.lock().await;
    for task in waves.values_mut() {
        if !task.is_finished() {
            task.abort();
        }
    }
    remove_endpoint(&home_id, &endpoint);
    result.map_err(|error| anyhow!("Home resident server error: {error}"))
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    let mut hangup = signal(SignalKind::hangup()).expect("install SIGHUP handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
        _ = hangup.recv() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn health(State(state): State<ResidentState>) -> Json<HomeResidentStatus> {
    Json(HomeResidentStatus {
        home_id: state.home_id,
        endpoint: state.endpoint,
    })
}

async fn start(
    State(state): State<ResidentState>,
    Json(request): Json<StartRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    for wave_id in request.wave_ids {
        let wave = state
            .store
            .get_wave(&wave_id)
            .await
            .map_err(internal_error)?
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    format!("Wave {wave_id} was not found"),
                )
            })?;
        let placement = state
            .store
            .placement(&WorkRef::Wave(wave_id.clone()))
            .await
            .map_err(internal_error)?;
        if placement.home_id != state.home_id {
            return Err((
                StatusCode::CONFLICT,
                format!(
                    "Wave {} is placed on {}, not resident Home {}",
                    wave.name(),
                    placement.home_id,
                    state.home_id
                ),
            ));
        }

        let repo = PathBuf::from(wave.repo());
        if wave::server::live_endpoint(&repo, wave.name())
            .await
            .is_some()
        {
            continue;
        }

        let mut tasks = state.waves.lock().await;
        if tasks.get(&wave_id).is_some_and(|task| !task.is_finished()) {
            drop(tasks);
            wait_for_wave(&repo, wave.name()).await.ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!(
                        "Wave {} is starting but did not publish a live endpoint",
                        wave.name()
                    ),
                )
            })?;
            continue;
        }
        if let Some(task) = tasks.remove(&wave_id) {
            task.abort();
        }
        let config = registry::RegistryConfig {
            store: state.store.clone(),
            wave: wave.clone(),
        };
        let name = wave.name().to_string();
        let task_name = name.clone();
        let listener_repo = repo.clone();
        let task = tokio::spawn(async move {
            if let Err(error) = wave::run_listener(
                listener_repo,
                task_name.clone(),
                Some(config),
                false,
                true,
                pending(),
            )
            .await
            {
                tracing::error!(wave = task_name, %error, "Wave listener stopped");
            }
        });
        tasks.insert(wave_id.clone(), task);
        drop(tasks);
        wait_for_wave(&repo, &name).await.ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Wave {name} did not publish a live endpoint"),
            )
        })?;
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn wait_for_wave(repo: &Path, name: &str) -> Option<String> {
    for _ in 0..40 {
        if let Some(endpoint) = wave::server::live_endpoint(repo, name).await {
            return Some(endpoint);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

fn internal_error(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

pub(crate) async fn ensure(home_id: &HomeId, repo: &Path) -> Result<HomeResidentStatus> {
    if let Some(status) = live(home_id).await {
        return Ok(status);
    }
    let argv = vec![
        resolve_lf_binary().to_string_lossy().to_string(),
        SUBCOMMAND.to_string(),
        home_id.to_string(),
    ];
    let launch = start_lf_session(&session_name(home_id), repo, &argv).await;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Some(status) = live(home_id).await {
            return Ok(status);
        }
    }
    match launch {
        Ok(()) => Err(anyhow!(
            "Home resident {home_id} started but did not publish a live endpoint"
        )),
        Err(error) => Err(anyhow!("failed to start Home resident {home_id}: {error}")),
    }
}

pub(crate) async fn start_waves(home_id: &HomeId, wave_ids: Vec<WaveId>) -> Result<()> {
    let status = live(home_id)
        .await
        .ok_or_else(|| anyhow!("Home resident {home_id} is not running"))?;
    let response = reqwest::Client::new()
        .post(format!("http://{}/start", status.endpoint))
        .json(&StartRequest { wave_ids })
        .send()
        .await?;
    let code = response.status();
    if !code.is_success() {
        return Err(anyhow!(
            "Home resident {home_id} refused start with HTTP {code}: {}",
            response.text().await.unwrap_or_default()
        ));
    }
    Ok(())
}

pub(crate) async fn live(home_id: &HomeId) -> Option<HomeResidentStatus> {
    let endpoint = std::fs::read_to_string(endpoint_path(home_id)).ok()?;
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return None;
    }
    let status = reqwest::Client::new()
        .get(format!("http://{endpoint}/health"))
        .timeout(Duration::from_millis(500))
        .send()
        .await
        .ok()?
        .json::<HomeResidentStatus>()
        .await
        .ok()?;
    (status.home_id == *home_id).then_some(status)
}

fn session_name(home_id: &HomeId) -> String {
    format!("lf-home-{}", tmux_session_slug(home_id.as_str()))
}

fn endpoint_path(home_id: &HomeId) -> PathBuf {
    lf_home_dir()
        .join("residents")
        .join(format!("{}.endpoint", home_id.as_str()))
}

fn write_endpoint(home_id: &HomeId, endpoint: &str) -> Result<()> {
    let path = endpoint_path(home_id);
    let parent = path
        .parent()
        .expect("a Home resident endpoint always has a parent");
    std::fs::create_dir_all(parent)?;
    std::fs::write(path, format!("{endpoint}\n"))?;
    Ok(())
}

fn remove_endpoint(home_id: &HomeId, endpoint: &str) {
    let path = endpoint_path(home_id);
    let owned = std::fs::read_to_string(&path)
        .ok()
        .is_some_and(|value| value.trim() == endpoint);
    if owned {
        let _ = std::fs::remove_file(path);
    }
}
