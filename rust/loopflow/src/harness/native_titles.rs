//! Naming hooks for plain native conversations and their published installation.
//! Providers retain ownership of native history and terminal output.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context};
use serde_json::{json, Value};

use crate::harness::codex_connection::rpc_request;
use crate::store::sqlite::SqliteStore;

/// Native providers own their names and terminal writes; hooks only supply the name.
pub(crate) fn name_native_session(provider: &str, input: &Value) -> anyhow::Result<Option<Value>> {
    let cwd = input["cwd"]
        .as_str()
        .ok_or_else(|| anyhow!("hook has no cwd"))?;
    if !Path::new(cwd)
        .ancestors()
        .any(|path| path.join(".lf").is_dir())
    {
        return Ok(None);
    }
    if input["hook_event_name"] != "UserPromptSubmit" {
        return Ok(None);
    }
    let Some(prompt) = input["prompt"].as_str() else {
        return Ok(None);
    };
    // lf's assembled context has its own attributed name and title transport.
    if prompt == crate::engine::prompt::INITIAL_TURN_PROMPT {
        return Ok(None);
    }
    let Some(title) = crate::engine::naming::request_title(prompt) else {
        return Ok(None);
    };
    if let Some(thread) = input["session_id"].as_str() {
        let database = crate::store::database_path_from_env()?;
        if database.exists()
            && !SqliteStore::open_processes_read_only(&database)?
                .sessions_for_agent_session(&thread.into())?
                .is_empty()
        {
            return Ok(None);
        }
    }
    match provider {
        "claude" => {
            if input["session_title"]
                .as_str()
                .is_some_and(|name| !name.is_empty())
            {
                return Ok(None);
            }
            Ok(Some(json!({"hookSpecificOutput": {
                "hookEventName": "UserPromptSubmit", "sessionTitle": title
            }})))
        }
        "codex" => {
            let id = input["session_id"]
                .as_str()
                .ok_or_else(|| anyhow!("hook has no session_id"))?;
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(3), name_codex_session(id, &title))
                        .await
                        .context("native naming timed out")?
                })?;
            Ok(None)
        }
        _ => Err(anyhow!("unsupported title provider {provider}")),
    }
}

async fn name_codex_session(id: &str, title: &str) -> anyhow::Result<()> {
    let home = std::env::var_os("CODEX_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
        .ok_or_else(|| anyhow!("Codex home unavailable"))?;
    let socket =
        tokio::net::UnixStream::connect(home.join("app-server-control/app-server-control.sock"))
            .await
            .context("Codex shared app-server unavailable")?;
    let (mut connection, _) = tokio_tungstenite::client_async("ws://localhost", socket).await?;
    rpc_request(
        &mut connection,
        "initialize",
        json!({"clientInfo": {
            "name": "loopflow_titles", "version": env!("CARGO_PKG_VERSION")
        }}),
    )
    .await?;
    let detail = rpc_request(
        &mut connection,
        "thread/read",
        json!({"threadId": id, "includeTurns": false}),
    )
    .await?;
    let thread = &detail["thread"];
    if thread["name"].as_str().is_some_and(|name| !name.is_empty()) {
        return Ok(());
    }
    rpc_request(
        &mut connection,
        "thread/name/set",
        json!({"threadId": id, "name": title}),
    )
    .await?;
    Ok(())
}

/// Publish naming hooks alongside installed native integrations, preserving other hooks.
pub(crate) fn install_native_hooks(home: &Path) -> anyhow::Result<()> {
    for (provider, relative) in [
        ("claude", ".claude/settings.json"),
        ("codex", ".codex/hooks.json"),
    ] {
        let path = home.join(relative);
        let path = if path.exists() {
            path.canonicalize()?
        } else {
            path
        };
        let mut document: Value = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({}),
            Err(error) => return Err(error.into()),
        };
        let original = document.clone();
        let root = document
            .as_object_mut()
            .ok_or_else(|| anyhow!("{} must be an object", path.display()))?;
        let hooks = root
            .entry("hooks")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| anyhow!("{} hooks must be an object", path.display()))?;
        let command = format!("lf __session-title {provider}");
        let entries = hooks
            .entry("UserPromptSubmit")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| anyhow!("UserPromptSubmit hooks must be an array"))?;
        if !entries.iter().any(|entry| {
            entry["hooks"].as_array().is_some_and(|handlers| {
                handlers.iter().any(|handler| handler["command"] == command)
            })
        }) {
            entries.push(json!({"hooks": [{"type":"command", "command":command, "timeout":5}]}));
        }
        if document == original {
            continue;
        }
        let parent = path.parent().expect("native hook file has a parent");
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        if path.exists() {
            temporary
                .as_file()
                .set_permissions(fs::metadata(&path)?.permissions())?;
        }
        serde_json::to_writer_pretty(&mut temporary, &document)?;
        writeln!(temporary)?;
        temporary.persist(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{install_native_hooks, name_native_session};
    use serde_json::{json, Value};

    #[test]
    fn native_hooks_name_requests_preserving_custom_names() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".lf")).unwrap();
        let mut input = json!({"cwd":root.path(), "hook_event_name":"UserPromptSubmit",
            "prompt":"Plan store migration for archived tasks"});
        let title = name_native_session("claude", &input).unwrap().unwrap();
        assert_eq!(
            title["hookSpecificOutput"]["sessionTitle"],
            "Plan store migration"
        );
        input["session_title"] = json!("Jack's hand name");
        assert!(name_native_session("claude", &input).unwrap().is_none());
        input.as_object_mut().unwrap().remove("session_title");
        for prompt in [
            json!(crate::engine::prompt::INITIAL_TURN_PROMPT),
            json!("..."),
            Value::Null,
        ] {
            input["prompt"] = prompt;
            for provider in ["claude", "codex"] {
                assert!(name_native_session(provider, &input).unwrap().is_none());
            }
        }
        std::fs::remove_dir(root.path().join(".lf")).unwrap();
        input["prompt"] = json!("Plan store migration");
        assert!(name_native_session("claude", &input).unwrap().is_none());
    }

    #[test]
    fn native_hook_installation_keeps_other_hooks_and_is_idempotent() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join(".codex")).unwrap();
        let shared = home.path().join("shared-hooks.json");
        std::fs::write(&shared, "{}").unwrap();
        std::os::unix::fs::symlink(&shared, home.path().join(".codex/hooks.json")).unwrap();
        let path = home.path().join(".claude/settings.json");
        std::fs::create_dir(path.parent().unwrap()).unwrap();
        let existing = json!({"permissions":{"allow":["Read"]},"hooks":{
            "UserPromptSubmit":[{"hooks":[{"type":"command","command":"other-message-hook"}]}]
        }});
        std::fs::write(&path, existing.to_string()).unwrap();
        install_native_hooks(home.path()).unwrap();
        let first = std::fs::read(&path).unwrap();
        install_native_hooks(home.path()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), first);
        let installed: serde_json::Value = serde_json::from_slice(&first).unwrap();
        assert_eq!(installed["permissions"], existing["permissions"]);
        assert_eq!(
            installed["hooks"]["UserPromptSubmit"][0],
            existing["hooks"]["UserPromptSubmit"][0]
        );
        assert_eq!(
            installed["hooks"]["UserPromptSubmit"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let codex: serde_json::Value =
            serde_json::from_slice(&std::fs::read(home.path().join(".codex/hooks.json")).unwrap())
                .unwrap();
        assert!(codex["hooks"].get("SessionStart").is_none());
        assert_eq!(
            codex["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"],
            "lf __session-title codex"
        );
        assert!(installed["hooks"].get("SessionStart").is_none());
        assert!(home.path().join(".codex/hooks.json").is_symlink());
        assert_eq!(
            std::fs::read(shared).unwrap(),
            std::fs::read(home.path().join(".codex/hooks.json")).unwrap()
        );
    }
}
