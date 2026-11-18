//! Test stores refuse SQL that reads or writes a member marked leaving.
//!
//! A migration marks what the product stopped using with
//! `-- leaving: <table>[.<column>] <Task ID>`. Once a member is marked, nothing
//! but its drop may name it, so every store test fails with the member's name
//! when production SQL touches one. Triggers still name leaving columns until
//! the contract drops them, so access from inside a trigger is allowed. So are
//! reads of foreign-key columns and deletes from tables a parent cascades into:
//! SQLite's own key checks and cascades report those when a parent row changes,
//! with nothing to tell them apart from a query.

use std::collections::HashSet;
use std::sync::OnceLock;

use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::Connection;

/// Marked leaving while production still reads them, each until its Task
/// removes the last read.
const STILL_READ: &[(&str, &str)] = &[
    ("agent_sessions.work_source", "LOO-836"),
    // Fixtures write the rows a released lf does; the bridge triggers read them.
    ("processes", "LOO-965"),
    ("tasks.regressed_in", "LOO-730"),
];

/// Leaving tables, and leaving columns as `table.column`.
fn leaving() -> &'static HashSet<String> {
    static LEAVING: OnceLock<HashSet<String>> = OnceLock::new();
    LEAVING.get_or_init(|| {
        crate::migration_runner::migration_sqls()
            .flat_map(str::lines)
            .filter_map(|line| line.strip_prefix("-- leaving:"))
            .filter_map(|mark| mark.split_whitespace().next())
            .filter(|member| !STILL_READ.iter().any(|(read, _)| read == member))
            .map(str::to_owned)
            .collect()
    })
}

/// What SQLite's foreign-key machinery touches on its own.
#[derive(Debug)]
struct Keys {
    /// Both ends of every foreign key, as `table.column`.
    columns: HashSet<String>,
    /// Tables a parent's delete cascades into.
    cascading: HashSet<String>,
}

fn keys(conn: &Connection) -> Keys {
    let mut statement = conn
        .prepare(
            "SELECT m.name, f.\"from\", f.\"table\", f.\"to\", f.on_delete
             FROM sqlite_master m, pragma_foreign_key_list(m.name) f WHERE m.type='table'",
        )
        .expect("list foreign keys");
    let mut keys = Keys {
        columns: HashSet::new(),
        cascading: HashSet::new(),
    };
    let mut rows = statement.query([]).expect("read foreign keys");
    while let Some(row) = rows.next().expect("foreign key row") {
        let child: String = row.get(0).expect("child table");
        let parent: String = row.get(2).expect("parent table");
        keys.columns.insert(format!(
            "{child}.{}",
            row.get::<_, String>(1).expect("child column")
        ));
        if let Some(to) = row.get::<_, Option<String>>(3).expect("parent column") {
            keys.columns.insert(format!("{parent}.{to}"));
        }
        if row.get::<_, String>(4).expect("delete action") == "CASCADE" {
            keys.cascading.insert(child);
        }
    }
    keys
}

fn leaving_member(context: &AuthContext<'_>, keys: &Keys) -> Option<String> {
    if context.accessor.is_some() || context.database_name.is_some_and(|name| name != "main") {
        return None;
    }
    let (table, column) = match context.action {
        AuthAction::Read {
            table_name,
            column_name,
        }
        | AuthAction::Update {
            table_name,
            column_name,
        } => (table_name, Some(column_name)),
        AuthAction::Insert { table_name } | AuthAction::Delete { table_name } => (table_name, None),
        _ => return None,
    };
    let leaving = leaving();
    let column = column.map(|column| format!("{table}.{column}"));
    let machinery = match context.action {
        AuthAction::Read { .. } => column.as_ref().is_some_and(|c| keys.columns.contains(c)),
        AuthAction::Delete { .. } => keys.cascading.contains(table),
        _ => false,
    };
    if machinery {
        return None;
    }
    if leaving.contains(table) {
        return Some(table.to_owned());
    }
    column.filter(|member| leaving.contains(member))
}

/// Install on a migrated connection; migrations still name what they leave.
pub(crate) fn install(conn: &Connection) {
    let keys = keys(conn);
    conn.authorizer(Some(move |context: AuthContext<'_>| {
        match leaving_member(&context, &keys) {
            Some(member) => {
                eprintln!("store SQL names leaving member {member}");
                Authorization::Deny
            }
            None => Authorization::Allow,
        }
    }))
    .expect("the store owns its connection");
}

#[cfg(test)]
mod tests {
    use crate::test_fixture;

    #[test]
    fn leaving_members_are_unreadable() {
        let home = tempfile::tempdir().unwrap();
        let store = test_fixture::open(&home.path().join("store.db")).unwrap();
        let repository = test_fixture::repository(&store, "/repo");
        let conn = store.conn.lock().unwrap();
        for sql in [
            "SELECT issue_title, branch FROM tasks",
            "SELECT * FROM work_pane_layouts",
            "UPDATE tasks SET worktree=NULL",
            "INSERT INTO relocated_task_processes(task_id,process_id) VALUES('t','p')",
            "SELECT task_ids_json FROM task_orders",
            "SELECT task_order_json FROM tracker_projects",
        ] {
            let error = conn.prepare(sql).err();
            assert!(error.is_some(), "{sql} was allowed");
        }
        conn.prepare("SELECT issue_title, planning_rank FROM tasks")
            .unwrap();
        // Triggers that still compare every column keep working.
        conn.execute(
            "INSERT INTO tasks(id,repository_id,issue_identifier,created_at) VALUES('t',?1,'T-1',1)",
            [repository.as_str()],
        )
        .unwrap();
        conn.execute("UPDATE tasks SET issue_title='renamed' WHERE id='t'", [])
            .unwrap();
    }
}
