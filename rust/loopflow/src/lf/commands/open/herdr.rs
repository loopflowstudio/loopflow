//! Herdr through its documented local socket protocol (0.9.3, protocol 22).
//! Layouts enter through `layout.apply` with argv; nothing is typed into a shell.
use std::io::Read;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::work::selection::SplitAxis;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct HerdrLocation {
    socket: String,
    pane: String,
}

impl HerdrLocation {
    pub(super) fn current() -> Option<Self> {
        Some(Self {
            socket: std::env::var("HERDR_SOCKET_PATH").ok()?,
            pane: std::env::var("HERDR_PANE_ID").ok()?,
        })
    }
}

pub(super) struct Herdr {
    socket: String,
    /// The caller's pane, resolved to its current workspace at opening.
    pane: Option<String>,
}

impl Herdr {
    /// The current server. Herdr panes carry its socket; there is no fallback.
    pub(super) fn local() -> Result<Self> {
        let location = match super::viewed_placement() {
            Some((super::OpenHost::Herdr, value)) => Some(
                serde_json::from_str::<HerdrLocation>(&value)
                    .context("Herdr viewer placement is unavailable")?,
            ),
            _ => HerdrLocation::current(),
        };
        Ok(match location {
            Some(location) => Self {
                socket: location.socket,
                pane: Some(location.pane),
            },
            None => Self {
                socket: std::env::var("HERDR_SOCKET_PATH")
                    .context("Herdr opening requires HERDR_SOCKET_PATH")?,
                pane: None,
            },
        })
    }

    /// Ask the recorded pane for its current workspace, including after moves.
    pub(super) fn here(&self) -> Result<Option<String>> {
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

    pub(super) fn focus_view(placement: &str) -> Result<()> {
        let location: HerdrLocation = serde_json::from_str(placement)
            .context("The recorded Herdr view has no pane identity")?;
        let host = Self {
            socket: location.socket,
            pane: Some(location.pane.clone()),
        };
        let current = host.call("pane.current", json!({"caller_pane_id":location.pane}))?;
        let tab = current["pane"]["tab_id"]
            .as_str()
            .context("Herdr did not identify the conversation's tab")?;
        host.focus(tab)
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

// Native layout rendering.
pub(super) fn terminal(label: &str, cwd: &Path, argv: Vec<String>) -> Value {
    json!({"type":"pane","label":label,"cwd":cwd,"command":argv,"env":{}})
}

pub(super) fn split(axis: SplitAxis, ratio: f64, before: Value, after: Value) -> Value {
    json!({"type":"split",
        "direction": if axis == SplitAxis::Vertical { "right" } else { "down" },
        "ratio":ratio,"first":before,"second":after,
    })
}

#[cfg(test)]
mod tests {
    use super::{split, terminal};
    use crate::work::selection::SplitAxis;
    use std::path::Path;

    #[test]
    fn layout_preserves_command_argv_and_split_axes() {
        let pane = terminal(
            "Files",
            Path::new("/repo/task"),
            vec!["editor".into(), "a b".into()],
        );
        assert_eq!(pane["cwd"], "/repo/task");
        assert_eq!(pane["command"], serde_json::json!(["editor", "a b"]));
        let below = split(SplitAxis::Horizontal, 0.6, pane.clone(), pane.clone());
        let beside = split(SplitAxis::Vertical, 0.5, pane.clone(), below);
        assert_eq!(beside["direction"], "right");
        assert_eq!(beside["second"]["direction"], "down");
        assert_eq!(beside["second"]["second"], pane);
    }
}
