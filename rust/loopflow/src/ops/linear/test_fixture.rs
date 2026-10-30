use std::path::PathBuf;
use std::sync::Arc;

use rusqlite::OptionalExtension;
use tempfile::TempDir;

use super::LinearTestContext;
use crate::id::WaveId;
use crate::store::{open_ephemeral_store, CredentialType, ProviderToken, StorageConfig, Store};
use crate::work::wave::Wave;

#[derive(Debug)]
pub(crate) struct Fixture {
    pub(crate) directory: TempDir,
    pub(crate) database: PathBuf,
    pub(crate) store: Arc<Store>,
}

impl Fixture {
    pub(in crate::ops) async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("loopflow.db");
        let store = Arc::new(
            open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
                .await
                .unwrap(),
        );
        Self {
            directory,
            database,
            store,
        }
    }

    pub(super) fn retained_task(
        &self,
        repo: &std::path::Path,
        selector: &str,
    ) -> crate::test_linear_input::IssueFixture {
        loopflow_test_support::retained_task_input(
            &self.database,
            &repo.to_string_lossy(),
            selector,
        )
    }

    pub(super) fn wave_refresh_at(&self, wave: &Wave) -> Option<i64> {
        rusqlite::Connection::open(&self.database)
            .unwrap()
            .query_row(
                "SELECT synced_at FROM pm_wave_sync WHERE wave_id=?1",
                [wave.id().as_str()],
                |r| r.get(0),
            )
            .optional()
            .unwrap()
    }

    pub(in crate::ops) fn context(&self, graphql_url: &str) -> LinearTestContext {
        LinearTestContext {
            path: self.database.clone(),
            store: self.store.clone(),
            graphql_url: graphql_url.into(),
        }
    }

    pub(in crate::ops) async fn seed(&self, expires_at: i64) -> ProviderToken {
        let token = token("A1", "R1", expires_at);
        self.store.upsert_provider_token(&token).await.unwrap();
        token
    }

    pub(in crate::ops) async fn planning_repo(&self) -> (PathBuf, Wave) {
        let repo = self.directory.path().join("repo");
        std::fs::create_dir_all(repo.join(".lf")).unwrap();
        // Local fixture history only; no installed Machine or remote is touched.
        for args in [
            vec!["init", "-q"],
            vec![
                "remote",
                "add",
                "origin",
                "https://github.com/loopflowstudio/fixture.git",
            ],
        ] {
            assert!(std::process::Command::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success());
        }
        std::fs::write(
            repo.join(".lf/config.yaml"),
            "planning:\n  provider: linear\n  team: team-1\n",
        )
        .unwrap();
        std::fs::create_dir_all(repo.join(".lf/wave/product")).unwrap();
        std::fs::write(
            repo.join(".lf/wave/product/GOAL.md"),
            "---\nlinear:\n  initiative: initiative-1\n---\nKeep working.\n",
        )
        .unwrap();
        let repo = std::fs::canonicalize(repo).unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".into(),
            repo.to_string_lossy().into_owned(),
        );
        self.store.create_wave(&wave).await.unwrap();
        self.store
            .sqlite
            .ensure_wave(repo.to_str().unwrap(), "product")
            .unwrap();
        (repo, wave)
    }
}

pub(in crate::ops) fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

pub(super) fn token(access: &str, refresh: &str, expires_at: i64) -> ProviderToken {
    ProviderToken {
        provider: "linear".into(),
        access_token: access.into(),
        refresh_token: Some(refresh.into()),
        oauth_client_id: Some("fixture-client".into()),
        expires_at: Some(expires_at),
        login: Some("fixture".into()),
        updated_at: now(),
        credential_type: CredentialType::OAuth,
    }
}

// Callers hold the shared test environment lock for this guard's lifetime.
pub(in crate::ops) struct PlanningEnvironment(
    Vec<(std::ffi::OsString, Option<std::ffi::OsString>)>,
);

impl PlanningEnvironment {
    pub(in crate::ops) fn isolate() -> Self {
        let mut saved = vec![("PATH".into(), std::env::var_os("PATH"))];
        for (name, value) in
            std::env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("LF_"))
        {
            std::env::remove_var(&name);
            saved.push((name, Some(value)));
        }
        // Status reconciliation checks launch authority, but never launches lf.
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        Self(saved)
    }
}

impl Drop for PlanningEnvironment {
    fn drop(&mut self) {
        for (name, _) in
            std::env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("LF_"))
        {
            std::env::remove_var(name);
        }
        for (name, value) in &self.0 {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

/// Refresh through the actual acquisition path, then retain its input for replay.
pub(crate) async fn refresh_task_input(
    repo: &std::path::Path,
    selector: &str,
    refresh: super::LinearRefresh,
) -> crate::ops::OpsResult<crate::test_linear_input::IssueFixture> {
    let read = super::inspect_task_planning_async(repo, selector, refresh).await?;
    if read.observation.state != crate::store::PlanningState::Available {
        return Err(crate::ops::OpsError::Message(
            read.refresh_error.unwrap_or_else(|| {
                format!(
                    "Task {selector:?} planning is {:?}; refresh planning",
                    read.observation.state
                )
            }),
        ));
    }
    let store = super::planning_store().await?;
    let database = store.sqlite.home_dir().unwrap().join("loopflow.db");
    let scope = crate::repository::CanonicalRepo::discover(repo).unwrap();
    Ok(loopflow_test_support::retained_task_input(
        &database,
        &scope.to_string(),
        selector,
    ))
}

pub(super) async fn capture_inventory(
    repo: &std::path::Path,
    wave: &str,
    context: &super::LinearRepository,
    initiative: &str,
    store: &Store,
) -> crate::ops::OpsResult<crate::test_linear_input::Inventory> {
    let projects =
        super::checked_projects_with_store(repo, context, initiative, wave, store).await?;
    let items = context
        .client
        .list_project_issues(&projects)
        .await
        .map_err(super::linear_error)?;
    Ok(crate::test_linear_input::Inventory { projects, items })
}
