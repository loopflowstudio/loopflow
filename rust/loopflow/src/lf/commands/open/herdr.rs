//! Herdr through its documented local socket protocol (0.9.3, protocol 22).
//! Layouts enter through `layout.apply` with argv; nothing is typed into a shell.
use std::io::Read;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

pub(super) struct Herdr {
    socket: String,
    /// The caller's pane, resolved to its current workspace at opening.
    pane: Option<String>,
}

impl Herdr {
    /// The current server. Herdr panes carry its socket; there is no fallback.
    pub(super) fn local() -> Result<Self> {
        let socket = std::env::var("HERDR_SOCKET_PATH")
            .context("Herdr opening requires the current server's HERDR_SOCKET_PATH")?;
        let viewed = match super::viewed_placement() {
            Some((super::OpenHost::Herdr, workspace)) => Some(workspace),
            _ => None,
        };
        // A viewed conversation's placement outranks its startup pane.
        Ok(Self {
            socket,
            pane: viewed
                .is_none()
                .then(|| std::env::var("HERDR_PANE_ID").ok())
                .flatten(),
        })
    }

    /// The workspace holding the caller's pane now, which follows pane moves.
    pub(super) fn here(&self) -> Result<Option<String>> {
        if let Some((super::OpenHost::Herdr, workspace)) = super::viewed_placement() {
            return Ok(Some(workspace));
        }
        let Some(pane) = &self.pane else {
            return Ok(None);
        };
        let current = self.call("pane.current", json!({"caller_pane_id":pane}))?;
        Ok(Some(
            current["pane"]["workspace_id"]
                .as_str()
                .context("Herdr did not identify the caller's workspace")?
                .into(),
        ))
    }

    pub(super) fn create_workspace(&self, label: &str, cwd: &Path) -> Result<(String, String)> {
        let created = self.call(
            "workspace.create",
            json!({"cwd":cwd,"label":label,"focus":false}),
        )?;
        Ok((
            created["workspace"]["workspace_id"]
                .as_str()
                .context("Herdr create omitted workspace identity; outcome is unknown")?
                .into(),
            created["tab"]["tab_id"]
                .as_str()
                .context("Herdr create omitted tab identity; outcome is unknown")?
                .into(),
        ))
    }

    pub(super) fn create_tab(&self, workspace: &str, label: &str, cwd: &Path) -> Result<String> {
        let created = self.call(
            "tab.create",
            json!({"workspace_id":workspace,"cwd":cwd,"label":label,"focus":false}),
        )?;
        Ok(created["tab"]["tab_id"]
            .as_str()
            .context("Herdr create omitted tab identity; outcome is unknown")?
            .into())
    }

    /// Apply a layout to a tab. Herdr replaces the tab; its reply names the new one.
    pub(super) fn apply(&self, tab: &str, label: &str, root: Value) -> Result<String> {
        let applied = self.call(
            "layout.apply",
            json!({"tab_id":tab,"tab_label":label,"root":root,"focus":false}),
        )?;
        Ok(first_tab(&applied).unwrap_or(tab).to_owned())
    }

    pub(super) fn focus(&self, tab: &str) -> Result<()> {
        self.call("tab.focus", json!({"tab_id":tab}))?;
        Ok(())
    }

    fn call(&self, method: &str, params: Value) -> Result<Value> {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixStream;
        let mut socket =
            UnixStream::connect(&self.socket).context("Herdr server is unavailable")?;
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
}

fn first_tab(value: &Value) -> Option<&str> {
    match value {
        Value::Object(map) => map
            .get("tab_id")
            .and_then(Value::as_str)
            .or_else(|| map.values().find_map(first_tab)),
        Value::Array(items) => items.iter().find_map(first_tab),
        _ => None,
    }
}
