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

use crate::controller::wave::{self, registry};
use crate::durable::{HomeId, WorkRef};
use crate::id::WaveId;
use crate::store::SharedStore;
use crate::work::wave::Wave;

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
        let desired_ids = assigned
            .iter()
            .map(|wave| wave.id().clone())
            .collect::<HashSet<_>>();
        let hosted_ids = self.waves.lock().await.keys().cloned().collect::<Vec<_>>();
        for wave_id in hosted_ids {
            if desired_ids.contains(&wave_id) {
                continue;
            }
            let wave = match self.store.get_wave(&wave_id).await {
                Ok(Some(wave)) => wave,
                Ok(None) => {
                    tracing::error!(wave_id = %wave_id, "hosted Wave disappeared from the registry");
                    continue;
                }
                Err(error) => {
                    tracing::error!(wave_id = %wave_id, %error, "could not read hosted Wave during reconciliation");
                    continue;
                }
            };
            if let Err(error) = self.stop_wave_runtime(&wave).await {
                tracing::error!(wave = wave.name(), %error, "Wave no longer desired on this Home failed to stop");
            }
        }
        for wave in assigned {
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

    async fn stop_wave_runtime(&self, wave: &Wave) -> Result<bool> {
        let requested = wave::request_stop(Path::new(wave.repo()), wave.name()).await?;
        let hosted = self.waves.lock().await.remove(wave.id());
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
        let placement = self
            .store
            .placement(&WorkRef::Wave(wave_id.clone()))
            .await?;
        if placement.home_id != self.home_id {
            return Err(anyhow!(
                "Wave {} moved to Home {}",
                wave.name(),
                placement.home_id
            ));
        }
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
    let machine = crate::controller::wave::placement::MachineIdentity::detect(home_id.clone());
    let mut assigned = Vec::new();
    for wave in store.list_waves(repo).await? {
        let config = match crate::work::wave::config::try_read_wave_config(
            Path::new(wave.repo()),
            wave.name(),
        ) {
            Ok(config) => config,
            Err(error) => {
                tracing::error!(wave = wave.name(), %error, "skipping Wave with invalid authored policy");
                continue;
            }
        };
        let decision =
            crate::controller::wave::placement::wave_start_decision(config.as_ref(), &machine);
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
    use std::path::Path;
    use std::process::Command;
    use std::sync::Arc;

    use crate::durable::{HomeId, WorkRef};
    use crate::id::WaveId;
    use crate::store::{StorageConfig, WaveLocatorUpdate};
    use crate::work::wave::{Wave, WaveLocator};

    use super::{waves_for_home, HostedWave, WaveHost, WaveStartState, WaveStartup};

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

    fn init_test_git_repo(repo: &Path) {
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Test User"],
            vec!["add", "."],
            vec!["commit", "-m", "initial"],
        ] {
            let output = Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(&args)
                .output()
                .expect("run git");
            assert!(
                output.status.success(),
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[tokio::test]
    async fn home_start_selects_only_assigned_and_locally_placed_waves() {
        let directory = tempfile::tempdir().expect("create temp directory");
        let repo = directory.path().join("repo");
        std::fs::create_dir_all(repo.join("wave/matching")).expect("create matching Wave");
        std::fs::create_dir_all(repo.join("wave/broken")).expect("create broken Wave");
        std::fs::create_dir_all(repo.join("wave/other-home")).expect("create other Home Wave");
        std::fs::create_dir_all(repo.join("wave/off")).expect("create off Wave");
        std::fs::create_dir_all(repo.join("wave/remote-placement"))
            .expect("create remotely placed Wave");
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                directory.path().join("registry.db"),
            ))
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
            repo.join("wave/broken/GOAL.md"),
            "---\nowner: [\n---\nMalformed policy.\n",
        )
        .expect("write broken policy");
        std::fs::write(
            repo.join("wave/other-home/GOAL.md"),
            "---\nhome: other.example.com\n---\nAssigned elsewhere.\n",
        )
        .expect("write other Home policy");
        std::fs::write(
            repo.join("wave/off/GOAL.md"),
            "Explicitly off on this Home.\n",
        )
        .expect("write off policy");
        std::fs::write(
            repo.join("wave/remote-placement/GOAL.md"),
            "No machine policy.\n",
        )
        .expect("write unassigned policy");

        for name in [
            "matching",
            "broken",
            "other-home",
            "off",
            "remote-placement",
        ] {
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
        rusqlite::Connection::open(directory.path().join("registry.db")).unwrap()
            .execute("UPDATE work_placements SET enabled=0 WHERE wave_id IN (SELECT id FROM waves WHERE name='off')", []).unwrap();
        let remote_wave = store
            .get_wave_at(
                &crate::work::wave::WaveLocator::discover(&repo, "remote-placement").unwrap(),
            )
            .await
            .expect("read remote Wave")
            .expect("remote Wave exists");
        store
            .place_work(&WorkRef::Wave(remote_wave.id().clone()), &remote.id)
            .await
            .expect("place Wave remotely");

        // Selection is independent of creation order, including across clock seconds.
        rusqlite::Connection::open(directory.path().join("registry.db"))
            .unwrap()
            .execute(
                "UPDATE waves SET created_at = CASE name WHEN 'off' THEN 2 ELSE 1 END",
                [],
            )
            .unwrap();

        let selected = waves_for_home(&store, &local.id, None)
            .await
            .expect("select assigned Waves");

        let mut names = selected.iter().map(|wave| wave.name()).collect::<Vec<_>>();
        names.sort_unstable();
        assert_eq!(names, vec!["matching", "off"]);
    }

    #[tokio::test]
    async fn reconciliation_stops_a_hosted_wave_after_retirement() {
        let directory = tempfile::tempdir().expect("create temp directory");
        let repo = directory.path().join("repo");
        std::fs::create_dir_all(repo.join("wave/assigned")).expect("create Wave directory");
        std::fs::write(repo.join("wave/assigned/GOAL.md"), "Assigned here.\n")
            .expect("write Wave goal");
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                directory.path().join("registry.db"),
            ))
            .await
            .expect("open store"),
        );
        let local = store.local_home().await.expect("read local Home");
        let target = WaveLocator::discover(&repo, "assigned").expect("discover Wave locator");
        let wave = Wave::new(
            WaveId::new(),
            "assigned".to_string(),
            target.repo().to_string(),
        );
        store.create_wave(&wave).await.expect("create Wave");
        let replacement = Wave::new(
            WaveId::new(),
            "replacement".to_string(),
            target.repo().to_string(),
        );
        store
            .create_wave(&replacement)
            .await
            .expect("create replacement Wave");
        let remote = store
            .observe_home(&HomeId::new(), "ssh://operator@remote.example.com")
            .await
            .expect("observe replacement Home");
        store
            .place_work(&WorkRef::Wave(replacement.id().clone()), &remote.id)
            .await
            .expect("place replacement Wave remotely");
        let host = WaveHost::new(local.id, store.clone(), None);
        let (_startup_tx, startup) =
            tokio::sync::watch::channel(WaveStartup::Live("127.0.0.1:1".to_string()));
        let task = tokio::spawn(std::future::pending());
        host.waves
            .lock()
            .await
            .insert(wave.id().clone(), HostedWave { task, startup });
        store
            .relocate_waves(vec![WaveLocatorUpdate {
                wave_id: replacement.id().clone(),
                expected_repo: replacement.repo().to_string(),
                expected_slug: replacement.name().to_string(),
                target,
                retire_collision: Some(wave.id().clone()),
            }])
            .await
            .expect("retire destination Wave during relocation");

        assert!(waves_for_home(&store, host.home_id(), None)
            .await
            .expect("select assigned Waves after retirement")
            .is_empty());
        host.reconcile().await;

        assert_eq!(host.active_count().await, 0);
        assert!(!host.waves.lock().await.contains_key(wave.id()));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn chat_connection_reports_the_listener_failure() {
        let _env_lock = crate::journal::test_env_lock();
        let _restore =
            EnvRestore::capture(&["LF_BIN", crate::controller::wave::discord::TOKEN_ENV]);
        std::env::set_var(
            "LF_BIN",
            std::env::current_exe().expect("resolve test executable"),
        );
        std::env::remove_var(crate::controller::wave::discord::TOKEN_ENV);
        let directory = tempfile::tempdir().expect("create temp directory");
        let repo = directory.path().join("repo");
        std::fs::create_dir_all(repo.join("wave/product")).expect("create Wave directory");
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                directory.path().join("registry.db"),
            ))
            .await
            .expect("open store"),
        );
        let local = store.local_home().await.expect("read local Home");
        std::fs::write(
            repo.join("wave/product/GOAL.md"),
            "---\nchat:\n  provider: discord\n  guild_id: \"guild\"\n  channel_id: \"channel\"\n---\nDiscord-backed product.\n",
        )
        .expect("write Wave goal");
        init_test_git_repo(&repo);
        let locator = WaveLocator::discover(&repo, "product").expect("discover Wave locator");
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            locator.repo().to_string(),
        );
        store.create_wave(&wave).await.expect("create Wave");
        let host = WaveHost::new(local.id, store.clone(), None);

        let outcomes = host.start_waves(vec![wave.id().clone()]).await;
        let WaveStartState::Failed { reason } = &outcomes[0].state else {
            panic!("Discord Wave without a token must fail")
        };

        assert!(
            reason.contains(crate::controller::wave::discord::TOKEN_ENV),
            "startup should preserve the actionable listener error: {reason}"
        );
    }
}
