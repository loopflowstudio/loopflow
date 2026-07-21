//! Wave listener ownership embedded in the machine-local `lfd` server.

use std::collections::{HashMap, HashSet};
use std::future::pending;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use secrecy::SecretString;
use tokio::sync::Mutex;

use crate::durable::{Containment, ContainmentObservation, HomeId, RunState, WorkRef};
use crate::id::WaveId;
use crate::store::SharedStore;
use crate::wave::{self, registry, Wave};

pub(crate) const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub(crate) struct WaveHost {
    home_id: HomeId,
    store: SharedStore,
    waves: Arc<Mutex<HashMap<WaveId, tokio::task::JoinHandle<Result<()>>>>>,
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
            .filter(|task| !task.is_finished())
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

    pub(crate) async fn start_waves(&self, wave_ids: Vec<WaveId>) -> Result<()> {
        {
            let mut suppressed = self.suppressed.lock().await;
            for wave_id in &wave_ids {
                suppressed.remove(wave_id);
            }
        }
        let mut errors = Vec::new();
        for wave_id in wave_ids {
            if let Err(error) = self.start_wave(&wave_id).await {
                errors.push(error.to_string());
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(errors.join("; ")))
        }
    }

    pub(crate) async fn stop_wave(&self, wave_id: &WaveId) -> Result<bool> {
        self.suppressed.lock().await.insert(wave_id.clone());
        let wave = self
            .store
            .get_wave(wave_id)
            .await?
            .ok_or_else(|| anyhow!("Wave {wave_id} was not found"))?;
        let requested = wave::request_stop(Path::new(wave.repo()), wave.name()).await?;
        let task = self.waves.lock().await.remove(wave_id);
        if let Some(mut task) = task {
            if tokio::time::timeout(Duration::from_secs(1), &mut task)
                .await
                .is_err()
            {
                task.abort();
            }
        }
        Ok(requested)
    }

    async fn start_wave(&self, wave_id: &WaveId) -> Result<()> {
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
        if wave::server::live_endpoint(&repo, wave.name())
            .await
            .is_some()
        {
            return Ok(());
        }

        let mut tasks = self.waves.lock().await;
        if tasks.get(wave_id).is_some_and(|task| !task.is_finished()) {
            drop(tasks);
            if self
                .wait_for_wave(wave_id, &repo, wave.name())
                .await
                .is_some()
            {
                return Ok(());
            }
            return Err(self
                .startup_failure(wave_id, wave.name())
                .await
                .unwrap_or_else(|| {
                    anyhow!(
                        "Wave {} is starting but did not publish a live endpoint",
                        wave.name()
                    )
                }));
        }
        if let Some(task) = tasks.remove(wave_id) {
            task.abort();
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
        let task = tokio::spawn(async move {
            let result = wave::run_listener(
                listener_repo,
                task_name.clone(),
                Some(config),
                false,
                true,
                discord_token,
                pending(),
            )
            .await;
            if let Err(error) = &result {
                tracing::error!(wave = task_name, %error, "Wave listener stopped");
            }
            result
        });
        tasks.insert(wave_id.clone(), task);
        drop(tasks);
        if self.wait_for_wave(wave_id, &repo, &name).await.is_some() {
            return Ok(());
        }
        Err(self
            .startup_failure(wave_id, &name)
            .await
            .unwrap_or_else(|| anyhow!("Wave {name} did not publish a live endpoint")))
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

    async fn startup_failure(&self, wave_id: &WaveId, name: &str) -> Option<anyhow::Error> {
        let task = {
            let mut tasks = self.waves.lock().await;
            if tasks.get(wave_id).is_some_and(|task| task.is_finished()) {
                tasks.remove(wave_id)
            } else {
                None
            }
        }?;
        Some(match task.await {
            Ok(Ok(())) => anyhow!("Wave {name} stopped before publishing a live endpoint"),
            Ok(Err(error)) => anyhow!("Wave {name} failed to start: {error}"),
            Err(error) => anyhow!("Wave {name} listener task failed: {error}"),
        })
    }

    async fn wait_for_wave(&self, wave_id: &WaveId, repo: &Path, name: &str) -> Option<String> {
        for _ in 0..40 {
            if let Some(endpoint) = wave::server::live_endpoint(repo, name).await {
                return Some(endpoint);
            }
            if self
                .waves
                .lock()
                .await
                .get(wave_id)
                .is_some_and(|task| task.is_finished())
            {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        None
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
        for task in waves.values_mut() {
            if !task.is_finished() {
                task.abort();
            }
        }
        waves.clear();
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

    use super::{waves_for_home, WaveHost};

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

        let error = host
            .start_waves(vec![wave.id().clone()])
            .await
            .expect_err("Discord Wave without a token must fail");

        assert!(
            error.to_string().contains(crate::wave::discord::TOKEN_ENV),
            "startup should preserve the actionable listener error: {error}"
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
