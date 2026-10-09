use std::path::Path;

use loopflow::work::task::Task;
use rusqlite::{params, Connection};

pub fn seed_unplaced_task(database: &Path, task: &Task) {
    Connection::open(database)
        .unwrap()
        .execute(
            "INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,issue_title,
            issue_description,pm_snapshot_synced_at,created_at,updated_at,agent,workspace_slug)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'')",
            params![
                task.id.as_str(),
                task.project_id.as_str(),
                task.plan.linear_id.as_ref().map(|id| id.as_str()),
                task.plan.identifier,
                task.plan.title,
                task.plan.description,
                task.plan.pm_snapshot_synced_at,
                task.created_at.unix_timestamp(),
                task.updated_at.unix_timestamp(),
                task.agent
            ],
        )
        .unwrap();
}
