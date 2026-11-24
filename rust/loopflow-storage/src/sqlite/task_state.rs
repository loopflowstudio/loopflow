//! A local Task decision reaches its tracker as a `state` action, which the
//! planning writer queues with the Disposition.

use super::planning_write;
use crate::work::edit::WorkValue as Edit;
use rusqlite::Connection;

use crate::{StoreError, StoreResult};
use loopflow_model::durable::TaskId;

pub(super) fn decide_in(conn: &Connection, task: &TaskId, target: &str) -> StoreResult<()> {
    let completed_at = (target == "completed")
        .then(|| {
            time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)
        })
        .transpose()
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    planning_write::local(
        conn,
        &crate::work::edit::WorkEdit::for_target(
            task.as_str(),
            &[Edit::Disposition(crate::work::edit::Disposition {
                planning_state: Some(target.into()),
                planning_completed: i64::from(target == "completed"),
                planning_completed_at: completed_at,
            })],
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::work::edit::{Disposition, WorkEdit, WorkValue};
    use loopflow_model::durable::TaskId;
    use rusqlite::params;

    // Released lf reads whether a Task is done from the old boolean; the guard
    // keeps this build off it, so a second connection reads what released lf
    // would.
    #[test]
    fn released_lf_sees_the_completion_a_state_decides() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("loopflow.db");
        let store = crate::test_fixture::open(&path).unwrap();
        let repo = crate::test_fixture::repository(&store, "/repo");
        let task = TaskId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO tasks(id,repository_id,issue_identifier,issue_title,issue_description,workspace_slug,created_at,updated_at) VALUES(?1,?2,'DONE-1','Done','','done',1,1)",
                params![task.as_str(), repo.as_str()],
            )
            .unwrap();
        let released = || {
            rusqlite::Connection::open(&path)
                .unwrap()
                .query_row(
                    "SELECT planning_completed FROM tasks WHERE id=?1",
                    [task.as_str()],
                    |row| row.get::<_, bool>(0),
                )
                .unwrap()
        };
        let decide = |disposition: Disposition| {
            let conn = store.conn.lock().unwrap();
            super::planning_write::local(
                &conn,
                &WorkEdit::for_target(task.as_str(), &[WorkValue::Disposition(disposition)]),
            )
            .unwrap();
        };
        for (state, done) in [("completed", true), ("started", false)] {
            let conn = store.conn.lock().unwrap();
            super::decide_in(&conn, &task, state).unwrap();
            drop(conn);
            assert_eq!(released(), done, "{state}");
        }
        // A peer's completion that names no state is a completed Task.
        decide(Disposition {
            planning_state: None,
            planning_completed: 1,
            planning_completed_at: None,
        });
        assert!(released());
        assert_eq!(
            store.task(&task).unwrap().unwrap().state.as_deref(),
            Some("completed")
        );
    }
}
