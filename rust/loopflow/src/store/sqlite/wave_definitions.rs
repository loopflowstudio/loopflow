//! Wave registration.

use crate::work::edit::WorkValue as Edit;
use std::path::Path;

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::id::WaveId;
use crate::store::{StoreError, StoreResult};
use crate::work::project::Project;

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
            let authored_id = crate::work::wave::config::try_read_wave_config(checkout, &prefix)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?
                .and_then(|config| config.id);
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
