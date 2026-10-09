use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::store::{StoreError, StoreResult};
use crate::work::task::follow_through::{
    FollowThrough, FollowThroughIntent, FollowThroughLink, FollowThroughSource,
};
use crate::work::task::{TaskEvent, TaskEventKind, TaskId};

use super::children::{insert_task_event_in, task_events_after_in};
use super::SqliteStore;

#[derive(Debug)]
pub(crate) struct FollowThroughRelation {
    pub task_id: TaskId,
    pub relation_id: String,
    pub source_issue_id: String,
    pub target_issue_id: String,
}

impl SqliteStore {
    pub(crate) fn pending_follow_through_relations(
        &self,
        repo: &str,
    ) -> StoreResult<Vec<FollowThroughRelation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT DISTINCT source.id, json_extract(e.kind_json, '$.intent.relation_id'),
                source.external_issue_id, target.external_issue_id
             FROM task_events e JOIN tasks source ON source.id=e.task_id
             JOIN projects p ON p.id=source.project_id JOIN waves w ON w.id=p.wave_id
             JOIN tasks target ON target.id=json_extract(e.kind_json, '$.intent.issue_id')
                OR target.external_issue_id=json_extract(e.kind_json, '$.intent.issue_id')
             WHERE w.repo=?1 AND json_extract(e.kind_json, '$.kind')='follow_through_intent'
                AND source.external_issue_id IS NOT NULL AND target.external_issue_id IS NOT NULL
                AND source.planning_deleted_at IS NULL AND target.planning_deleted_at IS NULL
                AND EXISTS(SELECT 1 FROM task_events linked WHERE linked.task_id=source.id
                    AND json_extract(linked.kind_json, '$.kind')='follow_through_linked'
                    AND json_extract(linked.kind_json, '$.link.key')=json_extract(e.kind_json, '$.intent.key')
                    AND json_extract(linked.kind_json, '$.link.issue_id')=json_extract(e.kind_json, '$.intent.issue_id'))
                AND NOT EXISTS(SELECT 1 FROM task_events confirmed WHERE confirmed.task_id=source.id
                    AND json_extract(confirmed.kind_json, '$.kind')='follow_through_relation_confirmed'
                    AND json_extract(confirmed.kind_json, '$.relation_id')=json_extract(e.kind_json, '$.intent.relation_id')
                    AND json_extract(confirmed.kind_json, '$.source_issue_id')=source.external_issue_id
                    AND json_extract(confirmed.kind_json, '$.target_issue_id')=target.external_issue_id)",
        )?;
        let rows = query.query_map([repo], |row| {
            Ok(FollowThroughRelation {
                task_id: TaskId::from_raw(row.get::<_, String>(0)?),
                relation_id: row.get(1)?,
                source_issue_id: row.get(2)?,
                target_issue_id: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub(crate) fn confirm_follow_through_relation(
        &self,
        relation: &FollowThroughRelation,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let kind = TaskEventKind::FollowThroughRelationConfirmed {
            relation_id: relation.relation_id.clone(),
            source_issue_id: relation.source_issue_id.clone(),
            target_issue_id: relation.target_issue_id.clone(),
        };
        if !task_events_after_in(&tx, &relation.task_id, 0)?
            .iter()
            .any(|event| event.kind == kind)
        {
            insert_task_event_in(&tx, &relation.task_id, &kind)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn task_follow_through(&self, task: &TaskId) -> StoreResult<FollowThrough> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        follow_through_in(&conn, &task_events_after_in(&conn, task, 0)?)
    }

    pub(crate) fn follow_through_from_events(
        &self,
        events: &[TaskEvent],
    ) -> StoreResult<FollowThrough> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        follow_through_in(&conn, events)
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
        if !none && (current.intents.is_empty() || !current.links_confirmed()) {
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

    /// Read confirmed source links once for the whole planning projection, including
    /// children without a checkout or a locally retained historical provider issue.
    pub fn follow_up_sources(&self) -> StoreResult<HashMap<String, Vec<FollowThroughSource>>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT COALESCE(t.external_issue_id,t.id), t.issue_identifier, e.kind_json,
                    COALESCE(target.external_issue_id,target.id)
             FROM task_events e JOIN tasks t ON t.id = e.task_id
             LEFT JOIN tasks target ON target.id=json_extract(e.kind_json, '$.link.issue_id')
                OR target.external_issue_id=json_extract(e.kind_json, '$.link.issue_id')
             WHERE json_extract(e.kind_json, '$.kind') = 'follow_through_linked'
             ORDER BY e.id",
        )?;
        let mut sources: HashMap<String, Vec<FollowThroughSource>> = HashMap::new();
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        for row in rows {
            let (issue_id, identifier, json, alias) = row?;
            let event: TaskEventKind = serde_json::from_str(&json).map_err(|error| {
                StoreError::InvalidData(format!("invalid follow-through event: {error}"))
            })?;
            if let TaskEventKind::FollowThroughLinked { link } = event {
                let source = FollowThroughSource {
                    issue_id,
                    identifier,
                };
                for key in std::iter::once(link.issue_id).chain(alias) {
                    let children = sources.entry(key).or_default();
                    if !children.contains(&source) {
                        children.push(source.clone());
                    }
                }
            }
        }
        Ok(sources)
    }
}

// Filing input stays immutable; navigation and dates follow the current saved Task.
fn follow_through_in(conn: &Connection, events: &[TaskEvent]) -> StoreResult<FollowThrough> {
    let mut follow_through = FollowThrough::from_events(events);
    let mut query = conn.prepare(
        "SELECT issue_identifier,planning_url,planning_due_date FROM tasks WHERE id=?1 OR external_issue_id=?1",
    )?;
    for link in &mut follow_through.links {
        if let Some((identifier, url, due)) = query
            .query_row([&link.issue_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .optional()?
        {
            link.identifier = identifier;
            link.url = url;
            link.due = due;
        }
    }
    Ok(follow_through)
}
