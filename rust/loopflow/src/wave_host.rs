//! Wave listener ownership embedded in the machine-local `lfd` server.

use std::collections::{HashMap, HashSet};
use std::future::pending;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tokio::sync::{watch, Mutex};

use crate::durable::{Containment, ContainmentObservation, HomeId, RunState, WorkRef};
use crate::id::WaveId;
use crate::store::SharedStore;
use crate::wave::{self, registry, Wave};

pub(crate) const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
enum WaveStartup {
    Starting,
    Live(String),
    Failed(String),
}

#[derive(Debug)]
struct HostedWave {
    task: tokio::task::JoinHandle<()>,
    startup: watch::Receiver<WaveStartup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum WaveStartState {
    Live { endpoint: String },
    Failed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WaveStartOutcome {
    pub wave_id: WaveId,
    #[serde(flatten)]
    pub state: WaveStartState,
}

#[derive(Debug, Clone)]
pub(crate) struct WaveHost {
    home_id: HomeId,
    store: SharedStore,
    waves: Arc<Mutex<HashMap<WaveId, HostedWave>>>,
    suppressed: Arc<Mutex<HashSet<WaveId>>>,
    discord_token: Option<SecretString>,
}

impl WaveHost {
    pub(crate) fn new(
        home_id: HomeId,
        store: SharedStore,
        discord_token: Option<SecretString>,
    ) -> Self {
        Self {
            home_id,
            store,
            waves: Arc::new(Mutex::new(HashMap::new())),
            suppressed: Arc::new(Mutex::new(HashSet::new())),
            discord_token,
        }
    }

    pub(crate) fn home_id(&self) -> &HomeId {
        &self.home_id
    }

    pub(crate) async fn active_count(&self) -> usize {
        self.waves
            .lock()
            .await
            .values()
            .filter(|wave| !wave.task.is_finished())
            .count()
    }

    pub(crate) async fn reconcile(&self) {
        let assigned = match waves_for_home(&self.store, &self.home_id, None).await {
            Ok(waves) => waves,
            Err(error) => {
                tracing::error!(%error, "could not select Waves assigned to this Home");
                return;
            }
        };
        for wave in assigned {
            if self.suppressed.lock().await.contains(wave.id()) {
                continue;
            }
            if let Err(error) = self.start_wave(wave.id()).await {
                tracing::error!(wave = wave.name(), %error, "assigned Wave failed to start");
            }
        }
    }

    pub(crate) async fn reconcile_forever(&self) {
        loop {
            self.reconcile().await;
            tokio::time::sleep(RECONCILE_INTERVAL).await;
        }
    }

    pub(crate) async fn start_waves(&self, wave_ids: Vec<WaveId>) -> Vec<WaveStartOutcome> {
        {
            let mut suppressed = self.suppressed.lock().await;
            for wave_id in &wave_ids {
                suppressed.remove(wave_id);
            }
        }
        let mut outcomes = Vec::with_capacity(wave_ids.len());
        for wave_id in wave_ids {
            let state = match self.start_wave(&wave_id).await {
                Ok(endpoint) => WaveStartState::Live { endpoint },
                Err(error) => WaveStartState::Failed {
                    reason: error.to_string(),
                },
            };
            outcomes.push(WaveStartOutcome { wave_id, state });
        }
        outcomes
    }

    pub(crate) async fn stop_wave(&self, wave_id: &WaveId) -> Result<bool> {
        self.suppressed.lock().await.insert(wave_id.clone());
        let wave = self
            .store
            .get_wave(wave_id)
            .await?
            .ok_or_else(|| anyhow!("Wave {wave_id} was not found"))?;
        let requested = wave::request_stop(Path::new(wave.repo()), wave.name()).await?;
        let hosted = self.waves.lock().await.remove(wave_id);
        if let Some(mut hosted) = hosted {
            if tokio::time::timeout(Duration::from_secs(1), &mut hosted.task)
                .await
                .is_err()
            {
                hosted.task.abort();
            }
        }
        Ok(requested)
    }

    async fn start_wave(&self, wave_id: &WaveId) -> Result<String> {
        let wave = self
            .store
            .get_wave(wave_id)
            .await?
            .ok_or_else(|| anyhow!("Wave {wave_id} was not found"))?;
        let placement = self
            .store
            .placement(&WorkRef::Wave(wave_id.clone()))
            .await?;
        if placement.home_id != self.home_id {
            return Err(anyhow!(
                "Wave {} is placed on {}, not resident Home {}",
                wave.name(),
                placement.home_id,
                self.home_id
            ));
        }

        let repo = PathBuf::from(wave.repo());
        if let Some(endpoint) = wave::server::live_endpoint(&repo, wave.name()).await {
            drain_observations(&endpoint, wave.name()).await?;
            return Ok(endpoint);
        }
        crate::engine::process::resolve_current_home_lf_binary_checked().map_err(|error| {
            anyhow!(
                "Wave {} cannot start its resident on this Home: {error}",
                wave.name()
            )
        })?;

        let mut tasks = self.waves.lock().await;
        if let Some(hosted) = tasks
            .get(wave_id)
            .filter(|hosted| !hosted.task.is_finished())
        {
            let startup = hosted.startup.clone();
            drop(tasks);
            let endpoint = wait_for_startup(startup, wave.name()).await?;
            if let Err(error) = drain_observations(&endpoint, wave.name()).await {
                self.abort_failed_start(wave_id, &wave).await;
                return Err(error);
            }
            return Ok(endpoint);
        }
        if let Some(hosted) = tasks.remove(wave_id) {
            hosted.task.abort();
        }
        self.reconcile_run_slot(&wave).await?;
        let config = registry::RegistryConfig {
            store: self.store.clone(),
            wave: wave.clone(),
        };
        let name = wave.name().to_string();
        let task_name = name.clone();
        let listener_repo = repo.clone();
        let discord_token = self.discord_token.clone();
        let (published, published_rx) = tokio::sync::oneshot::channel();
        let (startup_tx, startup_rx) = watch::channel(WaveStartup::Starting);
        let task = tokio::spawn(async move {
            let listener = wave::run_listener_with_startup(
                listener_repo,
                task_name.clone(),
                Some(config),
                false,
                true,
                discord_token,
                wave::ListenerSignals::new(Some(published), pending()),
            );
            tokio::pin!(listener);
            tokio::select! {
                result = &mut listener => {
                    let reason = match result {
                        Ok(()) => format!("Wave {task_name} stopped before publishing a live endpoint"),
                        Err(error) => format!("Wave {task_name} failed preflight: {error}"),
                    };
                    startup_tx.send_replace(WaveStartup::Failed(reason.clone()));
                    tracing::error!(wave = task_name, error = reason, "Wave listener stopped during startup");
                }
                published = published_rx => {
                    match published {
                        Ok(endpoint) => {
                            startup_tx.send_replace(WaveStartup::Live(endpoint));
                            if let Err(error) = listener.await {
                                tracing::error!(wave = task_name, %error, "Wave listener stopped");
                            }
                        }
                        Err(_) => {
                            startup_tx.send_replace(WaveStartup::Failed(format!(
                                "Wave {task_name} stopped before publishing a live endpoint"
                            )));
                        }
                    }
                }
            }
        });
        tasks.insert(
            wave_id.clone(),
            HostedWave {
                task,
                startup: startup_rx.clone(),
            },
        );
        drop(tasks);
        let endpoint = match wait_for_startup(startup_rx, &name).await {
            Ok(endpoint) => endpoint,
            Err(error) => {
                self.remove_failed_wave(wave_id).await;
                return Err(error);
            }
        };
        if let Err(error) = drain_observations(&endpoint, &name).await {
            self.abort_failed_start(wave_id, &wave).await;
            return Err(error);
        }
        Ok(endpoint)
    }

    async fn abort_failed_start(&self, wave_id: &WaveId, wave: &Wave) {
        if let Err(error) = wave::request_stop(Path::new(wave.repo()), wave.name()).await {
            tracing::warn!(wave = wave.name(), %error, "could not stop Wave after failed wake");
        }
        if let Some(hosted) = self.waves.lock().await.remove(wave_id) {
            hosted.task.abort();
        }
    }

    async fn remove_failed_wave(&self, wave_id: &WaveId) {
        let mut waves = self.waves.lock().await;
        let failed = waves.get(wave_id).is_some_and(|hosted| {
            hosted.task.is_finished() || matches!(&*hosted.startup.borrow(), WaveStartup::Failed(_))
        });
        if failed {
            if let Some(hosted) = waves.remove(wave_id) {
                hosted.task.abort();
            }
        }
    }

    async fn reconcile_run_slot(&self, wave: &Wave) -> Result<()> {
        let work = WorkRef::Wave(wave.id().clone());
        let Some(run) = self.store.current_run(&work).await? else {
            return Ok(());
        };
        let observation = match &run.containment {
            Some(Containment::ProcessGroup { id }) => {
                crate::engine::process::process_group_observation(*id)
            }
            Some(Containment::Tmux { .. }) => ContainmentObservation::Unprovable,
            None if run.state == RunState::Reserved
                && run.created_at + time::Duration::seconds(10)
                    <= time::OffsetDateTime::now_utc() =>
            {
                ContainmentObservation::Absent
            }
            None => ContainmentObservation::Unprovable,
        };
        if observation != ContainmentObservation::Absent {
            return Err(anyhow!(
                "Wave {} already has Run {} in {:?}; containment is {:?}",
                wave.name(),
                run.id,
                run.state,
                observation
            ));
        }
        self.store
            .recover_run(&run.id, ContainmentObservation::Absent)
            .await?;
        tracing::warn!(
            wave = wave.name(),
            run = %run.id,
            "recovered Wave Run after its containment disappeared"
        );
        Ok(())
    }

    pub(crate) async fn shutdown(&self) {
        let wave_ids = self.waves.lock().await.keys().cloned().collect::<Vec<_>>();
        for wave_id in wave_ids {
            let Ok(Some(wave)) = self.store.get_wave(&wave_id).await else {
                continue;
            };
            if let Err(error) = wave::request_stop(Path::new(wave.repo()), wave.name()).await {
                tracing::warn!(wave = wave.name(), %error, "failed to stop Wave during Home shutdown");
            }
        }
        let mut waves = self.waves.lock().await;
        for hosted in waves.values_mut() {
            if !hosted.task.is_finished() {
                hosted.task.abort();
            }
        }
        waves.clear();
    }
}

async fn wait_for_startup(mut startup: watch::Receiver<WaveStartup>, name: &str) -> Result<String> {
    tokio::time::timeout(STARTUP_TIMEOUT, async {
        loop {
            let state = startup.borrow().clone();
            match state {
                WaveStartup::Starting => startup.changed().await.map_err(|_| {
                    anyhow!("Wave {name} stopped before publishing a live endpoint")
                })?,
                WaveStartup::Live(endpoint) => return Ok(endpoint),
                WaveStartup::Failed(reason) => return Err(anyhow!(reason)),
            }
        }
    })
    .await
    .map_err(|_| anyhow!("Wave {name} did not publish a live endpoint within 10s"))?
}

async fn drain_observations(endpoint: &str, name: &str) -> Result<()> {
    let response = reqwest::Client::new()
        .post(format!("http://{endpoint}/observations"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|error| anyhow!("Wave {name} became live but its durable wake failed: {error}"))?;
    if response.status() == reqwest::StatusCode::NO_CONTENT {
        Ok(())
    } else {
        Err(anyhow!(
            "Wave {name} became live but its durable wake was refused with HTTP {}: {}",
            response.status(),
            response.text().await.unwrap_or_default()
        ))
    }
}

pub(crate) async fn waves_for_home(
    store: &SharedStore,
    home_id: &HomeId,
    repo: Option<&str>,
) -> Result<Vec<Wave>> {
    let machine = crate::engine::machine::MachineIdentity::detect(home_id.clone());
    let mut assigned = Vec::new();
    for wave in store.list_waves(repo).await? {
        let config =
            crate::engine::wave_config::read_wave_config(Path::new(wave.repo()), wave.name());
        let decision = crate::engine::machine::wave_start_decision(config.as_ref(), &machine);
        if !decision.should_start() {
            tracing::info!(wave = wave.name(), reason = %decision, "skipping Wave at Home startup");
            continue;
        }
        let placement = match store.placement(&WorkRef::Wave(wave.id().clone())).await {
            Ok(placement) => placement,
            Err(error) => {
                tracing::warn!(wave = wave.name(), %error, "skipping Wave with no readable Home placement");
                continue;
            }
        };
        if placement.home_id != *home_id {
            tracing::info!(
                wave = wave.name(),
                placed_home = %placement.home_id,
                local_home = %home_id,
                "skipping Wave placed on another Home"
            );
            continue;
        }
        assigned.push(wave);
    }
    Ok(assigned)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::sync::Arc;

    use crate::durable::{Containment, HomeId, RunAdvance, RunState, RunTrigger, WorkRef};
    use crate::id::WaveId;
    use crate::store::{open_store, StorageConfig};
    use crate::wave::Wave;

    use super::{waves_for_home, WaveHost, WaveStartState};

    struct EnvRestore(Vec<(&'static str, Option<OsString>)>);

    impl EnvRestore {
        fn capture(names: &[&'static str]) -> Self {
            Self(
                names
                    .iter()
                    .map(|name| (*name, std::env::var_os(name)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (name, value) in &self.0 {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    async fn wave_host_with_run(
        process_group: i64,
    ) -> (tempfile::TempDir, WaveHost, Wave, crate::durable::RunId) {
        let directory = tempfile::tempdir().expect("create temp directory");
        let repo = directory.path().join("repo");
        std::fs::create_dir_all(repo.join("wave/product")).expect("create Wave directory");
        std::fs::write(repo.join("wave/product/GOAL.md"), "Product Wave.\n")
            .expect("write Wave goal");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
                .await
                .expect("open store"),
        );
        let local = store.local_home().await.expect("read local Home");
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            repo.display().to_string(),
        );
        store.create_wave(&wave).await.expect("create Wave");
        let work = WorkRef::Wave(wave.id().clone());
        let (run, lease) = store
            .reserve_run(&work, RunTrigger::User)
            .await
            .expect("reserve Wave Run");
        store
            .advance_run(
                &lease,
                RunAdvance::RunStarting {
                    containment: Containment::ProcessGroup { id: process_group },
                    cwd: repo,
                },
            )
            .await
            .expect("start Wave Run");
        (
            directory,
            WaveHost::new(local.id, store, None),
            wave,
            run.id,
        )
    }

    #[tokio::test]
    async fn home_start_selects_only_assigned_and_locally_placed_waves() {
        let directory = tempfile::tempdir().expect("create temp directory");
        let repo = directory.path().join("repo");
        std::fs::create_dir_all(repo.join("wave/matching")).expect("create matching Wave");
        std::fs::create_dir_all(repo.join("wave/other-home")).expect("create other Home Wave");
        std::fs::create_dir_all(repo.join("wave/remote-placement"))
            .expect("create remotely placed Wave");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
                .await
                .expect("open store"),
        );
        let local = store.local_home().await.expect("read local Home");
        std::fs::write(
            repo.join("wave/matching/GOAL.md"),
            format!("---\nhome: {}\n---\nAssigned here.\n", local.id),
        )
        .expect("write matching policy");
        std::fs::write(
            repo.join("wave/other-home/GOAL.md"),
            "---\nhome: other.example.com\n---\nAssigned elsewhere.\n",
        )
        .expect("write other Home policy");
        std::fs::write(
            repo.join("wave/remote-placement/GOAL.md"),
            "No machine policy.\n",
        )
        .expect("write unassigned policy");

        for name in ["matching", "other-home", "remote-placement"] {
            store
                .create_wave(&Wave::new(
                    WaveId::new(),
                    name.to_string(),
                    repo.display().to_string(),
                ))
                .await
                .expect("create Wave");
        }
        let remote = store
            .observe_home(&HomeId::new(), "ssh://operator@remote.example.com")
            .await
            .expect("observe remote Home");
        let remote_wave = store
            .get_wave_by_name("remote-placement")
            .await
            .expect("read remote Wave")
            .expect("remote Wave exists");
        store
            .place_work(&WorkRef::Wave(remote_wave.id().clone()), &remote.id)
            .await
            .expect("place Wave remotely");

        let selected = waves_for_home(&store, &local.id, None)
            .await
            .expect("select assigned Waves");

        assert_eq!(
            selected.iter().map(|wave| wave.name()).collect::<Vec<_>>(),
            vec!["matching"]
        );
    }

    #[tokio::test]
    async fn reconciliation_respects_manual_stop_suppression() {
        let directory = tempfile::tempdir().expect("create temp directory");
        let repo = directory.path().join("repo");
        std::fs::create_dir_all(repo.join("wave/assigned")).expect("create Wave directory");
        std::fs::write(repo.join("wave/assigned/GOAL.md"), "Assigned here.\n")
            .expect("write Wave goal");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
                .await
                .expect("open store"),
        );
        let local = store.local_home().await.expect("read local Home");
        let wave = Wave::new(
            WaveId::new(),
            "assigned".to_string(),
            repo.display().to_string(),
        );
        store.create_wave(&wave).await.expect("create Wave");
        let host = WaveHost::new(local.id, store, None);
        host.suppressed.lock().await.insert(wave.id().clone());

        host.reconcile().await;

        assert!(crate::wave::server::live_endpoint(&repo, wave.name())
            .await
            .is_none());
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn wave_start_returns_the_listener_preflight_error() {
        let _env_lock = crate::journal::test_env_lock();
        let _restore = EnvRestore::capture(&["LF_BIN", crate::wave::discord::TOKEN_ENV]);
        std::env::set_var(
            "LF_BIN",
            std::env::current_exe().expect("resolve test executable"),
        );
        std::env::remove_var(crate::wave::discord::TOKEN_ENV);
        let directory = tempfile::tempdir().expect("create temp directory");
        let repo = directory.path().join("repo");
        std::fs::create_dir_all(repo.join("wave/product")).expect("create Wave directory");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
                .await
                .expect("open store"),
        );
        let local = store.local_home().await.expect("read local Home");
        std::fs::write(
            repo.join("wave/product/GOAL.md"),
            format!(
                "---\nchat:\n  provider: discord\n  home_id: \"{}\"\n  guild_id: \"guild\"\n  channel_id: \"channel\"\n---\nDiscord-backed product.\n",
                local.id
            ),
        )
        .expect("write Wave goal");
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            repo.display().to_string(),
        );
        store.create_wave(&wave).await.expect("create Wave");
        let host = WaveHost::new(local.id, store, None);

        let outcomes = host.start_waves(vec![wave.id().clone()]).await;
        let WaveStartState::Failed { reason } = &outcomes[0].state else {
            panic!("Discord Wave without a token must fail")
        };

        assert!(
            reason.contains(crate::wave::discord::TOKEN_ENV),
            "startup should preserve the actionable listener error: {reason}"
        );
    }

    #[tokio::test]
    async fn restart_releases_a_wave_run_with_a_gone_process_group() {
        let absent_group = i64::from(i32::MAX);
        assert_eq!(
            crate::engine::process::process_group_observation(absent_group),
            crate::durable::ContainmentObservation::Absent
        );
        let (_directory, host, wave, prior_run_id) = wave_host_with_run(absent_group).await;

        host.reconcile_run_slot(&wave)
            .await
            .expect("recover stale Wave Run");

        let work = WorkRef::Wave(wave.id().clone());
        assert!(host.store.current_run(&work).await.unwrap().is_none());
        let (next, _) = host
            .store
            .reserve_run(
                &work,
                RunTrigger::Recovery {
                    prior_run_id: prior_run_id.clone(),
                },
            )
            .await
            .expect("reserve replacement Wave Run");
        assert_eq!(next.retry_of, Some(prior_run_id));
    }

    #[tokio::test]
    async fn restart_keeps_a_live_wave_run_fenced() {
        let process_group = crate::engine::process::current_process_group_id()
            .expect("test process has a process group");
        let (_directory, host, wave, run_id) = wave_host_with_run(i64::from(process_group)).await;

        let error = host
            .reconcile_run_slot(&wave)
            .await
            .expect_err("live Wave Run must remain fenced");

        assert!(error.to_string().contains(run_id.as_str()));
        assert!(error.to_string().contains("Present"));
        let current = host
            .store
            .current_run(&WorkRef::Wave(wave.id().clone()))
            .await
            .unwrap()
            .expect("live Run remains current");
        assert_eq!(current.id, run_id);
        assert_eq!(current.state, RunState::Active);
    }

    #[tokio::test]
    async fn restart_keeps_unprovable_wave_containment_fenced() {
        let (_directory, host, wave, run_id) = wave_host_with_run(i64::MAX).await;

        let error = host
            .reconcile_run_slot(&wave)
            .await
            .expect_err("unprovable Wave Run must remain fenced");

        assert!(error.to_string().contains(run_id.as_str()));
        assert!(error.to_string().contains("Unprovable"));
        assert_eq!(
            host.store
                .current_run(&WorkRef::Wave(wave.id().clone()))
                .await
                .unwrap()
                .expect("unprovable Run remains current")
                .id,
            run_id
        );
    }
}
