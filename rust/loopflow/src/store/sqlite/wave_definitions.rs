//! Wave registration and the separately retained Workflow import.

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::id::WaveId;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::project::Project;

use super::{durable, SqliteStore};

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
        let workflows = read_workflows(checkout)?;
        let mut parent: Option<WaveId> = None;
        let mut prefix = String::new();
        for part in name.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let directory = checkout.join("wave").join(&prefix);
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
                    let now = now_unix();
                    tx.execute("INSERT INTO waves(id,name,repo,created_at,parent_wave_id) VALUES(?1,?2,?3,?4,?5)", params![wave, part, repo, now, parent])?;
                    durable::create_wave_work(&tx, &wave, now)?;
                    wave
                }
            };
            seed_wave_files(&directory, &wave, &prefix)?;
            save_imported_workflows(&tx, wave.as_str(), &workflows)?;
            parent = Some(wave);
        }
        tx.commit()?;
        Ok(parent.expect("validated Wave has at least one component"))
    }
}

pub(crate) fn import_registered_workflows(conn: &rusqlite::Connection) -> StoreResult<()> {
    let mut query = conn.prepare("SELECT id,repo FROM wave_addresses")?;
    let waves = query
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (wave, repo) in waves {
        import_workflows_on(conn, &wave, Path::new(&repo))?;
    }
    Ok(())
}

pub(super) fn import_workflows_on(
    conn: &rusqlite::Connection,
    wave: &str,
    repo: &Path,
) -> StoreResult<()> {
    save_imported_workflows(conn, wave, &read_workflows(repo)?)
}

fn save_imported_workflows(
    conn: &rusqlite::Connection,
    wave: &str,
    workflows: &BTreeMap<String, String>,
) -> StoreResult<()> {
    for (name, content) in workflows {
        conn.execute(
            "INSERT INTO wave_workflows(wave_id,name,content) VALUES(?1,?2,?3)
            ON CONFLICT(wave_id,name) DO NOTHING",
            params![wave, name, content],
        )?;
    }
    Ok(())
}

fn read_workflows(repo: &Path) -> StoreResult<BTreeMap<String, String>> {
    let directory = repo.join(".lf/workflows");
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => {
            return Err(StoreError::InvalidData(format!(
                "{}: {error}",
                directory.display()
            )))
        }
    };
    let mut paths = entries
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    // Match repository discovery's .yaml preference when both extensions exist.
    paths.sort();
    let mut workflows = BTreeMap::new();
    for path in paths {
        if !path.is_file()
            || !matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("yaml" | "yml")
            )
        {
            continue;
        }
        let name = path
            .file_stem()
            .expect("definition has a name")
            .to_string_lossy()
            .into_owned();
        let content = std::fs::read_to_string(&path)
            .map_err(|error| StoreError::InvalidData(format!("{}: {error}", path.display())))?;
        workflows.entry(name).or_insert(content);
    }
    Ok(workflows)
}

fn seed_wave_files(directory: &Path, id: &WaveId, name: &str) -> StoreResult<()> {
    use std::io::Write;
    std::fs::create_dir_all(directory)
        .map_err(|error| StoreError::InvalidData(format!("{}: {error}", directory.display())))?;
    for (file, content) in [
        (
            "GOAL.md",
            format!("---\nid: {id}\n---\n\n## Objective\n\n{name}\n"),
        ),
        ("MEMORY.md", format!("# {name} wave memory\n")),
    ] {
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
    use crate::store::migrations::{apply_before_current_draft, current_draft_sql};
    use crate::work::wave::{Wave, WaveLocator};

    #[test]
    fn wave_files_are_created_without_overwriting_authored_content() {
        let repo = loopflow_test_support::TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let directory = repo.path().join("wave/tools/parser");
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
        assert!(repo.path().join("wave/tools/GOAL.md").is_file());
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
    fn released_frontier_retains_workflows_without_copying_documents() {
        let repo = loopflow_test_support::TestRepo::new();
        std::fs::create_dir_all(repo.path().join(".lf/workflows")).unwrap();
        std::fs::create_dir_all(repo.path().join("wave/tools")).unwrap();
        std::fs::write(
            repo.path().join("wave/tools/MEMORY.md"),
            "Only in the repo.",
        )
        .unwrap();
        let definition = "nodes: {}\nedges: [{from: start, to: end}]\n";
        let workflow_path = repo.path().join(".lf/workflows/custom.yaml");
        std::fs::write(&workflow_path, definition).unwrap();
        std::fs::write(repo.path().join(".lf/workflows/custom.yml"), "not selected").unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_current_draft(&conn, "local_planning");
        let id = WaveId::new();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'tools',?2,123)",
            rusqlite::params![id, repo.path().to_string_lossy()],
        )
        .unwrap();
        conn.execute_batch(&current_draft_sql("local_planning"))
            .unwrap();
        super::import_registered_workflows(&conn).unwrap();
        std::fs::write(&workflow_path, "later source edit").unwrap();
        super::import_registered_workflows(&conn).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT content FROM wave_workflows WHERE wave_id=?1",
                [&id],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            definition
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='wave_documents'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("wave/tools/MEMORY.md")).unwrap(),
            "Only in the repo."
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
        std::fs::write(repo.path().join("wave/tools/parser/MEMORY.md"), "Retained").unwrap();
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
            std::fs::read_to_string(repo.path().join("wave/tools/parser/MEMORY.md")).unwrap(),
            "Retained"
        );
    }
}
