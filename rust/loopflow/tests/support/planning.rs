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
                task.linear_id.as_ref().map(|id| id.as_str()),
                task.identifier,
                task.title,
                task.description,
                task.pm_snapshot_synced_at,
                task.created_at.unix_timestamp(),
                task.updated_at.unix_timestamp(),
                task.agent
            ],
        )
        .unwrap();
}

/// Fixed acquisition evidence for execution fixtures; ingestion itself is tested privately.
#[allow(dead_code)]
pub fn seed_linear_admission(
    database: &Path,
    wave: &loopflow::work::wave::Wave,
    project: &loopflow::work::project::Project,
    task: &Task,
) {
    let conn = Connection::open(database).unwrap();
    let external_project = project.linear_id.as_ref().unwrap().as_str();
    let external_issue = task.linear_id.as_ref().unwrap().as_str();
    conn.execute(
        "UPDATE projects SET planning_teams='[\"team-task-pr-tests\"]',planning_initiatives='[\"initiative-task-pr-tests\"]' WHERE id=?1",
        [project.id.as_str()],
    ).unwrap();
    conn.execute(
        "INSERT INTO pm_projects(repo,provider,id,observed_at,slug,name,summary,workflow,status,krs,metric_targets,initiative_ids,team_ids)
         VALUES(?1,'linear',?2,?3,?4,?5,?6,?7,'started','[]','[]','[\"initiative-task-pr-tests\"]','[\"team-task-pr-tests\"]')",
        params![wave.repo(),external_project,project.pm_snapshot_synced_at,project.slug,project.name,project.summary,project.workflow],
    ).unwrap();
    conn.execute(
        "INSERT INTO pm_wave_sync(wave_id,provider,initiative,synced_at) VALUES(?1,'linear','initiative-task-pr-tests',?2)",
        params![wave.id(),project.pm_snapshot_synced_at],
    ).unwrap();
    conn.execute(
        "INSERT INTO pm_wave_projects(wave_id,project_id,position) VALUES(?1,?2,0)",
        params![wave.id(), external_project],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO pm_items(repo,provider,id,observed_at,identifier,project_id,claimed_project_id,name,description,rank,completed,team_id,project_id_observed,assignee_observed,completed_at_observed,due_date_observed)
         VALUES(?1,'linear',?2,?3,?4,?5,?5,?6,?7,1,0,'team-task-pr-tests',1,1,1,1)",
        params![wave.repo(),external_issue,task.pm_snapshot_synced_at,task.identifier,external_project,task.title,task.description],
    ).unwrap();
    let url = "https://linear.app/loopflow/issue/INF-123/prove-task-pr-transitions";
    conn.execute(
        "UPDATE pm_items SET url=?2 WHERE id=?1",
        params![external_issue, url],
    )
    .unwrap();
    conn.execute(
        "UPDATE tasks SET planning_url=?2,planning_team_id='team-task-pr-tests',planning_rank=1 WHERE id=?1",
        params![task.id.as_str(),url],
    )
    .unwrap();
}
