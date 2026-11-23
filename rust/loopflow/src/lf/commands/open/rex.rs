//! Rex CLI adapter: resolve the caller, create native sessions and tabs, and focus them.
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::OpenHost;
use loopflow_execution::transport::local_directory_report;

use loopflow_execution::terminal_host::{rex_command, rex_endpoint, RexLocation};

#[derive(Debug, Clone)]
pub(super) struct Rex {
    pub(super) server: String,
    pub(super) caller: Option<RexLocation>,
}

impl Rex {
    /// Rex's login wrapper hides a foreground tool's cwd. Report it so native
    /// splits inherit the tool's checkout. Shells use `terminal` instead: their
    /// process cwd follows `cd`, while a launch-time report would become stale.
    pub(super) fn block(&self, label: &str, cwd: &Path, argv: Vec<String>) -> Value {
        let argv = match local_directory_report(cwd) {
            Some(report) => [
                "/bin/sh".into(),
                "-c".into(),
                r#"printf %s "$1"; shift; exec "$@""#.into(),
                "lf-directory".into(),
                report,
            ]
            .into_iter()
            .chain(argv)
            .collect(),
            None => argv,
        };
        self.terminal(label, cwd, argv)
    }

    pub(super) fn terminal(&self, label: &str, cwd: &Path, mut argv: Vec<String>) -> Value {
        // Rex omits its endpoint from native child environments.
        argv.splice(0..0, ["env".into(), format!("REX_SERVER={}", self.server)]);
        json!({"block":{"flavor":"com.superlogical.terminal.shell","label":label,"options":{"cwd":cwd,"shell":"none","command":argv}}})
    }

    fn call(&self, method: &str, args: Value) -> Result<Value> {
        let mut command = rex_command(
            self.caller
                .as_ref()
                .and_then(|caller| caller.bin_dir.as_deref()),
        );
        command.args(["--autostart=false", "--timeout", "2s", "-S", &self.server]);
        command.args(["api", "call", method, &serde_json::to_string(&args)?]);
        let output = super::run_host(command, "Rex").with_context(|| format!("Rex {method}"))?;
        serde_json::from_str(&output).context("Rex reply is unreadable; outcome is unknown")
    }

    /// The current viewer's Rex, else the caller's terminal, else `--server`.
    pub(super) fn local(server: Option<&str>) -> Result<Self> {
        let location = match super::viewed_placement() {
            Some((OpenHost::Rex, value)) => Some(
                serde_json::from_str::<RexLocation>(&value)
                    .context("Current Rex viewer placement is unreadable")?
                    .resolve()?,
            ),
            Some(_) => None,
            None if super::in_rex() && loopflow_execution::journal::agent_caller().is_none() => {
                Some(RexLocation::current()?)
            }
            None => None,
        };
        let location =
            location.filter(|location| server.is_none_or(|server| server == location.server));
        let endpoint = match &location {
            Some(location) => location.server.clone(),
            None => rex_endpoint(server, None)?,
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

pub(super) fn focus_view(placement: &str) -> Result<()> {
    let location: RexLocation = serde_json::from_str(placement)?;
    let rex = Rex {
        server: location.server.clone(),
        caller: Some(location.clone()),
    };
    let windows = rex.windows(&location.session)?;
    let window = windows
        .iter()
        .find(|window| {
            window["layers"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|layer| layer["blocks"].as_array().into_iter().flatten())
                .any(|block| block["block_id"] == location.block)
        })
        .context("The recorded Rex view no longer exists")?;
    let id = window["window_id"]
        .as_str()
        .context("Rex view omitted window identity")?;
    let result = rex.focus(&location.session, id)?;
    anyhow::ensure!(!result.contains("unconfirmed"), "{result}");
    Ok(())
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
    use super::array;
    use serde_json::json;

    #[test]
    fn missing_inventory_is_not_an_empty_host() {
        assert!(array(json!({}), "windows").is_err());
        assert!(array(json!({"windows":null}), "windows")
            .unwrap()
            .is_empty());
    }
}
