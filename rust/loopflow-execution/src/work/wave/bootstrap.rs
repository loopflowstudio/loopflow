//! Authored Wave acquisition and bootstrap, above synchronous storage.

use std::path::Path;

use crate::store::{StoreError, StoreResult};
use loopflow_model::id::WaveId;
use loopflow_storage::sqlite::wave_definitions::WaveBootstrapRow;
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::project::Project;

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

/// Prepare repository registration without extending the Wave's SQL transaction.
pub fn upsert_wave(store: &SqliteStore, wave: &crate::work::wave::Wave) -> StoreResult<()> {
    loop {
        let registration = store
            .repository_id(wave.repo())?
            .is_none()
            .then(|| {
                crate::repository::location::prepare_registration(store, wave.repo(), None, false)
            })
            .transpose()?;
        if store.upsert_wave_prepared(wave, registration.as_ref().map(|r| (&r.inputs, &r.id)))? {
            return Ok(());
        }
    }
}

/// Provision a Wave hierarchy without a provider, preserving authored files.
pub fn ensure_wave_project(store: &SqliteStore, repo: &str, name: &str) -> StoreResult<Project> {
    let wave = ensure_wave(store, repo, name)?;
    let project = store.ensure_project(&wave, name)?;
    store.project(&project)?.ok_or(StoreError::NotFound)
}

pub fn ensure_wave(store: &SqliteStore, repo: &str, name: &str) -> StoreResult<WaveId> {
    ensure_wave_inner(store, repo, name, false)?.ok_or(StoreError::NotFound)
}

fn ensure_wave_inner(
    store: &SqliteStore,
    repo: &str,
    name: &str,
    skip_retired: bool,
) -> StoreResult<Option<WaveId>> {
    if name.split('/').any(|part| {
        part.trim().is_empty() || part.contains([':', '\\']) || matches!(part, "." | "..")
    }) {
        return Err(invalid("invalid Wave name"));
    }
    let checkout = Path::new(repo);
    let repo = crate::repository::discover(checkout)
        .map_err(invalid)?
        .to_string();
    'acquire: loop {
        let mut authored = Vec::new();
        let mut prefix = String::new();
        for part in name.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let id = crate::work::wave::config::try_read_wave_config(checkout, &prefix)
                .map_err(invalid)?
                .and_then(|config| config.id);
            authored.push((prefix.clone(), part.to_owned(), id));
        }
        let ids: Vec<_> = authored
            .iter()
            .filter_map(|(_, _, id)| id.clone())
            .collect();
        let inputs = store.wave_bootstrap_inputs(&repo, &ids)?;
        let registration = inputs
            .repository
            .selected
            .is_none()
            .then(|| crate::repository::location::prepare_registration(store, &repo, None, false))
            .transpose()?;
        let mut parent = None;
        let mut additions = Vec::new();
        for (prefix, part, authored_id) in authored {
            let retained = authored_id
                .as_ref()
                .and_then(|id| {
                    inputs
                        .waves
                        .iter()
                        .chain(additions.iter())
                        .find(|wave| &wave.id == id)
                })
                .cloned();
            if skip_retired
                && retained
                    .as_ref()
                    .is_some_and(|wave| wave.retired_at.is_some())
            {
                return Ok(None);
            }
            let live = retained.as_ref().filter(|wave| wave.retired_at.is_none());
            let existing = inputs.waves.iter().chain(additions.iter()).find(|wave| {
                wave.repo == repo
                    && wave.name == part
                    && wave.parent == parent
                    && wave.retired_at.is_none()
            });
            let wave = if let Some(live) = live {
                // An older checkout must not undo a move or retirement in saved Work.
                live.id.clone()
            } else if let Some(existing) = existing {
                if retained.is_none() && authored_id.as_ref().is_some_and(|id| id != &existing.id) {
                    return Err(invalid(format!(
                        "Wave {prefix} is already registered with another id"
                    )));
                }
                existing.id.clone()
            } else {
                let id = if retained.is_some() {
                    WaveId::new()
                } else {
                    authored_id.clone().unwrap_or_default()
                };
                additions.push(WaveBootstrapRow {
                    id: id.clone(),
                    repo: repo.clone(),
                    name: part,
                    parent: parent.clone(),
                    retired_at: None,
                });
                id
            };
            let directory = checkout.join(".lf/wave").join(&prefix);
            seed_wave_files(&directory, &wave, &prefix)?;
            let current = crate::work::wave::config::try_read_wave_config(checkout, &prefix)
                .map_err(invalid)?
                .and_then(|config| config.id);
            if current.is_none()
                || (retained
                    .as_ref()
                    .is_some_and(|row| row.retired_at.is_some())
                    && current == authored_id)
            {
                crate::work::wave::config::update_wave_config(checkout, &prefix, |values| {
                    let latest = values
                        .get("id")
                        .cloned()
                        .map(serde_yaml_ng::from_value::<WaveId>)
                        .transpose()
                        .map_err(|error| error.to_string())?;
                    if latest == current {
                        values.insert("id".into(), wave.to_string().into());
                    }
                    Ok(())
                })
                .map_err(invalid)?;
            }
            if crate::work::wave::config::try_read_wave_config(checkout, &prefix)
                .map_err(invalid)?
                .and_then(|config| config.id)
                .as_ref()
                != Some(&wave)
            {
                continue 'acquire;
            }
            parent = Some(wave);
        }
        if store.commit_wave_bootstrap(
            &repo,
            &ids,
            &inputs,
            registration.as_ref().map(|r| (&r.inputs, &r.id)),
            &additions,
        )? {
            return Ok(parent);
        }
    }
}

/// Register every authored Wave directory under `<checkout>/.lf/wave/`, matching
/// by `config.yaml` id first. A directory whose id names a removed Wave is left
/// alone with its subtree, so a clone or a checkout behind the planning store
/// never brings a removed Wave back. A directory that cannot be read or
/// registered is logged and skipped with its subtree; the rest still register.
pub fn register_authored_waves(store: &SqliteStore, repo: &str) -> StoreResult<()> {
    let checkout = Path::new(repo);
    let root = checkout.join(".lf/wave");
    let mut pending = child_directories(&root, "");
    while let Some(slug) = pending.pop() {
        let directory = root.join(&slug);
        if directory.join("GOAL.md").is_file() || directory.join("config.yaml").is_file() {
            match ensure_wave_inner(store, repo, &slug, true) {
                Ok(Some(_)) => {}
                Ok(None) => {
                    tracing::info!(
                        slug,
                        "skipping authored Wave directory whose Wave was removed"
                    );
                    continue;
                }
                Err(error) => {
                    tracing::warn!(slug, %error, "authored Wave directory was not registered");
                    continue;
                }
            }
        }
        pending.extend(child_directories(&root, &slug));
    }
    Ok(())
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

    use crate::work::wave::{Wave, WaveLocator};
    use loopflow_model::id::WaveId;

    #[test]
    fn wave_files_are_created_without_overwriting_authored_content() {
        let repo = loopflow_test_support::TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let directory = repo.path().join(".lf/wave/tools/parser");
        std::fs::create_dir_all(&directory).unwrap();
        let id = WaveId::new();
        let goal = format!("---\nid: {id}\n---\nKeep tokens λ.\n");
        std::fs::write(directory.join("GOAL.md"), &goal).unwrap();
        assert_eq!(
            super::ensure_wave(&store, repo.path().to_str().unwrap(), "tools/parser").unwrap(),
            id
        );
        let parent = crate::work::wave::config::read_wave_config(repo.path(), "tools").unwrap();
        assert!(parent.id.is_some());
        assert!(repo.path().join(".lf/wave/tools/GOAL.md").is_file());
        assert!(!directory.join("config.yaml").exists());
        assert!(directory.join("MEMORY.md").is_file());
        std::fs::write(directory.join("MEMORY.md"), "Edited directly.\n").unwrap();
        assert_eq!(
            super::ensure_wave(&store, repo.path().to_str().unwrap(), "tools/parser").unwrap(),
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
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let root = repo.path().to_str().unwrap();
        let renamed = crate::work::wave::bootstrap::ensure_wave(&store, root, "product").unwrap();
        let removed = crate::work::wave::bootstrap::ensure_wave(&store, root, "archive").unwrap();
        // The store renames `product` and retires `archive`; this checkout still
        // has both old directories.
        let saved = store.get_wave(&renamed).unwrap().unwrap();
        store
            .relocate_waves(&[loopflow_storage::WaveLocatorUpdate {
                wave_id: renamed.clone(),
                expected_repo: saved.repo().into(),
                expected_slug: "product".into(),
                target: loopflow_storage::work::wave::WaveLocator::new(
                    loopflow_storage::address::CanonicalRepo::from_canonical_path(
                        std::path::PathBuf::from(saved.repo()),
                    ),
                    "prod",
                )
                .unwrap(),
            }])
            .unwrap();
        store
            .retire_waves(std::slice::from_ref(&removed), "removed")
            .unwrap();

        assert_eq!(
            super::ensure_wave(&store, root, "product").unwrap(),
            renamed
        );
        assert_eq!(store.get_wave(&renamed).unwrap().unwrap().name(), "prod");
        let fresh = crate::work::wave::bootstrap::ensure_wave(&store, root, "archive").unwrap();
        assert_ne!(fresh, removed, "a removed identity is never re-created");
        assert!(store.get_wave(&removed).unwrap().unwrap().is_retired());
    }

    #[test]
    fn registering_a_clones_directories_lists_them_and_skips_a_removed_wave() {
        let repo = loopflow_test_support::TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let root = repo.path().to_str().unwrap();
        // The clone carries a removed Wave's directory and two nobody registered.
        let removed = crate::work::wave::bootstrap::ensure_wave(&store, root, "archive").unwrap();
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

        crate::work::wave::bootstrap::register_authored_waves(&store, root).unwrap();
        let canonical = crate::repository::discover(repo.path())
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
        crate::work::wave::bootstrap::register_authored_waves(&store, root).unwrap();
        assert_eq!(
            store.list_waves(Some(&canonical)).unwrap().len(),
            3,
            "registering again changes nothing"
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
        let canonical = crate::repository::discover(repo.path()).unwrap();
        let id = crate::work::wave::bootstrap::ensure_wave(
            &store.sqlite,
            &canonical.to_string(),
            "tools/parser",
        )
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
    #[test]
    fn file_failure_registers_neither_repository_nor_part_of_the_hierarchy() {
        let repo = loopflow_test_support::TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let blocked = repo.path().join(".lf/wave/tools/parser/MEMORY.md");
        std::fs::create_dir_all(&blocked).unwrap();
        assert!(super::ensure_wave(&store, repo.path().to_str().unwrap(), "tools/parser").is_err());
        let canonical = crate::repository::discover(repo.path())
            .unwrap()
            .to_string();
        assert!(store.repository_id(&canonical).unwrap().is_none());
        assert!(store.list_waves(Some(&canonical)).unwrap().is_empty());
        let parent = crate::work::wave::config::read_wave_config(repo.path(), "tools")
            .unwrap()
            .id
            .unwrap();
        std::fs::remove_dir(blocked).unwrap();
        let child =
            super::ensure_wave(&store, repo.path().to_str().unwrap(), "tools/parser").unwrap();
        assert_eq!(
            store.get_wave(&child).unwrap().unwrap().parent_wave_id(),
            Some(&parent)
        );
    }

    #[test]
    fn sql_rollback_retries_with_the_prepared_ids_and_later_authored_bytes() {
        let repo = loopflow_test_support::TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TEMP TRIGGER fail_child BEFORE INSERT ON waves WHEN NEW.name='parser'
             BEGIN SELECT RAISE(ABORT,'child registration failed'); END;",
            )
            .unwrap();
        assert!(super::ensure_wave(&store, repo.path().to_str().unwrap(), "tools/parser").is_err());
        let canonical = crate::repository::discover(repo.path())
            .unwrap()
            .to_string();
        assert!(store.repository_id(&canonical).unwrap().is_none());
        assert!(store.list_waves(Some(&canonical)).unwrap().is_empty());
        let parent = crate::work::wave::config::read_wave_config(repo.path(), "tools")
            .unwrap()
            .id
            .unwrap();
        let child = crate::work::wave::config::read_wave_config(repo.path(), "tools/parser")
            .unwrap()
            .id
            .unwrap();
        let goal = repo.path().join(".lf/wave/tools/parser/GOAL.md");
        std::fs::write(&goal, "Later authored λ.\n").unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_child")
            .unwrap();
        assert_eq!(
            super::ensure_wave(&store, repo.path().to_str().unwrap(), "tools/parser").unwrap(),
            child
        );
        assert_eq!(
            store.get_wave(&child).unwrap().unwrap().parent_wave_id(),
            Some(&parent)
        );
        assert_eq!(
            std::fs::read_to_string(goal).unwrap(),
            "Later authored λ.\n"
        );
    }

    #[test]
    fn prepared_hierarchy_cannot_undo_a_rename_retirement_parent_or_repository_change() {
        for change in [
            "name='renamed'",
            "retired_at=9",
            "parent_wave_id=?2",
            "repo='/moved'",
        ] {
            let repo = loopflow_test_support::TestRepo::new();
            let home = tempfile::tempdir().unwrap();
            let store =
                crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
            let path = repo.path().to_str().unwrap();
            let parent = super::ensure_wave(&store, path, "parent").unwrap();
            let id = super::ensure_wave(&store, path, "tools").unwrap();
            let canonical = crate::repository::discover(repo.path())
                .unwrap()
                .to_string();
            let ids = vec![id.clone()];
            let inputs = store.wave_bootstrap_inputs(&canonical, &ids).unwrap();
            {
                let conn = store.conn.lock().unwrap();
                let sql = format!("UPDATE waves SET {change} WHERE id=?1");
                if change.contains("?2") {
                    conn.execute(&sql, rusqlite::params![id, parent]).unwrap();
                } else {
                    conn.execute(&sql, [&id]).unwrap();
                }
            }
            let addition = loopflow_storage::sqlite::wave_definitions::WaveBootstrapRow {
                id: WaveId::new(),
                repo: canonical.clone(),
                name: "parser".into(),
                parent: Some(id),
                retired_at: None,
            };
            assert!(
                !store
                    .commit_wave_bootstrap(
                        &canonical,
                        &ids,
                        &inputs,
                        None,
                        std::slice::from_ref(&addition)
                    )
                    .unwrap(),
                "{change}"
            );
            assert!(store.get_wave(&addition.id).unwrap().is_none());
        }
    }

    #[test]
    fn linked_checkout_bootstrap_authors_there_and_saves_the_main_address() {
        let repo = loopflow_test_support::TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let checkout = home.path().join("linked");
        let output = std::process::Command::new("git")
            .current_dir(repo.path())
            .args(["worktree", "add", "-b", "linked"])
            .arg(&checkout)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let id = super::ensure_wave(&store, checkout.to_str().unwrap(), "tools/parser").unwrap();
        assert!(checkout.join(".lf/wave/tools/parser/GOAL.md").is_file());
        assert!(!repo.path().join(".lf/wave/tools").exists());
        assert_eq!(
            store.get_wave(&id).unwrap().unwrap().repo(),
            crate::repository::discover(repo.path())
                .unwrap()
                .to_string()
        );
    }

    #[test]
    fn discovery_skips_unreadable_and_retired_subtrees_and_continues() {
        let repo = loopflow_test_support::TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let path = repo.path().to_str().unwrap();
        let removed = super::ensure_wave(&store, path, "removed").unwrap();
        store.retire_waves(&[removed], "removed").unwrap();
        for prefix in ["broken", "broken/child", "removed/child", "good"] {
            let directory = repo.path().join(".lf/wave").join(prefix);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("GOAL.md"), "## Objective\n\nKeep.\n").unwrap();
        }
        std::fs::write(repo.path().join(".lf/wave/broken/config.yaml"), "id: [\n").unwrap();
        super::register_authored_waves(&store, path).unwrap();
        let canonical = crate::repository::discover(repo.path())
            .unwrap()
            .to_string();
        let live = store.list_waves(Some(&canonical)).unwrap();
        assert_eq!(
            live.iter().map(|wave| wave.slug()).collect::<Vec<_>>(),
            ["good"]
        );
        assert!(!repo
            .path()
            .join(".lf/wave/removed/child/config.yaml")
            .exists());
    }

    #[test]
    fn authored_ids_match_earlier_prepared_prefixes_before_sql_insertion() {
        let repo = loopflow_test_support::TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let id = WaveId::new();
        for prefix in ["tools", "tools/parser"] {
            let directory = repo.path().join(".lf/wave").join(prefix);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("config.yaml"), format!("id: {id}\n")).unwrap();
        }
        assert_eq!(
            super::ensure_wave(&store, repo.path().to_str().unwrap(), "tools/parser").unwrap(),
            id
        );
        let saved = store.get_wave(&id).unwrap().unwrap();
        assert_eq!(saved.name(), "tools");
        assert!(saved.parent_wave_id().is_none());
    }
}
