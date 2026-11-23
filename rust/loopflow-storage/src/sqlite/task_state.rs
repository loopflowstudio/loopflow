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
