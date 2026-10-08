//! Field receipts share one protocol; each object's table retains its foreign key.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::durable::{ProjectId, TaskId};
use crate::planning::PlanningChange;
use crate::store::StoreResult;

#[derive(Debug, Clone, Copy)]
pub(super) enum PlanningChanges<'a> {
    Task(&'a TaskId),
    Project(&'a ProjectId),
}

impl<'a> PlanningChanges<'a> {
    fn owner(self) -> (&'static str, &'a str) {
        match self {
            Self::Task(id) => ("task", id.as_str()),
            Self::Project(id) => ("project", id.as_str()),
        }
    }

    pub(super) fn record(
        self,
        conn: &Connection,
        field: &str,
        previous: Value,
        value: Value,
    ) -> StoreResult<bool> {
        let previous = self.normalize(conn, field, previous)?;
        let value = self.normalize(conn, field, value)?;
        if previous == value {
            return Ok(false);
        }
        let (owner, id) = self.owner();
        let observation = match self {
            Self::Task(_) => "SELECT i.body FROM tasks t JOIN projects p ON p.id=t.project_id
                JOIN waves w ON w.id=p.wave_id
                JOIN pm_items i ON i.id=t.external_issue_id AND i.repo=w.repo AND i.provider='linear'
                WHERE t.id=?1",
            Self::Project(_) => "SELECT o.body FROM projects p JOIN waves w ON w.id=p.wave_id
                JOIN pm_projects o ON o.id=p.external_project_id AND o.repo=w.repo AND o.provider='linear'
                WHERE p.id=?1",
        };
        let body: Option<String> = conn
            .query_row(observation, [id], |row| row.get(0))
            .optional()?;
        let base = body
            .map(|body| -> StoreResult<Value> {
                let body: Value = serde_json::from_str(&body)?;
                Ok(serde_json::json!({"revision": body["revision"], "value": self.normalize(conn, field, body[field].clone())?}))
            })
            .transpose()?;
        conn.execute(
            &format!(
                "INSERT INTO {owner}_changes(id,{owner}_id,field,value_json,base_json)
                VALUES(?1,?2,?3,?4,?5)"
            ),
            params![
                uuid::Uuid::new_v4().to_string(),
                id,
                field,
                value.to_string(),
                base.map(|v| v.to_string())
            ],
        )?;
        Ok(true)
    }

    // Membership receipts retain durable identity even when a provider mapping arrives later.
    fn normalize(self, conn: &Connection, field: &str, value: Value) -> StoreResult<Value> {
        if matches!(self, Self::Task(_)) && field == "project_id" {
            if let Some(selector) = value.as_str() {
                let id: Option<String> = conn
                    .query_row(
                        "SELECT id FROM projects WHERE id=?1 OR external_project_id=?1",
                        [selector],
                        |row| row.get(0),
                    )
                    .optional()?;
                if let Some(id) = id {
                    return Ok(Value::String(id));
                }
            }
        }
        Ok(value)
    }

    pub(super) fn pending(self, conn: &Connection) -> StoreResult<Vec<PlanningChange>> {
        let (owner, id) = self.owner();
        let mut query = conn.prepare(&format!(
            "SELECT id,field,value_json,base_json FROM {owner}_changes c
             WHERE {owner}_id=?1 AND acknowledged=0 AND conflict_json IS NULL AND seq=(SELECT max(seq) FROM {owner}_changes
                 WHERE {owner}_id=c.{owner}_id AND field=c.field) ORDER BY seq"
        ))?;
        let rows = query.query_map([id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (id, field, value, base) = row?;
            Ok(PlanningChange {
                id,
                field,
                value: serde_json::from_str(&value)?,
                base: base.map(|v| serde_json::from_str(&v)).transpose()?,
            })
        })
        .collect()
    }

    /// Unchanged baselines preserve saves; observed conflicts retire their delivery.
    /// Both values remain in the receipt. Matching reads never acknowledge writes.
    pub(super) fn reconcile<T: Serialize + DeserializeOwned>(
        self,
        conn: &Connection,
        observed: &T,
    ) -> StoreResult<T> {
        let (owner, _) = self.owner();
        let mut saved = serde_json::to_value(observed)?;
        for change in self.pending(conn)? {
            // A normal inventory cannot observe deletion.
            let Some(value) = saved.get(&change.field) else {
                continue;
            };
            let remote = self.normalize(conn, &change.field, value.clone())?;
            if remote != change.value
                && change
                    .base
                    .as_ref()
                    .is_none_or(|base| base["value"] != remote)
            {
                conn.execute(
                    &format!("UPDATE {owner}_changes SET conflict_json=?2 WHERE id=?1 AND conflict_json IS NULL"),
                    params![change.id, serde_json::json!({"revision": saved["revision"], "value": remote}).to_string()],
                )?;
            } else {
                saved[&change.field] = change.value;
            }
        }
        Ok(serde_json::from_value(saved)?)
    }
}
