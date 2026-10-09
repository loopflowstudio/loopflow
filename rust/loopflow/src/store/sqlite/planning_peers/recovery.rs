//! Read-only recovery of selected origins and their retained private dependencies.
//! This view must never be used as an export: inspection grants no membership.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{params, Connection, OptionalExtension};

use crate::engine::planning_exchange::{
    winning_heads_by, PlanningKind, PlanningObject, PlanningSnapshot,
};
use crate::store::{PeerPlanningRecord, PeerPlanningValue, StoreResult};

use super::{
    belongs_elsewhere, invalid, member_destination, read_mutation, references, resolved_owners,
};

pub(super) fn records(
    conn: &Connection,
    repo: &str,
    mut snapshot: PlanningSnapshot,
) -> StoreResult<Vec<PeerPlanningRecord>> {
    let mut pending: BTreeSet<_> = snapshot.objects().into_iter().cloned().collect();
    let mut visited = BTreeSet::new();
    let mut outside = BTreeSet::new();
    let mut query = conn.prepare_cached(
        "SELECT kind,object_id,field,value,clock,linear,parents,id
         FROM planning_peer_changes WHERE kind=?1 AND object_id=?2",
    )?;
    while let Some(object) = pending.pop_first() {
        if !visited.insert(object.clone()) {
            continue;
        }
        if belongs_elsewhere(conn, &object, repo)? {
            outside.insert(object);
            continue;
        }
        let mut rows = query.query(params![object.kind.as_str(), object.id])?;
        while let Some(row) = rows.next()? {
            let change = read_mutation(row)?;
            pending.extend(references(&change));
            // Causal links may retain losing values under another origin.
            for parent in &change.parents {
                let origin = conn
                    .query_row(
                        "SELECT kind,object_id FROM planning_peer_changes WHERE id=?1",
                        [parent],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?
                    .ok_or_else(|| invalid("missing recovery predecessor"))?;
                pending.insert(PlanningObject {
                    kind: serde_json::from_value(origin.0.into())?,
                    id: origin.1,
                });
            }
            snapshot.changes.insert(row.get(7)?, change);
        }
        let mut associations = conn.prepare_cached(
            "SELECT origin_id,COALESCE(task_id,project_id) FROM planning_associations
             WHERE repo=?1 AND kind=?2 AND (origin_id=?3 OR COALESCE(task_id,project_id)=?3)",
        )?;
        let pairs = associations
            .query_map(params![repo, object.kind.as_str(), object.id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
        for pair in pairs {
            let (origin, local) = pair?;
            for id in [origin, local] {
                pending.insert(PlanningObject {
                    kind: object.kind,
                    id,
                });
            }
        }
        if object.kind == PlanningKind::Task {
            // Include private comments and old memberships, not just the current thread.
            let mut comments = conn.prepare_cached(
                "SELECT id FROM task_comments WHERE task_id=?1
                 UNION SELECT object_id FROM planning_peer_changes
                 WHERE kind='comment' AND field='task_id'
                 AND CASE WHEN json_valid(value) THEN json_extract(value,'$') END=?1",
            )?;
            for id in comments.query_map([&object.id], |row| row.get::<_, String>(0))? {
                pending.insert(PlanningObject {
                    kind: PlanningKind::Comment,
                    id: id?,
                });
            }
        }
    }
    snapshot.validate().map_err(invalid)?;
    let owners = resolved_owners(conn, repo, &snapshot)?;
    let candidates: BTreeSet<_> =
        winning_heads_by(snapshot.frontier_by(|object| &owners[object]), |change| {
            &owners[&change.object]
        })
        .map(|(id, _)| id)
        .collect();
    let mut records = BTreeMap::<_, PeerPlanningRecord>::new();
    for (id, change) in &snapshot.changes {
        // Receipt payloads remain with the common sync reader. Show only planning values.
        if outside.contains(&change.object)
            || !change.object.kind.fields().contains(&change.field.as_str())
        {
            continue;
        }
        let record = match records.entry(change.object.clone()) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => entry.insert(PeerPlanningRecord {
                object: change.object.clone(),
                local_id: owners[&change.object].id.clone(),
                destination: member_destination(conn, repo, &change.object)?,
                references: Vec::new(),
                values: Vec::new(),
            }),
        };
        record.references.extend(references(change));
        record.values.push(PeerPlanningValue {
            id: id.clone(),
            field: change.field.clone(),
            value_json: serde_json::to_string(&change.value)?,
            candidate: candidates.contains(id.as_str()),
            author: if change.object.kind == PlanningKind::Comment && change.field == "content" {
                change.value["author"]
                    .as_str()
                    .map(serde_json::from_str)
                    .transpose()?
            } else {
                None
            },
            observed_at: change.linear.as_ref().map(|fact| fact.observed_at),
        });
    }
    for record in records.values_mut() {
        record.references.sort();
        record.references.dedup();
        record
            .values
            .sort_by(|a, b| (&a.field, !a.candidate, &a.id).cmp(&(&b.field, !b.candidate, &b.id)));
    }
    Ok(records.into_values().collect())
}
