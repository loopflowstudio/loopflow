//! Wave definitions have one stored owner. Repository files are explicit import sources.

use super::planning_write::{self, PlanningEdit as Edit};
use crate::engine::planning_exchange::PlanningKind;
use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::id::WaveId;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::project::Project;

use super::{durable, SqliteStore};

impl SqliteStore {
    pub(crate) fn wave_document(&self, wave: &WaveId, name: &str) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT content FROM wave_documents WHERE wave_id=?1 AND name=?2",
            params![wave, name],
            |row| row.get(0),
        )
        .optional()
        .map_err(StoreError::from)
    }

    pub fn wave_documents(&self, wave: &WaveId) -> StoreResult<BTreeMap<String, String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query =
            conn.prepare("SELECT name,content FROM wave_documents WHERE wave_id=?1 ORDER BY name")?;
        let rows = query.query_map([wave], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<_, _>>().map_err(StoreError::from)
    }

    pub fn update_wave_document(
        &self,
        wave: &WaveId,
        name: &str,
        content: &str,
    ) -> StoreResult<()> {
        if !matches!(name, "GOAL.md" | "MEMORY.md") {
            return Err(StoreError::InvalidData(
                "expected GOAL.md or MEMORY.md".into(),
            ));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO wave_documents(wave_id,name,content) VALUES(?1,?2,?3)
            ON CONFLICT(wave_id,name) DO UPDATE SET content=excluded.content
            WHERE content!=excluded.content",
            params![wave, name, content],
        )?;
        Ok(())
    }

    /// Provision a Wave hierarchy without a provider or tracked-file write.
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
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let workflows = read_workflows(Path::new(repo))?;
        let mut parent: Option<WaveId> = None;
        let mut prefix = String::new();
        for part in name.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let documents = read_files(&Path::new(repo).join("wave").join(&prefix), &["md"])?;
            let authored_id = documents
                .get("GOAL.md")
                .map(|content| crate::work::wave::config::parse_wave_config(content))
                .transpose()
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
                    planning_write::create(
                        &tx,
                        repo,
                        PlanningKind::Wave,
                        wave.as_str(),
                        &[
                            Edit::WaveName(part.into()),
                            Edit::WaveParent(parent.as_ref().map(ToString::to_string)),
                        ],
                    )?;
                    durable::create_wave_work(&tx, &wave, now)?;
                    wave
                }
            };
            save_imported_documents(&tx, wave.as_str(), documents, &workflows)?;
            parent = Some(wave);
        }
        tx.commit()?;
        Ok(parent.expect("validated Wave has at least one component"))
    }
}

pub(crate) fn import_registered_documents(conn: &rusqlite::Connection) -> StoreResult<()> {
    let mut query = conn.prepare("SELECT id,repo,slug FROM wave_addresses")?;
    let waves = query
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (wave, repo, name) in waves {
        import_documents_on(conn, &wave, Path::new(&repo), &name)?;
    }
    Ok(())
}

pub(super) fn import_documents_on(
    conn: &rusqlite::Connection,
    wave: &str,
    repo: &Path,
    name: &str,
) -> StoreResult<()> {
    save_imported_documents(
        conn,
        wave,
        read_files(&repo.join("wave").join(name), &["md"])?,
        &read_workflows(repo)?,
    )
}

fn save_imported_documents(
    conn: &rusqlite::Connection,
    wave: &str,
    documents: BTreeMap<String, String>,
    workflows: &BTreeMap<String, String>,
) -> StoreResult<()> {
    for (name, content) in documents {
        conn.execute(
            "INSERT INTO wave_documents(wave_id,name,content) VALUES(?1,?2,?3)
            ON CONFLICT(wave_id,name) DO NOTHING",
            params![wave, name, content],
        )?;
    }
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
    let mut workflows = BTreeMap::new();
    for (name, content) in read_files(&repo.join(".lf/workflows"), &["yaml", "yml"])? {
        let name = Path::new(&name)
            .file_stem()
            .expect("definition has a name")
            .to_string_lossy()
            .into_owned();
        // Match repository discovery's .yaml preference when both extensions exist.
        workflows.entry(name).or_insert(content);
    }
    Ok(workflows)
}

fn read_files(directory: &Path, extensions: &[&str]) -> StoreResult<BTreeMap<String, String>> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => {
            return Err(StoreError::InvalidData(format!(
                "{}: {error}",
                directory.display()
            )))
        }
    };
    let mut files = BTreeMap::new();
    for entry in entries {
        let entry = entry.map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extensions.contains(&extension))
        {
            files.insert(
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read_to_string(&path).map_err(|error| {
                    StoreError::InvalidData(format!("{}: {error}", path.display()))
                })?,
            );
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::SqliteStore;
    use crate::id::WaveId;
    use crate::store::migrations::{apply_before_current_draft, current_draft_sql};
    use crate::work::wave::{Wave, WaveLocator};

    #[test]
    fn released_wave_documents_import_without_rewriting_source_or_identity() {
        let repo = loopflow_test_support::TestRepo::new();
        let directory = repo.path().join("wave/tools");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::create_dir_all(repo.path().join(".lf/workflows")).unwrap();
        let id = WaveId::new();
        let goal = format!("---\nid: {id}\nagent: codex\n---\nKeep the original goal.\n");
        std::fs::write(directory.join("GOAL.md"), &goal).unwrap();
        std::fs::write(directory.join("MEMORY.md"), "Retain decisions λ.\n").unwrap();
        std::fs::write(
            repo.path().join(".lf/workflows/custom.yaml"),
            "nodes: {}\nedges: [{from: start, to: end}]\n",
        )
        .unwrap();
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_current_draft(&conn, "local_planning");
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'tools',?2,123)",
            rusqlite::params![id, repo.path().to_string_lossy()],
        )
        .unwrap();
        let tx = conn.transaction().unwrap();
        tx.execute_batch(&current_draft_sql("local_planning"))
            .unwrap();
        super::import_registered_documents(&tx).unwrap();
        tx.commit().unwrap();
        let docs = || {
            conn.prepare("SELECT name,content FROM wave_documents WHERE wave_id=?1 ORDER BY name")
                .unwrap()
                .query_map([&id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(
            docs(),
            vec![
                ("GOAL.md".into(), goal.clone()),
                ("MEMORY.md".into(), "Retain decisions λ.\n".into())
            ]
        );
        conn.execute(
            "UPDATE wave_documents SET content='Saved edit' WHERE wave_id=?1 AND name='MEMORY.md'",
            [&id],
        )
        .unwrap();
        super::import_registered_documents(&conn).unwrap();
        assert_eq!(docs()[1].1, "Saved edit");
        assert_eq!(
            conn.query_row("SELECT id,created_at FROM waves", [], |row| Ok((
                row.get::<_, WaveId>(0)?,
                row.get::<_, i64>(1)?
            )))
            .unwrap(),
            (id, 123)
        );
        assert_eq!(
            std::fs::read_to_string(directory.join("GOAL.md")).unwrap(),
            goal
        );
        assert_eq!(
            conn.query_row("SELECT name FROM wave_workflows", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            "custom"
        );
        for retired in ["personal_plans", "personal_wave_definitions"] {
            assert_eq!(
                conn.query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name=?1",
                    [retired],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
                0
            );
        }
    }

    #[test]
    fn wave_creation_rolls_back_definitions_and_ancestors_together() {
        let repo = loopflow_test_support::TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        std::fs::create_dir_all(repo.path().join("wave/tools/parser")).unwrap();
        std::fs::write(
            repo.path().join("wave/tools/parser/GOAL.md"),
            "Retain tokens.",
        )
        .unwrap();
        std::fs::create_dir_all(repo.path().join(".lf/workflows")).unwrap();
        let definition = "nodes: {}\nedges: [{from: start, to: end}]\n";
        std::fs::write(repo.path().join(".lf/workflows/review.yaml"), definition).unwrap();
        std::fs::write(repo.path().join(".lf/workflows/review.yml"), "not selected").unwrap();
        store.conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_import BEFORE INSERT ON wave_documents BEGIN SELECT RAISE(ABORT,'disk failure'); END;").unwrap();
        assert!(store
            .ensure_wave(repo.path().to_str().unwrap(), "tools/parser")
            .is_err());
        assert!(store.list_waves(None).unwrap().is_empty());
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_import;")
            .unwrap();
        let first = store
            .ensure_wave(repo.path().to_str().unwrap(), "tools/parser")
            .unwrap();
        assert_eq!(
            store
                .ensure_wave(repo.path().to_str().unwrap(), "tools/parser")
                .unwrap(),
            first
        );
        assert_eq!(store.list_waves(None).unwrap().len(), 2);
        std::fs::remove_dir_all(repo.path().join(".lf/workflows")).unwrap();
        for wave in store.list_waves(None).unwrap() {
            assert_eq!(
                store.wave_workflow(wave.id(), "review").unwrap().as_deref(),
                Some(definition)
            );
        }
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
        store
            .sqlite
            .update_wave_document(&id, "MEMORY.md", "Retained")
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
            store.sqlite.wave_documents(&id).unwrap()["MEMORY.md"],
            "Retained"
        );
    }
}
