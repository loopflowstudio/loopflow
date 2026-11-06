//! Rex CLI adapter: resolve the caller, create native sessions and tabs, and focus them.
use std::collections::BTreeSet;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::OpenHost;
use crate::journal::OsProcess;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct RexLocation {
    pub server: String,
    pub session: String,
    pub block: String,
}

impl RexLocation {
    pub(super) fn current() -> Result<Self> {
        let server = match std::env::var("REX_SERVER").ok() {
            Some(server) => server,
            None => caller_server()?.context(
                "Rex caller server is unavailable from its process ancestry; select it with --server",
            )?,
        };
        Self {
            server,
            session: std::env::var("REX_SESSION").context("Rex caller session is missing")?,
            block: std::env::var("REX_BLOCK").context("Rex caller block is missing")?,
        }
        .resolve()
    }

    fn resolve(mut self) -> Result<Self> {
        self.server = Rex::endpoint(Some(&self.server))?;
        Ok(self)
    }
}

/// Native Rex shells descend from the serving process, even through login and
/// tool shells. Its explicit listener is destination evidence; a running-app or
/// socket inventory is not. Current conversation placement is resolved first.
fn caller_server() -> Result<Option<String>> {
    let mut pid = std::process::id();
    let mut seen = BTreeSet::new();
    while pid > 1 && seen.insert(pid) {
        let Some(process) = OsProcess::read(pid)? else {
            break;
        };
        if OsProcess::executable(pid)
            .is_some_and(|path| path.file_name().is_some_and(|name| name == "rex"))
        {
            let args = OsProcess::arguments(pid)?
                .context("Rex caller server arguments are unavailable")?;
            if args.windows(2).any(|pair| pair == ["server", "run"]) {
                return Ok(listening_endpoint(&args));
            }
        }
        pid = process.ppid;
    }
    Ok(None)
}

fn listening_endpoint(args: &[String]) -> Option<String> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--listen" {
            return args.next().cloned();
        }
        if let Some(endpoint) = arg.strip_prefix("--listen=") {
            return Some(endpoint.into());
        }
    }
    None
}

#[derive(Debug, Clone)]
pub(super) struct Rex {
    pub(super) server: String,
    pub(super) caller: Option<RexLocation>,
}

impl Rex {
    fn call(&self, method: &str, args: Value) -> Result<Value> {
        let mut command = Command::new("rex");
        command.args(["--autostart=false", "--timeout", "2s", "-S", &self.server]);
        command.args(["api", "call", method, &serde_json::to_string(&args)?]);
        let output = super::run_host(command, "Rex")
            .map_err(|error| anyhow::anyhow!("Rex {method}: {}", error.reason()))?;
        serde_json::from_str(&output).context("Rex reply is unreadable; outcome is unknown")
    }

    /// The current viewer's Rex, else the caller's terminal, else `--server`.
    pub(super) fn local(server: Option<&str>) -> Result<Self> {
        let location = if server.is_some() {
            None
        } else {
            match super::viewed_placement() {
                Some((OpenHost::Rex, value)) => Some(
                    serde_json::from_str::<RexLocation>(&value)
                        .context("Current Rex viewer placement is unreadable")?
                        .resolve()?,
                ),
                Some(_) => None,
                None if super::in_rex() => {
                    anyhow::ensure!(
                        crate::journal::agent_caller().is_none(),
                        "Current conversation viewer is unavailable; refusing stale Rex startup placement"
                    );
                    Some(RexLocation::current()?)
                }
                None => None,
            }
        };
        let endpoint = match &location {
            Some(location) => location.server.clone(),
            None => Self::endpoint(server)?,
        };
        let rex = Self {
            server: endpoint,
            caller: location,
        };
        if let Some(location) = &rex.caller {
            let windows = rex.windows(&location.session)?;
            let caller_exists = windows
                .iter()
                .flat_map(|window| window["layers"].as_array().into_iter().flatten())
                .flat_map(|layer| layer["blocks"].as_array().into_iter().flatten())
                .any(|block| block["block_id"] == location.block);
            anyhow::ensure!(
                caller_exists,
                "The current Rex caller block is unavailable; nothing was opened"
            );
        }
        Ok(rex)
    }

    fn endpoint(server: Option<&str>) -> Result<String> {
        let mut command = Command::new("rex");
        for name in ["REX_SERVER", "REX_SESSION", "REX_BLOCK"] {
            command.env_remove(name);
        }
        if let Some(server) = server {
            command.args(["-S", server]);
        }
        command.args(["--autostart=false", "--timeout", "2s", "whoami", "--json"]);
        let output = super::run_host(command, "Rex")
            .map_err(|error| anyhow::anyhow!("{}", error.reason()))?;
        let who: Value = serde_json::from_str(&output)?;
        let server = who["endpoint"]
            .as_str()
            .context("Rex did not identify its server")?;
        // Unix peer authentication identifies this Machine's account. A remotely
        // selected default is never accepted as the local Task execution host.
        anyhow::ensure!(server.starts_with("unix://") && who["principal"]["transport"] == "unix", "Rex is connected to another Machine; run lf open on the Task's recorded Machine with its local Rex server");
        Ok(server.into())
    }

    pub(super) fn create_session(&self, name: &str) -> Result<String> {
        let created = self.call("session.create", json!({"label":name,"initial_windows":[]}))?;
        Ok(created["session_id"]
            .as_str()
            .context("Rex create reply omitted session identity; outcome is unknown")?
            .into())
    }

    pub(super) fn create_window(&self, session: &str, name: &str, layout: Value) -> Result<String> {
        let created = self.call(
            "session.new_window",
            json!({"session_id":session,"window_label":name,"focus":false,"layout":layout}),
        )?;
        Ok(created["window_id"]
            .as_str()
            .context("Rex create reply omitted window identity; outcome is unknown")?
            .into())
    }

    pub(super) fn sessions(&self) -> Result<Vec<Value>> {
        array(self.call("session.list", json!({}))?, "sessions")
    }

    fn windows(&self, container: &str) -> Result<Vec<Value>> {
        let view = self.call("session.view", json!({"session_id":container}))?;
        // Rex omits windows for an empty session. Its identity and revision
        // distinguish a complete empty view from an incomplete reply.
        if view.get("windows").is_none()
            && view["session_id"] == container
            && view["revision"].is_u64()
        {
            return Ok(Vec::new());
        }
        array(view, "windows")
    }

    pub(super) fn focus(&self, container: &str, window: &str) -> Result<String> {
        self.call(
            "session.focus_window",
            json!({"session_id":container,"window_id":window}),
        )?;
        let clients = array(self.call("client.list", json!({}))?, "clients")?;
        let mut candidates = Vec::new();
        for client in clients {
            // The CLI performing client.list appears in its own reply and exits
            // before the next request. It cannot display the workspace.
            if client["info"]["kind"] == "cli" {
                continue;
            }
            let Some(id) = client["client_id"].as_str() else {
                continue;
            };
            let detail = self.call("client.inspect", json!({"client_id":id}))?;
            if detail["actions"].as_array().is_some_and(|actions| {
                actions
                    .iter()
                    .any(|action| action["name"] == "session.select")
            }) {
                candidates.push(id.to_string());
            }
        }
        if candidates.len() != 1 {
            return Ok(
                "Materialized in Rex; app visibility is unconfirmed (no unique capable client)"
                    .into(),
            );
        }
        self.call("client.perform", json!({"client_id":candidates[0],"action":"session.select","args":{"session_id":container,"window_id":window}}))?;
        Ok("Rex accepted display selection".into())
    }
}

fn array(mut value: Value, key: &str) -> Result<Vec<Value>> {
    match value.as_object_mut().and_then(|object| object.remove(key)) {
        Some(Value::Array(items)) => Ok(items),
        Some(Value::Null) => Ok(Vec::new()),
        _ => bail!("Host inventory omitted {key}; absence is unknown"),
    }
}

#[cfg(test)]
mod tests {
    use super::{array, listening_endpoint};
    use serde_json::json;

    #[test]
    fn caller_listener_preserves_argument_boundaries() {
        for args in [
            vec![
                "rex",
                "server",
                "run",
                "--listen",
                "unix:///Task servers/rex.sock",
            ],
            vec![
                "rex",
                "server",
                "run",
                "--listen=unix:///Task servers/rex.sock",
            ],
        ] {
            assert_eq!(
                listening_endpoint(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()),
                Some("unix:///Task servers/rex.sock".into())
            );
        }
        assert_eq!(
            listening_endpoint(&["--config".into(), "/rex.lua".into()]),
            None
        );
    }

    #[test]
    fn missing_inventory_is_not_an_empty_host() {
        assert!(array(json!({}), "windows").is_err());
        assert!(array(json!({"windows":null}), "windows")
            .unwrap()
            .is_empty());
    }
}
