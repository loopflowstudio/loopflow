use super::{invalid, SqliteStore};
use crate::durable::RepositoryId;
use crate::store::{StoreError, StoreResult};
use crate::work::selection::{
    PaneIntent, PaneTree, WorkPaneLayout, WorkWorkspaceKey, WorkWorkspaceKind,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::collections::BTreeSet;

fn kind_name(kind: &WorkWorkspaceKind) -> &'static str {
    match kind {
        WorkWorkspaceKind::Repository => "repository",
        WorkWorkspaceKind::Wave => "wave",
        WorkWorkspaceKind::Task => "task",
    }
}

// Pane persistence needs Work ownership, not selection membership or Session activity.
fn validate_owner(conn: &Connection, work: &WorkWorkspaceKey) -> StoreResult<()> {
    RepositoryId::parse(&work.repository).map_err(invalid)?;
    let sql: String = match work.kind {
        WorkWorkspaceKind::Repository =>
            "SELECT EXISTS(SELECT 1 FROM repository_plans WHERE id=?1 AND id=?2)".into(),
        WorkWorkspaceKind::Wave =>
            "SELECT EXISTS(SELECT 1 FROM wave_addresses w JOIN repository_plans r ON r.repo=w.repo WHERE r.id=?1 AND w.id=?2 AND w.retired_at IS NULL)".into(),
        WorkWorkspaceKind::Task =>
            format!("SELECT EXISTS(SELECT 1 FROM tasks t LEFT JOIN projects p ON p.id=t.project_id LEFT JOIN waves w ON w.id=p.wave_id WHERE t.repository_id=?1 AND t.id=?2 AND w.retired_at IS NULL AND {})", super::super::children::TASK_VISIBLE),
    };
    let belongs: bool =
        conn.query_row(&sql, params![work.repository, work.id], |row| row.get(0))?;
    if belongs {
        Ok(())
    } else {
        Err(StoreError::NotFound)
    }
}

pub(super) fn layout_in(
    conn: &Connection,
    work: &WorkWorkspaceKey,
) -> StoreResult<Option<WorkPaneLayout>> {
    let json: Option<String> = conn.query_row(
        "SELECT layout_json FROM work_pane_layouts WHERE repository_id=?1 AND kind=?2 AND work_id=?3",
        params![work.repository,kind_name(&work.kind),work.id], |row| row.get(0),
    ).optional()?;
    json.map(|json| serde_json::from_str(&json).map_err(StoreError::from))
        .transpose()
}

fn validate_sessions(
    conn: &Connection,
    work: &WorkWorkspaceKey,
    tree: &PaneTree,
) -> StoreResult<()> {
    for (_, content) in tree.panes() {
        let PaneIntent::Session {
            session_id: Some(session_id),
            ..
        } = content
        else {
            continue;
        };
        let scope = match work.kind {
            WorkWorkspaceKind::Repository => "s.primary_scope='repository'".to_string(),
            WorkWorkspaceKind::Wave => "s.primary_scope='wave' AND s.wave_id=?3".to_string(),
            WorkWorkspaceKind::Task => {
                format!("s.id IN ({})", super::super::task_work::session_ids("?3"))
            }
        };
        let belongs: bool = conn.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM agent_sessions s JOIN repository_plans r ON r.repo=coalesce(s.repo,(SELECT repo FROM waves WHERE id=s.wave_id)) WHERE r.id=?1 AND s.id=?2 AND ({scope}) AND ?3 IS NOT NULL)"),
            params![work.repository,session_id,work.id], |row| row.get(0),
        )?;
        if !belongs {
            return Err(invalid("Session is unavailable in this repository"));
        }
    }
    Ok(())
}

impl SqliteStore {
    pub fn pane_layout(&self, work: &WorkWorkspaceKey) -> StoreResult<Option<WorkPaneLayout>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        validate_owner(&snapshot, work)?;
        layout_in(&snapshot, work)
    }

    /// Zero creates intent; subsequent writes require the last observed revision.
    pub fn save_pane_layout(&self, layout: &WorkPaneLayout) -> StoreResult<WorkPaneLayout> {
        layout.validate().map_err(invalid)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_owner(&tx, &layout.work)?;
        let prior = layout_in(&tx, &layout.work)?;
        if prior.as_ref().map_or(0, |prior| prior.revision) != layout.revision {
            return Err(invalid("pane layout changed; read it again before editing"));
        }
        validate_sessions(&tx, &layout.work, &layout.tree)?;
        let mut saved = layout.clone();
        saved.revision += 1;
        tx.execute("INSERT INTO work_pane_layouts(repository_id,kind,work_id,layout_json) VALUES(?1,?2,?3,?4) ON CONFLICT(repository_id,kind,work_id) DO UPDATE SET layout_json=excluded.layout_json",
            params![saved.work.repository,kind_name(&saved.work.kind),saved.work.id,serde_json::to_string(&saved)?])?;
        tx.commit()?;
        Ok(saved)
    }
}

// ── Draft `designated_panes`: free pane trees become designated panes ──

/// Rewrites every stored layout from the released free tree (`empty`, `session`,
/// `shell`, `files`, `log`) to designated panes. Files and Log merge into one
/// browser pane at the first one's place, empty and extra session panes leave
/// the layout, and missing designated panes are added hidden. Drafts are kept.
pub(crate) fn convert_released_layouts(conn: &Connection) -> StoreResult<()> {
    let rows = conn
        .prepare("SELECT repository_id,kind,work_id,layout_json FROM work_pane_layouts")?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (repository, kind, work, json) in rows {
        let released: serde_json::Value = serde_json::from_str(&json)?;
        let layout = convert_released_layout(released).map_err(|error| {
            StoreError::InvalidData(format!("pane layout for {kind} {work}: {error}"))
        })?;
        conn.execute(
            "UPDATE work_pane_layouts SET layout_json=?4 WHERE repository_id=?1 AND kind=?2 AND work_id=?3",
            params![repository, kind, work, serde_json::to_string(&layout)?],
        )?;
    }
    Ok(())
}

fn convert_released_layout(mut released: serde_json::Value) -> Result<WorkPaneLayout, String> {
    use crate::work::selection::{BrowserTab, SplitAxis};
    use serde_json::{json, Value};

    let work: WorkWorkspaceKey =
        serde_json::from_value(released["work"].take()).map_err(|error| error.to_string())?;
    let task = work.kind == WorkWorkspaceKind::Task;
    let mut session: Option<String> = None;
    let mut shell: Option<String> = None;
    let mut browser: Option<String> = None;
    let mut browser_tabs: Vec<Value> = Vec::new();
    let mut browser_location = Value::Null;

    // Rewrite leaves in layout order; a leaf that leaves the layout returns None.
    fn rewrite(
        node: Value,
        visit: &mut dyn FnMut(String, Value) -> Option<Value>,
    ) -> Option<Value> {
        match node["kind"].as_str() {
            Some("split") => {
                let mut node = node;
                let first = rewrite(node["first"].take(), visit);
                let second = rewrite(node["second"].take(), visit);
                match (first, second) {
                    (Some(first), Some(second)) => {
                        node["first"] = first;
                        node["second"] = second;
                        Some(node)
                    }
                    (one, other) => one.or(other),
                }
            }
            _ => {
                let id = node["id"].as_str().unwrap_or_default().to_string();
                let content = visit(id.clone(), node["content"].clone())?;
                Some(json!({"kind": "pane", "id": id, "content": content}))
            }
        }
    }
    let tree = rewrite(
        released["tree"].take(),
        &mut |id, content| match content["kind"].as_str() {
            Some("session") if session.is_none() => {
                session = Some(id);
                Some(content)
            }
            Some("shell") if task && shell.is_none() => {
                shell = Some(id);
                Some(content)
            }
            Some(kind @ ("files" | "log")) if task => {
                if browser_location.is_null() {
                    browser_location = content["location"].clone();
                }
                browser_tabs.push(if kind == "files" {
                    json!({"kind": "files", "id": "files", "tabs": content["tabs"], "selected": content["selected"]})
                } else {
                    json!({"kind": "graph_log", "id": "graph-log"})
                });
                if browser.is_some() {
                    return None;
                }
                browser = Some(id);
                Some(json!({"kind": "browser", "location": null, "tabs": [], "selected": null}))
            }
            _ => None,
        },
    );

    let mut hidden: BTreeSet<String> =
        serde_json::from_value(released["hidden"].take()).map_err(|error| error.to_string())?;
    let mut tree = tree;
    let add = |tree: &mut Option<Value>,
               id: &str,
               content: Value,
               hidden: &mut BTreeSet<String>| {
        let pane = json!({"kind": "pane", "id": id, "content": content});
        *tree = Some(match tree.take() {
            Some(existing) => {
                hidden.insert(id.to_string());
                json!({"kind": "split", "axis": SplitAxis::Vertical, "ratio": 0.5, "first": existing, "second": pane})
            }
            None => pane,
        });
    };
    if session.is_none() {
        add(
            &mut tree,
            "session",
            json!({"kind": "session", "session_id": null, "location": null}),
            &mut hidden,
        );
    }
    if task && shell.is_none() {
        add(
            &mut tree,
            "shell",
            json!({"kind": "shell", "tabs": [], "selected": null}),
            &mut hidden,
        );
    }
    if task && browser.is_none() {
        add(
            &mut tree,
            "browser",
            json!({"kind": "browser", "location": null, "tabs": [], "selected": null}),
            &mut hidden,
        );
        browser = Some("browser".into());
    }
    let mut tree: PaneTree = serde_json::from_value(tree.expect("a session pane was added"))
        .map_err(|error| error.to_string())?;
    if let Some(browser) = &browser {
        let tabs: Vec<BrowserTab> = serde_json::from_value(Value::Array(browser_tabs))
            .map_err(|error| error.to_string())?;
        let selected = tabs.first().map(|tab| tab.id().to_string());
        let location =
            serde_json::from_value(browser_location).map_err(|error| error.to_string())?;
        set_content(
            &mut tree,
            browser,
            PaneIntent::Browser {
                location,
                tabs,
                selected,
            },
        );
    }
    let ids: BTreeSet<String> = tree.panes().iter().map(|(id, _)| id.to_string()).collect();
    hidden.retain(|id| ids.contains(id));
    if hidden.len() == ids.len() {
        hidden.remove(session.as_deref().unwrap_or("session"));
    }
    let visible = |id: &Option<String>| {
        id.clone()
            .filter(|id| ids.contains(id) && !hidden.contains(id))
    };
    let focused: Option<String> =
        serde_json::from_value(released["focused"].take()).map_err(|error| error.to_string())?;
    let zoomed: Option<String> =
        serde_json::from_value(released["zoomed"].take()).map_err(|error| error.to_string())?;
    let layout = WorkPaneLayout {
        work,
        revision: released["revision"].as_i64().ok_or("revision is missing")?,
        focused: visible(&focused),
        zoomed: visible(&zoomed),
        hidden,
        drafts: serde_json::from_value(released["drafts"].take())
            .map_err(|error| error.to_string())?,
        tree,
    };
    layout.validate()?;
    Ok(layout)
}

fn set_content(tree: &mut PaneTree, target: &str, intent: PaneIntent) {
    match tree {
        PaneTree::Pane { id, content } if id == target => *content = intent,
        PaneTree::Pane { .. } => {}
        PaneTree::Split { first, second, .. } => {
            set_content(first, target, intent.clone());
            set_content(second, target, intent);
        }
    }
}
