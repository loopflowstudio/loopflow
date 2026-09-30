use std::num::NonZeroU32;

use rusqlite::{params, params_from_iter, types::Value, OptionalExtension};

use crate::durable::{FlowDetail, FlowFilter, FlowInventoryEntry, FlowPage};
use crate::engine::flow_graph::{project_cursor, FlowGraph};
use crate::session::FlowSummaryState;
use crate::store::{StoreError, StoreResult};

use super::flows::{flow_in, read_flow_summary, FLOW_METADATA_COLUMNS};
use super::SqliteStore;

// Wave owns bound repository identity. The nullable scalar on an unbound Flow
// records its launch repository; cwd never becomes read-time identity.
const INVENTORY_FROM: &str = "FROM flow_sessions f INDEXED BY flow_metadata
    LEFT JOIN tasks t ON t.id=f.task_id LEFT JOIN waves w ON w.id=f.wave_id";
const INVENTORY_EXTRA: &str = "COALESCE(w.repo,f.unbound_repo),
    COALESCE(t.current_invocation_id=f.id,0),f.ended_at";

fn query(filter: &FlowFilter, after: Option<&str>, limit: NonZeroU32) -> (String, Vec<Value>) {
    let mut sql = format!("WITH page AS MATERIALIZED (SELECT f.id {INVENTORY_FROM} WHERE 1");
    let mut values = Vec::new();
    let mut bind = |value| {
        values.push(value);
        format!("?{}", values.len())
    };
    for (column, value) in [
        ("COALESCE(w.repo,f.unbound_repo)", filter.repo.as_deref()),
        ("f.task_id", filter.task_id.as_ref().map(|id| id.as_str())),
        ("f.wave_id", filter.wave_id.as_ref().map(|id| id.as_str())),
    ] {
        if let Some(value) = value {
            sql.push_str(&format!(
                " AND {column}={}",
                bind(Value::Text(value.into()))
            ));
        }
    }
    if filter.taskless {
        sql.push_str(" AND f.task_id IS NULL");
    }
    if let Some(managed) = filter.managed {
        sql.push_str(&format!(
            " AND COALESCE(t.current_invocation_id=f.id,0)={}",
            bind(Value::Integer(i64::from(managed)))
        ));
    }
    if let Some(state) = filter.state {
        sql.push_str(match state {
            FlowSummaryState::Current => " AND f.state='current'",
            FlowSummaryState::Completed => " AND f.state='completed'",
            FlowSummaryState::Replaced => " AND f.state='replaced'",
        });
    }
    if let Some(search) = &filter.search {
        let search = bind(Value::Text(search.clone()));
        // Matches the existing index expression; substring search scans eligible
        // indexed names, never captured steps. '%' and '_' remain literal.
        sql.push_str(&format!(" AND (instr(lower(CASE WHEN json_valid(f.invocation_json) THEN CASE WHEN json_type(f.invocation_json,'$.flow')='text' THEN json_extract(f.invocation_json,'$.flow') END END),lower({search}))>0 OR instr(f.id,{search})>0)"));
    }
    if let Some(after) = after {
        sql.push_str(&format!(" AND f.id>{}", bind(Value::Text(after.into()))));
    }
    let limit = bind(Value::Integer(i64::from(limit.get()) + 1));
    sql.push_str(&format!(
        " ORDER BY f.id LIMIT {limit}) SELECT {FLOW_METADATA_COLUMNS},
        {INVENTORY_EXTRA} {INVENTORY_FROM} JOIN page ON page.id=f.id ORDER BY f.id"
    ));
    (sql, values)
}

fn read_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoreResult<FlowInventoryEntry>> {
    Ok((|| {
        Ok(FlowInventoryEntry {
            summary: read_flow_summary(row, 0)?.ok_or(StoreError::NotFound)?,
            repo: row.get(8)?,
            managed: row.get(9)?,
            ended_at: row.get(10)?,
        })
    })())
}

impl SqliteStore {
    pub fn flow_inventory(
        &self,
        filter: &FlowFilter,
        after: Option<&str>,
        limit: NonZeroU32,
    ) -> StoreResult<FlowPage> {
        let (sql, values) = query(filter, after, limit);
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&sql)?;
        let mut entries = query
            .query_map(params_from_iter(values), read_entry)?
            .collect::<rusqlite::Result<StoreResult<Vec<_>>>>()??;
        let next = if entries.len() > limit.get() as usize {
            entries.pop();
            entries.last().map(|entry| entry.summary.id.clone())
        } else {
            None
        };
        Ok(FlowPage { entries, next })
    }

    pub fn flow_detail(&self, selector: &str) -> StoreResult<Option<FlowDetail>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let exact: Option<String> = tx
            .query_row(
                "SELECT id FROM flow_sessions WHERE id=?1",
                [selector],
                |row| row.get(0),
            )
            .optional()?;
        let id = match exact {
            Some(id) => id,
            None => {
                let upper = format!("{selector}\u{10ffff}");
                let mut query = tx.prepare(
                    "SELECT id FROM flow_sessions WHERE id>=?1 AND id<?2 ORDER BY id LIMIT 2",
                )?;
                let ids = query
                    .query_map(params![selector, upper], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                match ids.as_slice() {
                    [] => return Ok(None),
                    [id] => id.clone(),
                    _ => {
                        return Err(StoreError::InvalidData(format!(
                            "Ambiguous FlowSession prefix {selector:?}"
                        )))
                    }
                }
            }
        };
        let entry = tx.query_row(
            &format!(
                "SELECT {FLOW_METADATA_COLUMNS},{INVENTORY_EXTRA} {INVENTORY_FROM} WHERE f.id=?1"
            ),
            [&id],
            read_entry,
        )??;
        let flow = flow_in(&tx, &id)?.ok_or(StoreError::NotFound)?;
        let graph = FlowGraph::new(&flow.invocation.flow, &flow.invocation.steps);
        let projection = project_cursor(&graph, &flow.cursor);
        let detail = FlowDetail {
            entry,
            graph,
            current: projection.current,
            completed: projection.completed,
            returns: projection.returns,
            version: flow.version,
            cwd: flow.cwd,
            failure: flow.failure,
        };
        tx.commit()?;
        Ok(Some(detail))
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use rusqlite::params;

    use crate::durable::{FlowFilter, FlowSession};
    use crate::engine::invocation::QueuedInvocation;
    use crate::engine::{ConcreteSkill, ConcreteStep, ExecutionCursor, OccurrencePolicy, Skill};
    use crate::session::FlowSummaryState;
    use crate::store::sqlite::SqliteStore;

    fn seed(store: &SqliteStore, id: &str, name: &str) {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO flow_sessions(id,invocation_json,cwd,step_index,iteration,
            position_version,worker_generation,updated_at,state)
            VALUES(?1,?2,'/missing',0,0,1,0,17,'current')",
            params![
                id,
                serde_json::json!({"id":id,"flow":name,"steps":"corrupt"}).to_string()
            ],
        )
        .unwrap();
    }

    #[test]
    fn flow_inventory_pages_metadata_and_keeps_corrupt_detail() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        for id in ["a", "ab", "b"] {
            seed(&store, id, "feature%_");
        }
        let filter = FlowFilter {
            search: Some("%_".into()),
            ..FlowFilter::default()
        };
        let size = NonZeroU32::new(2).unwrap();
        let first = store.flow_inventory(&filter, None, size).unwrap();
        assert_eq!(
            first
                .entries
                .iter()
                .map(|e| e.summary.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "ab"]
        );
        assert_eq!(first.next.as_deref().unwrap(), "ab");
        assert!(first.entries.iter().all(|e| e.repo.is_none() && !e.managed));
        assert!(store
            .flow_detail("a")
            .unwrap_err()
            .to_string()
            .contains("unreadable"));
        assert!(store
            .flow_detail("")
            .unwrap_err()
            .to_string()
            .contains("Ambiguous"));
        assert!(store.flow_detail("absent").unwrap().is_none());
        let second = store
            .flow_inventory(&filter, first.next.as_deref(), size)
            .unwrap();
        assert_eq!(second.entries[0].summary.id, "b");
        assert!(second.next.is_none());
        let conn = store.conn.lock().unwrap();
        let bytes: String = conn
            .query_row(
                "SELECT invocation_json FROM flow_sessions WHERE id='a'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&bytes).unwrap()["steps"],
            "corrupt"
        );
    }

    #[test]
    fn flow_inventory_scopes_terminal_rows_without_loading_capture() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        seed(&store, "a", "feature");
        seed(&store, "b", "feature");
        seed(&store, "c", "feature");
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE flow_sessions SET unbound_repo='/repo',state='completed',ended_at=18 WHERE id IN ('a','c')",[]).unwrap();
        }
        let filter = FlowFilter {
            repo: Some("/repo".into()),
            taskless: true,
            state: Some(FlowSummaryState::Completed),
            ..FlowFilter::default()
        };
        let first = store
            .flow_inventory(&filter, None, NonZeroU32::new(1).unwrap())
            .unwrap();
        assert_eq!(first.entries[0].summary.id, "a");
        assert_eq!(first.entries[0].ended_at, Some(18));
        let second = store
            .flow_inventory(&filter, first.next.as_deref(), NonZeroU32::new(1).unwrap())
            .unwrap();
        assert_eq!(second.entries[0].summary.id, "c");
        assert!(second.next.is_none());
    }

    #[test]
    fn flow_detail_uses_its_capture_without_template_or_checkout() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let saved = store
            .create_flow(&FlowSession {
                invocation: QueuedInvocation::new(
                    "removed-template",
                    vec![ConcreteStep::Skill(ConcreteSkill {
                        skill: Skill::named("removed-skill"),
                        flow_parents: vec![],
                        policy: OccurrencePolicy::default(),
                    })],
                )
                .unwrap(),
                cursor: ExecutionCursor::default(),
                version: 0,
                task_id: None,
                wave_id: None,
                cwd: dir.path().join("absent"),
                message: None,
                model: None,
                current_attempt: None,
                pending_session_id: None,
                ready_summary: None,
                worker_generation: 0,
                claim: None,
                failure: None,
                finished: false,
                updated_at: time::OffsetDateTime::now_utc(),
            })
            .unwrap();
        let detail = store.flow_detail(&saved.id()[..12]).unwrap().unwrap();
        assert_eq!(detail.entry.summary.id, saved.id());
        assert_eq!(detail.graph.name, "removed-template");
        assert_eq!(detail.graph.steps[0].label, "removed-skill");
        assert_eq!(detail.current, Some(0));
        assert_eq!(store.flow(saved.id()).unwrap().unwrap(), saved);
    }
    #[test]
    fn flow_inventory_distinguishes_task_selection_without_starting_done_work() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        seed(&store, "managed", "feature");
        let wave = "00000000-0000-0000-0000-000000000001";
        let task = "task_11111111111111111111111111111111";
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'wave','/repo',1)",
                [wave],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO projects(id,wave_id,external_project_id,created_at,status)
                VALUES('proj_11111111111111111111111111111111',?1,'old-plan',1,'completed')",
                [wave],
            )
            .unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at,work_state,work_terminal_at)
                VALUES(?1,'proj_11111111111111111111111111111111','old-issue','PROOF-1',1,'done',2)",[task]).unwrap();
            conn.execute(
                "UPDATE flow_sessions SET cwd=NULL,task_id=?1,wave_id=?2",
                params![task, wave],
            )
            .unwrap();
            conn.execute(
                "UPDATE tasks SET current_invocation_id='managed' WHERE id=?1",
                [task],
            )
            .unwrap();
        }
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO flow_sessions(id,invocation_json,cwd,step_index,iteration,position_version,worker_generation,updated_at,state,task_id,wave_id)
                 SELECT 'other','{\"id\":\"other\",\"flow\":\"feature\",\"steps\":\"corrupt\"}',cwd,0,0,1,0,17,'current',task_id,wave_id FROM flow_sessions WHERE id='managed'",
                [],
            )
            .unwrap();
        let id = store
            .resolve_task_id("PROOF-1", Some("/repo"))
            .unwrap()
            .unwrap();
        let mut filter = FlowFilter {
            task_id: Some(id),
            managed: Some(true),
            repo: Some("/repo".into()),
            ..FlowFilter::default()
        };
        let page = store
            .flow_inventory(&filter, None, NonZeroU32::new(10).unwrap())
            .unwrap();
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].summary.id, "managed");
        filter.managed = Some(false);
        let page = store
            .flow_inventory(&filter, None, NonZeroU32::new(10).unwrap())
            .unwrap();
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].summary.id, "other");
        let conn = store.conn.lock().unwrap();
        let state: (String, Option<i64>) = conn
            .query_row(
                "SELECT work_state,started_at FROM tasks WHERE id=?1",
                [task],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(state, ("done".into(), None));
    }
}
