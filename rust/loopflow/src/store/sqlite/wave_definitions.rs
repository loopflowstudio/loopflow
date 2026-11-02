//! Wave registration.

use crate::work::edit::WorkValue as Edit;
use std::path::Path;

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::id::WaveId;
use crate::store::{StoreError, StoreResult};
use crate::work::project::Project;
use crate::work::wave::{Wave, WaveLocator};

use super::{planning_write, SqliteStore};

impl SqliteStore {
    /// Provision a Wave hierarchy without a provider, preserving existing authored files.
    pub fn ensure_wave_project(&self, repo: &str, name: &str) -> StoreResult<Project> {
        let wave = self.ensure_wave(repo, name)?;
        let project = self.ensure_project(&wave, name)?;
        self.project(&project)?.ok_or(StoreError::NotFound)
    }

    pub fn ensure_wave(&self, repo: &str, name: &str) -> StoreResult<WaveId> {
        if name.split('/').any(|part| {
            part.trim().is_empty() || part.contains([':', '\\']) || matches!(part, "." | "..")
        }) {
            return Err(StoreError::InvalidData("invalid Wave name".into()));
        }
        let checkout = Path::new(repo);
        let canonical = crate::repository::CanonicalRepo::discover(checkout)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let repo = canonical.to_string();
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut parent: Option<WaveId> = None;
        let mut prefix = String::new();
        for part in name.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let directory = checkout.join(".lf/wave").join(&prefix);
            let mut authored_id =
                crate::work::wave::config::try_read_wave_config(checkout, &prefix)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?
                    .and_then(|config| config.id);
            // Authored directories match by id first: a checkout that has not yet
            // rebased past a rename or removal neither renames the Wave back nor
            // resurrects a removed identity.
            if let Some(id) = authored_id.clone() {
                let live: Option<bool> = tx
                    .query_row(
                        "SELECT retired_at IS NULL FROM waves WHERE id=?1",
                        [&id],
                        |row| row.get(0),
                    )
                    .optional()?;
                match live {
                    Some(true) => {
                        seed_wave_files(&directory, &id, &prefix)?;
                        parent = Some(id);
                        continue;
                    }
                    Some(false) => {
                        tracing::info!(
                            directory = %directory.display(),
                            "skipping authored Wave directory whose Wave was removed"
                        );
                        authored_id = None;
                    }
                    None => {}
                }
            }
            let existing: Option<WaveId> = tx.query_row(
                "SELECT id FROM waves WHERE repo=?1 AND name=?2 AND parent_wave_id IS ?3 AND retired_at IS NULL",
                params![repo, part, parent], |row| row.get(0)).optional()?;
            let wave = match existing {
                Some(wave) => {
                    if authored_id.as_ref().is_some_and(|id| id != &wave) {
                        return Err(StoreError::InvalidData(format!(
                            "Wave {prefix} is already registered with another id"
                        )));
                    }
                    wave
                }
                None => {
                    let wave = authored_id.unwrap_or_default();
                    planning_write::create(
                        &tx,
                        &repo,
                        &crate::work::edit::WorkEdit::for_target(
                            wave.as_str(),
                            &[
                                Edit::WaveName(part.into()),
                                Edit::WaveParent(parent.as_ref().map(ToString::to_string)),
                            ],
                        ),
                    )?;
                    wave
                }
            };
            seed_wave_files(&directory, &wave, &prefix)?;
            parent = Some(wave);
        }
        tx.commit()?;
        Ok(parent.expect("validated Wave has at least one component"))
    }

    /// Register every authored Wave directory under `<checkout>/.lf/wave/`, matching
    /// by `config.yaml` id first. A directory whose id names a removed Wave is left
    /// alone with its subtree, so a clone or a checkout behind the planning store
    /// never brings a removed Wave back. A directory that cannot be read or
    /// registered is logged and skipped with its subtree; the rest still register.
    pub fn register_authored_waves(&self, repo: &str) -> StoreResult<()> {
        let checkout = Path::new(repo);
        let root = checkout.join(".lf/wave");
        let mut pending = child_directories(&root, "");
        while let Some(slug) = pending.pop() {
            let directory = root.join(&slug);
            if directory.join("GOAL.md").is_file() || directory.join("config.yaml").is_file() {
                let authored = match crate::work::wave::config::try_read_wave_config(
                    checkout, &slug,
                ) {
                    Ok(config) => config.and_then(|config| config.id),
                    Err(error) => {
                        tracing::warn!(slug, %error, "authored Wave directory was not registered");
                        continue;
                    }
                };
                let removed = authored
                    .map(|id| self.get_wave(&id))
                    .transpose()?
                    .flatten()
                    .is_some_and(|wave| wave.is_retired());
                if removed {
                    tracing::info!(
                        slug,
                        "skipping authored Wave directory whose Wave was removed"
                    );
                    continue;
                }
                if let Err(error) = self.ensure_wave(repo, &slug) {
                    tracing::warn!(slug, %error, "authored Wave directory was not registered");
                    continue;
                }
            }
            pending.extend(child_directories(&root, &slug));
        }
        Ok(())
    }

    /// Retire Waves together through the planning writer, so Git planning exchange
    /// carries the removal. Rows, Projects, Tasks and Sessions stay readable;
    /// an already retired Wave keeps its original retirement.
    pub(crate) fn retire_waves(&self, waves: &[WaveId], reason: &str) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let at = crate::store::rows::now_unix();
        for wave in waves {
            let live: bool = tx.query_row(
                "SELECT retired_at IS NULL FROM waves WHERE id=?1",
                [wave],
                |row| row.get(0),
            )?;
            if !live {
                continue;
            }
            planning_write::local(
                &tx,
                &crate::work::edit::WorkEdit::for_target(
                    wave.as_str(),
                    &[Edit::WaveRetired(Some(at))],
                ),
            )?;
            tx.execute(
                "UPDATE waves SET retirement_reason=?2, work_state='abandoned',
                     work_terminal_at=COALESCE(work_terminal_at, ?3)
                 WHERE id=?1",
                params![wave, reason, at],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The most recently retired Wave at this address, if any.
    pub(crate) fn retired_wave_at(&self, locator: &WaveLocator) -> StoreResult<Option<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT id, name, repo, created_at, parent_wave_id, promoted_at,
                    retired_at, superseded_by_wave_id, retirement_reason, slug
             FROM wave_addresses
             WHERE repo=?1 AND slug=?2 AND retired_at IS NOT NULL
             ORDER BY retired_at DESC LIMIT 1",
            params![locator.repo().to_string(), locator.slug()],
            |row| Ok(crate::store::rows::map_wave_row(row)),
        )
        .optional()?
        .transpose()
    }

    /// Wave conversations (not Task Sessions) recorded for these Waves.
    pub(crate) fn wave_session_count(&self, waves: &[WaveId]) -> StoreResult<usize> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut count = 0usize;
        for wave in waves {
            count += conn.query_row(
                "SELECT COUNT(*) FROM agent_sessions WHERE wave_id=?1 AND task_id IS NULL",
                [wave],
                |row| row.get::<_, i64>(0),
            )? as usize;
        }
        Ok(count)
    }
}

/// The directories directly under `prefix`, as slugs. A directory without its own
/// authored files still leads to nested Waves.
fn child_directories(root: &Path, prefix: &str) -> Vec<String> {
    let directory = if prefix.is_empty() {
        root.to_path_buf()
    } else {
        root.join(prefix)
    };
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut slugs: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .filter(|name| !name.starts_with('.'))
        .map(|name| {
            if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            }
        })
        .collect();
    slugs.sort();
    slugs
}

fn seed_wave_files(directory: &Path, id: &WaveId, name: &str) -> StoreResult<()> {
    use std::io::Write;
    std::fs::create_dir_all(directory)
        .map_err(|error| StoreError::InvalidData(format!("{}: {error}", directory.display())))?;
    // A Wave authored before config.yaml keeps its id in GOAL.md frontmatter.
    let config =
        (!directory.join("GOAL.md").exists()).then(|| ("config.yaml", format!("id: {id}\n")));
    for (file, content) in config.into_iter().chain([
        ("GOAL.md", format!("## Objective\n\n{name}\n")),
        ("MEMORY.md", format!("# {name} wave memory\n")),
    ]) {
        let path = directory.join(file);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => file.write_all(content.as_bytes()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && path.is_file() => {
                continue
            }
            Err(error) => Err(error),
        }
        .map_err(|error| StoreError::InvalidData(format!("{}: {error}", path.display())))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::SqliteStore;
    use crate::id::WaveId;
    use crate::work::wave::{Wave, WaveLocator};

    #[test]
    fn wave_files_are_created_without_overwriting_authored_content() {
        let repo = loopflow_test_support::TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let directory = repo.path().join(".lf/wave/tools/parser");
        std::fs::create_dir_all(&directory).unwrap();
        let id = WaveId::new();
        let goal = format!("---\nid: {id}\n---\nKeep tokens λ.\n");
        std::fs::write(directory.join("GOAL.md"), &goal).unwrap();
        assert_eq!(
            store
                .ensure_wave(repo.path().to_str().unwrap(), "tools/parser")
                .unwrap(),
            id
        );
        let parent = crate::work::wave::config::read_wave_config(repo.path(), "tools").unwrap();
        assert!(parent.id.is_some());
        assert!(repo.path().join(".lf/wave/tools/GOAL.md").is_file());
        assert!(!directory.join("config.yaml").exists());
        assert!(directory.join("MEMORY.md").is_file());
        std::fs::write(directory.join("MEMORY.md"), "Edited directly.\n").unwrap();
        assert_eq!(
            store
                .ensure_wave(repo.path().to_str().unwrap(), "tools/parser")
                .unwrap(),
            id
        );
        assert_eq!(
            std::fs::read_to_string(directory.join("GOAL.md")).unwrap(),
            goal
        );
        assert_eq!(
            std::fs::read_to_string(directory.join("MEMORY.md")).unwrap(),
            "Edited directly.\n"
        );
    }

    #[test]
    fn stale_checkouts_match_by_id_and_never_resurrect_a_removed_wave() {
        let repo = loopflow_test_support::TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let root = repo.path().to_str().unwrap();
        let renamed = store.ensure_wave(root, "product").unwrap();
        let removed = store.ensure_wave(root, "archive").unwrap();
        // The store renames `product` and retires `archive`; this checkout still
        // has both old directories.
        {
            let conn = store.conn.lock().unwrap();
            super::super::planning_write::local(
                &conn,
                &crate::work::edit::WorkEdit::for_target(
                    renamed.as_str(),
                    &[crate::work::edit::WorkValue::WaveName("prod".into())],
                ),
            )
            .unwrap();
        }
        store
            .retire_waves(std::slice::from_ref(&removed), "removed")
            .unwrap();

        assert_eq!(store.ensure_wave(root, "product").unwrap(), renamed);
        assert_eq!(store.get_wave(&renamed).unwrap().unwrap().name(), "prod");
        let fresh = store.ensure_wave(root, "archive").unwrap();
        assert_ne!(fresh, removed, "a removed identity is never re-created");
        assert!(store.get_wave(&removed).unwrap().unwrap().is_retired());
    }

    #[test]
    fn registering_a_clones_directories_lists_them_and_skips_a_removed_wave() {
        let repo = loopflow_test_support::TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let root = repo.path().to_str().unwrap();
        // The clone carries a removed Wave's directory and two nobody registered.
        let removed = store.ensure_wave(root, "archive").unwrap();
        store
            .retire_waves(std::slice::from_ref(&removed), "removed")
            .unwrap();
        for slug in ["research", "tools/parser"] {
            let directory = repo.path().join(".lf/wave").join(slug);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join("GOAL.md"),
                format!("## Objective\n\n{slug}\n"),
            )
            .unwrap();
        }
        std::fs::create_dir_all(repo.path().join(".lf/wave/research/metrics")).unwrap();

        store.register_authored_waves(root).unwrap();
        let canonical = crate::repository::CanonicalRepo::discover(repo.path())
            .unwrap()
            .to_string();
        let mut live: Vec<String> = store
            .list_waves(Some(&canonical))
            .unwrap()
            .into_iter()
            .filter(|wave| !wave.is_retired())
            .map(|wave| wave.slug().to_string())
            .collect();
        live.sort();
        assert_eq!(
            live,
            ["research", "tools", "tools/parser"],
            "tools registers through its child"
        );
        assert!(store.get_wave(&removed).unwrap().unwrap().is_retired());
        store.register_authored_waves(root).unwrap();
        assert_eq!(
            store.list_waves(Some(&canonical)).unwrap().len(),
            3,
            "registering again changes nothing"
        );
    }

    #[test]
    fn released_waves_upgrade_and_removal_needs_no_successor() {
        use crate::store::migrations::{apply_before_migration, apply_pending_migrations};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "wave_removal");
        conn.execute_batch(include_str!(
            "../../../tests/fixtures/migrations/wave_removal_released.sql"
        ))
        .unwrap();
        apply_pending_migrations(&conn);

        let old: (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT superseded_by_wave_id, retirement_reason FROM waves WHERE id='wave-old'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(old, (Some("wave-live".into()), Some("relocated".into())));
        let addresses: Vec<String> = conn
            .prepare("SELECT slug FROM wave_addresses WHERE retired_at IS NULL ORDER BY slug")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(addresses, ["product", "product/desktop"]);

        conn.execute("UPDATE waves SET retired_at=9 WHERE id='wave-child'", [])
            .unwrap();
        assert!(
            conn.execute(
                "UPDATE waves SET retirement_reason='removed' WHERE id='wave-live'",
                []
            )
            .is_err(),
            "a live Wave carries no retirement reason"
        );
    }

    #[tokio::test]
    async fn relocation_preserves_definitions_and_refuses_established_collisions_and_cycles() {
        let repo = loopflow_test_support::TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
            home.path().join("store.db"),
        ))
        .await
        .unwrap();
        let canonical = crate::repository::CanonicalRepo::discover(repo.path()).unwrap();
        let id = store
            .sqlite
            .ensure_wave(&canonical.to_string(), "tools/parser")
            .unwrap();
        let root = store
            .get_wave_at(&WaveLocator::new(canonical.clone(), "tools").unwrap())
            .await
            .unwrap()
            .unwrap();
        std::fs::write(
            repo.path().join(".lf/wave/tools/parser/MEMORY.md"),
            "Retained",
        )
        .unwrap();
        let collision = Wave::new(WaveId::new(), "occupied".into(), canonical.to_string());
        store.create_wave(&collision).await.unwrap();
        store
            .sqlite
            .ensure_project(collision.id(), "Existing plan")
            .unwrap();
        assert!(crate::work::wave::relocate::relocate_wave(
            &store,
            root.id(),
            repo.path(),
            None,
            Some("occupied")
        )
        .await
        .is_err());
        assert!(crate::work::wave::relocate::relocate_wave(
            &store,
            root.id(),
            repo.path(),
            None,
            Some("tools/parser/loop")
        )
        .await
        .is_err());
        assert_eq!(
            store.get_wave(&id).await.unwrap().unwrap().slug(),
            "tools/parser"
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join(".lf/wave/tools/parser/MEMORY.md")).unwrap(),
            "Retained"
        );
    }
}
