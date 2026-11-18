use crate::planning_git::PlanningDestination;

use super::{run_sqlite, Store, StoreResult};

pub use loopflow_storage::git_tracker::{GitDestination, GitDestinationConflict, PlanningJournal};

impl Store {
    pub async fn git_journal(
        &self,
        repo: &str,
        active: Option<&str>,
    ) -> StoreResult<Vec<PlanningJournal>> {
        let repo = repo.to_string();
        let active = active.map(str::to_string);
        run_sqlite(&self.sqlite, move |sqlite| {
            sqlite.git_journal(&repo, active.as_deref())
        })
        .await
    }

    pub async fn git_destinations(
        &self,
        repo: &str,
        active: Option<&str>,
    ) -> StoreResult<Vec<GitDestination>> {
        let repo = repo.to_string();
        let active = active.map(str::to_string);
        run_sqlite(&self.sqlite, move |sqlite| {
            sqlite.git_destinations(&repo, active.as_deref())
        })
        .await
    }

    pub async fn provision_planning_user_key(&self, key: &str) -> StoreResult<()> {
        let key = key.to_string();
        run_sqlite(&self.sqlite, move |sqlite| {
            sqlite.provision_planning_user_key(&key)
        })
        .await
    }

    pub async fn planning_user_key(&self) -> StoreResult<Option<String>> {
        run_sqlite(&self.sqlite, move |sqlite| sqlite.planning_user_key()).await
    }

    pub async fn bind_git_destination(
        &self,
        repo: &str,
        destination: &PlanningDestination,
    ) -> StoreResult<String> {
        let repo = repo.to_string();
        let destination = destination.clone();
        run_sqlite(&self.sqlite, move |sqlite| {
            sqlite.bind_git_destination(&repo, &destination)
        })
        .await
    }
}

/// Read the Git tracker configuration and repository identity before the SQL import.
pub fn import_git_plan(
    store: &super::sqlite::SqliteStore,
    repo: &str,
    destination: &str,
    revision: &str,
    incoming: &loopflow_storage::planning_exchange::PlanningSnapshot,
) -> StoreResult<()> {
    if store.git_import_current(repo, destination, revision)? {
        return Ok(());
    }
    let incoming = incoming
        .canonicalized()
        .map_err(|error| super::StoreError::InvalidData(error.to_string()))?;
    incoming
        .validate()
        .map_err(|error| super::StoreError::InvalidData(error.to_string()))?;
    loop {
        if store.repository_id(repo)?.is_none() {
            crate::repository::location::ensure_repository(store, repo)?;
        }
        let inputs = store.repository_registration(repo)?;
        let config = crate::config::placement::global_settings_at(&store.home_dir()?)
            .map_err(|error| super::StoreError::InvalidData(error.to_string()))?;
        let machine_names = config
            .machines
            .into_iter()
            .map(|(name, declaration)| (declaration.id, name))
            .collect();
        let connected = crate::ops::tracker::connected(repo);
        let import = super::sqlite::GitPlanImport {
            repo,
            destination,
            revision,
            incoming: &incoming,
            inputs: &inputs,
            planning_connected: connected,
            machine_names,
        };
        if store.import_prepared_git_plan(&import)? {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::import_git_plan;
    use crate::planning_git::PlanningDestination;
    use crate::repository::location::{
        bind_repository, ensure_repository, locator_directory, read_locator,
    };
    use loopflow_model::durable::RepositoryId;
    use loopflow_storage::planning_exchange::PlanningSnapshot;
    use loopflow_storage::sqlite::GitPlanImport;
    use loopflow_test_support::TestRepo;

    #[test]
    fn first_repository_import_retries_with_the_locator_left_by_a_failed_registration() {
        let home = tempfile::tempdir().unwrap();
        let repo = TestRepo::new();
        let path = repo.path().canonicalize().unwrap();
        let path = path.to_str().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let destination =
            PlanningDestination::new("/unused/fixture", "refs/loopflow/planning/shared/retry")
                .unwrap();
        let route = store.bind_git_destination(path, &destination).unwrap();
        let incoming = PlanningSnapshot::default();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TEMP TRIGGER fail_registration BEFORE INSERT ON repository_plans
             BEGIN SELECT RAISE(ABORT,'registration failed'); END;",
            )
            .unwrap();
        assert!(import_git_plan(&store, path, &route, "one", &incoming).is_err());
        assert!(store.repository_id(path).unwrap().is_none());
        let directory = locator_directory(
            &crate::git::worktrees::git_common_dir(repo.path()).unwrap(),
            &store.repository_registration(path).unwrap().machine,
        );
        let original = read_locator(&directory).unwrap().unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_registration")
            .unwrap();
        import_git_plan(&store, path, &route, "one", &incoming).unwrap();
        assert_eq!(store.repository_id(path).unwrap(), Some(original));
        assert!(store.git_import_current(path, &route, "one").unwrap());
        assert!(store.git_destinations(path, Some(&route)).unwrap()[0].active);
    }

    #[test]
    fn changed_repository_scope_reacquires_before_an_import_is_recorded() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        ensure_repository(&store, "/repo").unwrap();
        let destination =
            PlanningDestination::new("/unused/fixture", "refs/loopflow/planning/shared/scope")
                .unwrap();
        let route = store.bind_git_destination("/repo", &destination).unwrap();
        let inputs = store.repository_registration("/repo").unwrap();
        let incoming = PlanningSnapshot::default();
        let prepared = GitPlanImport {
            inputs: &inputs,
            repo: "/repo",
            destination: &route,
            revision: "one",
            incoming: &incoming,
            planning_connected: false,
            machine_names: Default::default(),
        };
        let later = RepositoryId::new();
        bind_repository(&store, "/repo", &later).unwrap();
        assert!(!store.import_prepared_git_plan(&prepared).unwrap());
        assert!(!store.git_import_current("/repo", &route, "one").unwrap());
        import_git_plan(&store, "/repo", &route, "one", &incoming).unwrap();
        assert_eq!(store.repository_id("/repo").unwrap(), Some(later));
        assert!(store.git_import_current("/repo", &route, "one").unwrap());
    }
}
