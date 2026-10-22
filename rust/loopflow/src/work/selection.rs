//! Built-in selections are passive projections of the Work reader's pinned snapshot.
//! Membership follows Started and terminal facts, never process liveness or pane focus.
use std::collections::BTreeSet;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::store::sqlite::SqliteStore;

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkWorkspaceKind {
    Repository,
    Wave,
    Task,
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct WorkWorkspaceKey {
    pub repository: String,
    pub kind: WorkWorkspaceKind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewMember {
    pub work: WorkWorkspaceKey,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewPlan {
    pub owner: WorkWorkspaceKey,
    pub members: Vec<ViewMember>,
}

/// Repository and Wave defaults share the same membership rule. All reads must
/// run inside the Work reader's snapshot; failures never become empty membership.
pub(crate) fn default_plans(store: &SqliteStore, repo: &str) -> Result<Vec<ViewPlan>> {
    let repository = store
        .repository_id(repo)?
        .context("Repository identity is unavailable")?;
    let key = |kind, id| WorkWorkspaceKey {
        repository: repository.to_string(),
        kind,
        id,
    };
    let mut waves = store.list_waves(Some(repo))?;
    waves.sort_by(|a, b| (a.name(), a.id().as_str()).cmp(&(b.name(), b.id().as_str())));
    let mut tasks = store.list_tasks(None)?;
    tasks.retain(|task| task.repository_id == repository);
    tasks.sort_by(|a, b| (a.rank, a.id.as_str()).cmp(&(b.rank, b.id.as_str())));
    let mut members = Vec::new();
    for task in tasks {
        if !store.task_owned(&task.id)? || store.task_state(&task.id)?.is_terminal() {
            continue;
        }
        members.push((
            task.wave_id,
            ViewMember {
                work: key(WorkWorkspaceKind::Task, task.id.to_string()),
                title: task.title,
            },
        ));
    }
    // Keep authored Task order; Session activity is read separately.
    members.extend(waves.iter().map(|wave| {
        (
            wave.parent_wave_id().cloned(),
            ViewMember {
                work: key(WorkWorkspaceKind::Wave, wave.id().to_string()),
                title: wave.name().into(),
            },
        )
    }));
    let mut plans = vec![ViewPlan {
        owner: key(WorkWorkspaceKind::Repository, repository.to_string()),
        members: members.iter().map(|(_, member)| member.clone()).collect(),
    }];
    for wave in &waves {
        // Fixed-point closure tolerates unordered parents and cannot loop on cycles.
        let mut descendants = BTreeSet::from([wave.id().as_str()]);
        loop {
            let before = descendants.len();
            for child in &waves {
                if child
                    .parent_wave_id()
                    .is_some_and(|parent| descendants.contains(parent.as_str()))
                {
                    descendants.insert(child.id().as_str());
                }
            }
            if before == descendants.len() {
                break;
            }
        }
        plans.push(ViewPlan {
            owner: key(WorkWorkspaceKind::Wave, wave.id().to_string()),
            members: members
                .iter()
                .filter(|(parent, member)| {
                    member.work.id != wave.id().as_str()
                        && parent
                            .as_ref()
                            .is_some_and(|parent| descendants.contains(parent.as_str()))
                })
                .map(|(_, member)| member.clone())
                .collect(),
        });
    }
    Ok(plans)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{default_plans, WorkWorkspaceKind};
    use crate::durable::TaskId;
    use crate::store::sqlite::SqliteStore;
    use crate::work::task::NewTask;

    #[test]
    fn defaults_keep_started_idle_and_paused_work_without_executing() {
        let _ledger = crate::journal::TestLedgerGuard::new();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        let path = dir.path().join("db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let repo = crate::repository::CanonicalRepo::discover(dir.path())
            .unwrap()
            .to_string();
        let repo = repo.as_str();
        let project = store.ensure_wave_project(repo, "parent").unwrap();
        let child = store.ensure_wave_project(repo, "child").unwrap();
        let mut ids = Vec::new();
        for (title, project_id) in [
            ("Idle", Some(&project.id)),
            ("Paused", Some(&child.id)),
            ("Backlog", Some(&project.id)),
            ("Done", Some(&project.id)),
            ("Standalone", None),
        ] {
            let task = store
                .create_task(&NewTask {
                    id: TaskId::new(),
                    repository_id: store.repository_id(repo).unwrap().unwrap(),
                    project_id: project_id.cloned(),
                    title: title.into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap();
            ids.push(task.id);
        }
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute(
            "UPDATE waves SET parent_wave_id=?1 WHERE id=?2",
            rusqlite::params![project.wave_id.as_str(), child.wave_id.as_str()],
        )
        .unwrap();
        for id in [&ids[0], &ids[1], &ids[3], &ids[4]] {
            conn.execute("INSERT INTO task_events(task_id,kind_json,created_at) VALUES(?1,'{\"kind\":\"started\"}',1)", [id.as_str()]).unwrap();
            conn.execute("INSERT INTO task_ownership(id,task_id,kind,at,written_here) VALUES('own_'||lower(hex(randomblob(16))),?1,'recorded',1,1)", [id.as_str()])
                .unwrap();
        }
        conn.execute("UPDATE tasks SET paused=1 WHERE id=?1", [ids[1].as_str()])
            .unwrap();
        conn.execute(
            "UPDATE tasks SET planning_completed=1 WHERE id=?1",
            [ids[3].as_str()],
        )
        .unwrap();
        let revisions = store.revisions().unwrap();
        {
            let _snapshot = store.read_snapshot().unwrap();
            let plans = default_plans(&store, repo).unwrap();
            let root = &plans[0];
            assert_eq!(root.members.len(), 5);
            assert_eq!(
                root.members
                    .iter()
                    .filter(|member| member.work.kind == WorkWorkspaceKind::Task)
                    .map(|member| member.title.as_str())
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["Idle", "Paused", "Standalone"])
            );
            let parent = plans
                .iter()
                .find(|plan| plan.owner.id == project.wave_id.as_str())
                .unwrap();
            assert_eq!(parent.members.len(), 3);
            let child = plans
                .iter()
                .find(|plan| plan.owner.id == child.wave_id.as_str())
                .unwrap();
            assert_eq!(child.members.len(), 1);
            assert_eq!(child.members[0].title, "Paused");
            assert!(default_plans(&store, "/unknown-repository").is_err());
        }
        assert_eq!(store.revisions().unwrap(), revisions);
        for table in ["agent_sessions", "processes", "graph_processes"] {
            assert_eq!(
                conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        conn.execute(
            "UPDATE tasks SET planning_completed=1 WHERE id=?1",
            [ids[0].as_str()],
        )
        .unwrap();
        let _snapshot = store.read_snapshot().unwrap();
        let current = default_plans(&store, repo).unwrap();
        assert!(!current[0]
            .members
            .iter()
            .any(|member| member.work.id == ids[0].as_str()));
    }
}
