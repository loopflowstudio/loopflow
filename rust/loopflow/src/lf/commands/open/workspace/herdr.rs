use std::io::Read;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::super::{Opened, RouteError};
use super::{
    array, identity, layout, now, scope, HostWorkspace, Presentation, Resolved, SqliteStore,
};

// Herdr's bounded adapter uses its documented local socket protocol. Layouts
// enter through layout.apply with argv; nothing is typed into a shell.
pub(in super::super) fn open_herdr(target: &Resolved, focus: bool) -> Result<Opened, RouteError> {
    herdr(target, focus).map_err(|error| RouteError::Unknown(format!("{error:#}")))
}

fn herdr(target: &Resolved, focus: bool) -> Result<Opened> {
    let socket = std::env::var("HERDR_SOCKET_PATH")
        .context("Herdr workspace support requires the current server's HERDR_SOCKET_PATH")?;
    anyhow::ensure!(
        target.execution_machine.is_none() && target.execution_unavailable.is_none(),
        "Herdr must run on the Task's recorded Machine; no local fallback"
    );
    let task = target
        .task
        .as_ref()
        .context("Herdr workspace support currently opens one explicitly selected Task")?;
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
    let selected = scope(&store, target)?;
    let plan = store.plan_selection(&task.repository_id, selected.clone())?;
    let content = plan
        .contents
        .iter()
        .find(|item| item.work.id == task.id.as_str())
        .context("Task is unavailable")?;
    let id = identity(&format!("herdr\n{socket}\n{}", task.id));
    let mut workspace = store.host_workspace(&id)?.unwrap_or(HostWorkspace {
        id: id.clone(),
        repository_id: task.repository_id.to_string(),
        host: "herdr".into(),
        server: socket.clone(),
        selection: selected,
        copy_name: None,
        container_id: None,
        creation_pending: false,
        updated_at: now(),
    });
    let items = array(
        herdr_call(&socket, "workspace.list", json!({}))?,
        "workspaces",
    )?;
    if let Some(container) = &workspace.container_id {
        if items.iter().any(|item| item["workspace_id"] == *container) {
            anyhow::ensure!(!workspace.creation_pending, "Herdr layout creation is uncertain; inspect its retained workspace before retrying");
            if focus {
                herdr_call(
                    &socket,
                    "workspace.focus",
                    json!({"workspace_id":container}),
                )?;
            }
            return Ok(Opened::opening(Some(container.clone())));
        }
    }
    anyhow::ensure!(
        !workspace.creation_pending || workspace.container_id.is_some(),
        "Herdr creation is uncertain; no duplicate workspace was attempted"
    );
    let (native, _) = layout(
        &store,
        content,
        &target.repository_path,
        Presentation {
            session: target.session.as_deref(),
            diff: target.url.diff,
            ..Presentation::default()
        },
    )?;
    workspace.container_id = None;
    workspace.creation_pending = true;
    store.save_host_workspace(&workspace)?;
    let created = herdr_call(
        &socket,
        "workspace.create",
        json!({"cwd":content.location.as_ref().map_or(target.repository_path.as_path(),|location|Path::new(&location.cwd)),"label":format!("{} [lf:{id}]",target.destination_name()),"focus":false}),
    )?;
    let container = created["workspace"]["workspace_id"]
        .as_str()
        .context("Herdr create omitted workspace identity")?
        .to_string();
    let tab = created["tab"]["tab_id"]
        .as_str()
        .context("Herdr create omitted tab identity")?;
    workspace.container_id = Some(container.clone());
    store.save_host_workspace(&workspace)?;
    herdr_call(
        &socket,
        "layout.apply",
        json!({"tab_id":tab,"root":herdr_layout(&native)?,"focus":false}),
    )?;
    // Herdr reopens the whole workspace; its replacement tab ID is not a locator.
    workspace.creation_pending = false;
    workspace.updated_at = now();
    store.save_host_workspace(&workspace)?;
    if focus {
        herdr_call(
            &socket,
            "workspace.focus",
            json!({"workspace_id":container}),
        )?;
    }
    Ok(Opened::opening(Some(container)))
}

fn herdr_layout(native: &Value) -> Result<Value> {
    if let Some(block) = native.get("block") {
        return Ok(
            json!({"type":"pane","label":block["label"],"cwd":block["options"]["cwd"],"command":block["options"]["command"],"env":{}}),
        );
    }
    let split = native.get("split").context("Tool layout is unavailable")?;
    Ok(
        json!({"type":"split","direction":if split["direction"] == "horizontal" {"right"} else {"down"},"ratio":split["ratio"],"first":herdr_layout(&split["before"] )?,"second":herdr_layout(&split["after"])?}),
    )
}

fn herdr_call(path: &str, method: &str, params: Value) -> Result<Value> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    let mut socket = UnixStream::connect(path).context("Herdr server is unavailable")?;
    socket.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
    socket.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;
    let id = uuid::Uuid::new_v4().to_string();
    serde_json::to_writer(
        &mut socket,
        &json!({"id":id,"method":method,"params":params}),
    )?;
    socket.write_all(b"\n")?;
    let mut line = String::new();
    BufReader::new(socket)
        .take(4 * 1024 * 1024)
        .read_line(&mut line)
        .context("Herdr did not acknowledge the request; outcome is unknown")?;
    let reply: Value = serde_json::from_str(&line)?;
    anyhow::ensure!(
        reply["id"] == id,
        "Herdr reply identity did not match; outcome is unknown"
    );
    if let Some(error) = reply.get("error") {
        bail!("Herdr {method}: {error}");
    }
    reply
        .get("result")
        .cloned()
        .context("Herdr reply has no result; outcome is unknown")
}
