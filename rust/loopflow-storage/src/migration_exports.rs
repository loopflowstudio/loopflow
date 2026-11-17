//! What `unused_store_members` drops, written beside the store before its SQL runs.
//!
//! The file is `<lf home>/exports/unused-store-members.json`: one key per
//! dropped table (every row) and per dropped column (`id` and value for each
//! row that holds one). Empty members are left out. A retried upgrade writes
//! the same rows again, so the file always matches the store it came from.

use std::fs::File;
use std::path::Path;

use rusqlite::Connection;
use serde_json::{Map, Value};

use crate::{StoreError, StoreResult};

pub(crate) const UNUSED_STORE_MEMBERS: &str = "unused_store_members";

const TABLES: &[&str] = &[
    "tool_responses",
    "pm_project_name_cutover",
    "project_transition_items",
    "task_issue_identities",
    "task_deletions",
    "task_pr_repair_incidents",
    "project_binding_imports",
    "work_pane_layouts",
    "session_steers",
    "steer_deliveries",
    "steer_batches",
    "pools",
    "ondemands",
];

const COLUMNS: &[(&str, &str)] = &[
    ("machines", "pool"),
    ("machines", "cost_usd"),
    ("machines", "expires_at"),
    ("processes", "viewer_hold"),
    ("processes", "viewer_hold_at"),
    ("processes", "viewed"),
    ("processes", "native_input"),
    ("desktop_shells", "close_requested"),
    ("task_prs", "repair_tracking_complete"),
    ("task_ownership", "written_here"),
    ("tasks", "worktree"),
    ("tasks", "branch"),
    ("tasks", "base_commit"),
    ("tasks", "parent_pr_id"),
    ("tasks", "checkout_machine_id"),
    ("tasks", "primary_session_id"),
    ("tasks", "ship_lf_process_id"),
    ("tasks", "ship_restarts"),
    ("tasks", "ship_approved_lf_process_id"),
    ("tasks", "ship_stopped_at"),
    ("tasks", "ship_stopped_lf_process_id"),
    ("tasks", "ship_stopped_person"),
];

/// Write the export and keep it locked until the migration commits or rolls back.
pub(crate) fn export_unused_store_members(
    conn: &Connection,
    guards: &mut Vec<File>,
) -> StoreResult<()> {
    let Some(home) = conn.path().filter(|path| !path.is_empty()).map(Path::new) else {
        return Ok(());
    };
    let home = home
        .parent()
        .ok_or_else(|| StoreError::InvalidData("the store has no directory".into()))?;
    let mut export = Map::new();
    for table in TABLES {
        let columns = columns(conn, table)?;
        let rows = rows(conn, table, &columns, "1")?;
        if !rows.is_empty() {
            export.insert((*table).to_string(), Value::Array(rows));
        }
    }
    for (table, column) in COLUMNS {
        let columns = ["id".to_string(), (*column).to_string()];
        let rows = rows(conn, table, &columns, &format!("{column} IS NOT NULL"))?;
        if !rows.is_empty() {
            export.insert(format!("{table}.{column}"), Value::Array(rows));
        }
    }
    if export.is_empty() {
        return Ok(());
    }
    let directory = home.join("exports");
    std::fs::create_dir_all(&directory).map_err(io)?;
    let path = directory.join("unused-store-members.json");
    let staged = directory.join("unused-store-members.json.partial");
    std::fs::write(&staged, serde_json::to_vec_pretty(&Value::Object(export))?).map_err(io)?;
    std::fs::rename(&staged, &path).map_err(io)?;
    guards.push(File::open(&path).map_err(io)?);
    Ok(())
}

fn io(error: std::io::Error) -> StoreError {
    StoreError::InvalidData(format!("exporting unused store members: {error}"))
}

fn columns(conn: &Connection, table: &str) -> StoreResult<Vec<String>> {
    let mut statement = conn.prepare("SELECT name FROM pragma_table_xinfo(?1) ORDER BY cid")?;
    let names = statement.query_map([table], |row| row.get(0))?;
    Ok(names.collect::<Result<_, _>>()?)
}

fn rows(
    conn: &Connection,
    table: &str,
    columns: &[String],
    filter: &str,
) -> StoreResult<Vec<Value>> {
    let object = columns
        .iter()
        .map(|column| format!("'{column}',\"{column}\""))
        .collect::<Vec<_>>()
        .join(",");
    let mut statement = conn.prepare(&format!(
        "SELECT json_object({object}) FROM \"{table}\" WHERE {filter}"
    ))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
}

#[cfg(test)]
mod tests {
    use super::export_unused_store_members;

    #[test]
    fn dropped_rows_and_values_reach_the_export_and_empty_members_do_not() {
        let home = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        let mut create = super::TABLES
            .iter()
            .map(|table| format!("CREATE TABLE {table}(id TEXT);"))
            .collect::<String>();
        for table in [
            "machines",
            "processes",
            "desktop_shells",
            "task_prs",
            "task_ownership",
            "tasks",
        ] {
            let columns = super::COLUMNS
                .iter()
                .filter(|(owner, _)| *owner == table)
                .map(|(_, column)| format!(",{column} TEXT"))
                .collect::<String>();
            create.push_str(&format!("CREATE TABLE {table}(id TEXT{columns});"));
        }
        conn.execute_batch(&create).unwrap();
        conn.execute_batch(
            "INSERT INTO session_steers(id) VALUES('steer');
             INSERT INTO tasks(id,worktree) VALUES('task','/repo/task'),('bare',NULL);",
        )
        .unwrap();
        let mut guards = Vec::new();
        export_unused_store_members(&conn, &mut guards).unwrap();
        let export: serde_json::Value = serde_json::from_slice(
            &std::fs::read(home.path().join("exports/unused-store-members.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            export,
            serde_json::json!({
                "session_steers": [{"id": "steer"}],
                "tasks.worktree": [{"id": "task", "worktree": "/repo/task"}],
            })
        );
        assert_eq!(guards.len(), 1);
    }
}
