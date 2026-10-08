use std::collections::HashMap;

use rusqlite::TransactionBehavior;

use crate::store::{StoreError, StoreResult};
use crate::work::task::follow_through::{
    FollowThrough, FollowThroughIntent, FollowThroughLink, FollowThroughSource,
};
use crate::work::task::{TaskEventKind, TaskId};

use super::children::{insert_task_event_in, task_events_after_in};
use super::SqliteStore;

impl SqliteStore {
    pub fn task_follow_through(&self, task: &TaskId) -> StoreResult<FollowThrough> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(FollowThrough::from_events(&task_events_after_in(
            &conn, task, 0,
        )?))
    }

    pub fn reserve_follow_through(
        &self,
        task: &TaskId,
        intent: &FollowThroughIntent,
    ) -> StoreResult<FollowThroughIntent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = FollowThrough::from_events(&task_events_after_in(&tx, task, 0)?);
        if let Some(existing) = current.intents.iter().find(|prior| prior.key == intent.key) {
            return Ok(existing.clone());
        }
        if super::durable::task_state_in(&tx, task)?.is_terminal() || current.resolved() {
            return Err(StoreError::InvalidAuthority(
                "Follow-through is already resolved; file new scope as a separate Task".into(),
            ));
        }
        insert_task_event_in(
            &tx,
            task,
            &TaskEventKind::FollowThroughIntent {
                intent: intent.clone(),
            },
        )?;
        tx.commit()?;
        Ok(intent.clone())
    }

    pub fn link_follow_through(&self, task: &TaskId, link: &FollowThroughLink) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = FollowThrough::from_events(&task_events_after_in(&tx, task, 0)?);
        if !current
            .intents
            .iter()
            .any(|intent| intent.key == link.key && intent.issue_id == link.issue_id)
        {
            return Err(StoreError::InvalidData(
                "follow-up has no matching filing intent".into(),
            ));
        }
        if !current.links.contains(link) {
            insert_task_event_in(
                &tx,
                task,
                &TaskEventKind::FollowThroughLinked { link: link.clone() },
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn finish_follow_through(
        &self,
        task: &TaskId,
        reason: &str,
        none: bool,
    ) -> StoreResult<()> {
        if reason.trim().is_empty() {
            return Err(StoreError::InvalidData(
                "follow-through needs a reason".into(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = FollowThrough::from_events(&task_events_after_in(&tx, task, 0)?);
        if current.resolved() {
            return Ok(());
        }
        if none && !current.intents.is_empty() {
            return Err(StoreError::InvalidAuthority(
                "filing intents exist; confirm them with --finish".into(),
            ));
        }
        if !none
            && (current.intents.is_empty()
                || current.intents.iter().any(|intent| {
                    !current
                        .links
                        .iter()
                        .any(|link| link.key == intent.key && link.issue_id == intent.issue_id)
                }))
        {
            return Err(StoreError::InvalidAuthority(
                "follow-up filing or relation is still unconfirmed".into(),
            ));
        }
        insert_task_event_in(
            &tx,
            task,
            &TaskEventKind::FollowThroughDisposition {
                reason: reason.trim().into(),
            },
        )?;
        tx.commit()?;
        Ok(())
    }
}

impl SqliteStore {
    /// Read confirmed source links once for the whole planning projection, including
    /// children that have never acquired a checkout or local Task row.
    pub fn follow_up_sources(&self) -> StoreResult<HashMap<String, Vec<FollowThroughSource>>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT t.external_issue_id, t.issue_identifier, e.kind_json
             FROM task_events e JOIN tasks t ON t.id = e.task_id
             WHERE json_extract(e.kind_json, '$.kind') = 'follow_through_linked'
             ORDER BY e.id",
        )?;
        let mut sources: HashMap<String, Vec<FollowThroughSource>> = HashMap::new();
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (issue_id, identifier, json) = row?;
            let event: TaskEventKind = serde_json::from_str(&json).map_err(|error| {
                StoreError::InvalidData(format!("invalid follow-through event: {error}"))
            })?;
            if let TaskEventKind::FollowThroughLinked { link } = event {
                let source = FollowThroughSource {
                    issue_id,
                    identifier,
                };
                let children = sources.entry(link.issue_id).or_default();
                if !children.contains(&source) {
                    children.push(source);
                }
            }
        }
        Ok(sources)
    }
}
