//! Stable provider hook declarations and separately saved launch context. No user files
//! or host hook trust decisions are replaced; Claude folder trust is granted only by
//! `claude_trust`, for lf-created worktrees. Saved sources survive driver replacement.
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::context_block::ContextDelivery;
use crate::prompt::write_prompt_log;

pub(crate) fn claude_args(context: &ContextDelivery) -> Result<Vec<String>> {
    let mut hooks = context.hook_settings()?;
    hooks["PostToolUseFailure"] = hooks["PostToolUse"].clone();
    let settings = json!({"hooks": hooks});
    let bytes = serde_json::to_vec(&settings)?;
    let path = save_hook(&context.home, "claude", "json", &bytes)?;
    Ok(vec!["--settings".into(), path.display().to_string()])
}

/// Codex runs thread-configured hooks only when that configuration also trusts
/// them, and its review screen lists only launch layers. Trust exactly these
/// declarations for this thread; host entries and persisted decisions stay as found.
/// Hash protocol: codex-rs/hooks/src/engine/discovery.rs (0.164.0).
pub(crate) fn codex_config(
    context: &ContextDelivery,
    session_hooks: Option<&Value>,
) -> Result<Value> {
    // Thread configuration replaces the app-server's session-flags layer. Keep
    // wrapper declarations at their original indices so their trust IDs survive.
    let mut hooks = session_hooks.cloned().unwrap_or_else(|| json!({}));
    let additions = context.hook_settings()?;
    let mut states = hooks
        .get("state")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (event, event_name) in [
        ("SessionStart", "session_start"),
        ("Stop", "stop"),
        ("PostToolUse", "post_tool_use"),
        ("UserPromptSubmit", "user_prompt_submit"),
    ] {
        let Some(groups) = additions[event].as_array() else {
            continue;
        };
        let mut combined = hooks
            .get(event)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for group in groups {
            let mut handler = group["hooks"][0].clone();
            handler["async"] = json!(false);
            let mut identity = json!({"event_name":event_name, "hooks":[handler]});
            if let Some(matcher) = group.get("matcher") {
                identity["matcher"] = matcher.clone();
            }
            identity.sort_all_objects();
            let hash = hex::encode(Sha256::digest(serde_json::to_vec(&identity)?));
            states.insert(
                format!(
                    "/<session-flags>/config.toml:{event_name}:{}:0",
                    combined.len()
                ),
                json!({"trusted_hash":format!("sha256:{hash}")}),
            );
            combined.push(group.clone());
        }
        hooks[event] = json!(combined);
    }
    hooks["state"] = Value::Object(states);
    Ok(json!({"hooks":hooks}))
}

pub(crate) fn add_opencode_context(
    config: &mut Value,
    context: &ContextDelivery,
    instructions: &str,
) -> Result<()> {
    let spec = json!({"executable":crate::os_process::resolve_lf_binary()?, "delivery":context, "instructions":instructions});
    let delivery = write_prompt_log(
        &context.repo,
        &serde_json::to_string(context)?,
        "context-delivery",
        None,
    )?;
    let source = format!(
        "const spec = {};\nconst delivery = {};\n{}",
        spec,
        serde_json::to_string(&delivery)?,
        include_str!("context_opencode.js")
    );
    let path = write_prompt_log(&context.repo, &source, "opencode-plugin", None)?;
    let plugin = path.with_extension("plugin");
    std::fs::create_dir(&plugin)?;
    std::fs::rename(path, plugin.join("index.js"))?;
    std::fs::write(
        plugin.join("package.json"),
        r#"{"name":"loopflow-context","type":"module","exports":{"./server":"./index.js"}}"#,
    )?;
    config
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("OpenCode configuration must be an object"))?
        .entry("plugins")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("OpenCode plugin config must be an array"))?
        .push(json!(format!("file://{}", plugin.display())));
    Ok(())
}

pub(crate) fn native_args(harness: &str, context: Option<&ContextDelivery>) -> Result<Vec<String>> {
    match (harness, context) {
        ("claude", Some(context)) => claude_args(context),
        ("pi", context) => pi_args(context, true),
        _ => Ok(Vec::new()),
    }
}

pub(crate) fn pi_args(context: Option<&ContextDelivery>, native: bool) -> Result<Vec<String>> {
    let source = format!(
        "const spec = {};\n{}",
        json!({"native":native, "executable":crate::os_process::resolve_lf_binary()?}),
        include_str!("context_pi.mjs")
    );
    let home = context
        .map(|c| c.home.clone())
        .unwrap_or_else(crate::store::lf_home_dir);
    let extension = save_hook(&home, "pi", "mjs", source.as_bytes())?;
    Ok(vec!["--extension".into(), extension.display().to_string()])
}

fn save_hook(home: &Path, provider: &str, extension: &str, bytes: &[u8]) -> Result<PathBuf> {
    let directory = home.join("hook-settings");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!(
        "{provider}-{}.{extension}",
        hex::encode(Sha256::digest(bytes))
    ));
    if !path.exists() {
        let file = tempfile::NamedTempFile::new_in(&directory)?;
        std::fs::write(file.path(), bytes)?;
        file.persist(&path)?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::add_opencode_context;
    use crate::context_block::ContextDelivery;

    #[test]
    fn hook_identity_is_shared_across_tasks_and_sessions() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let repos = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
        let contexts: Vec<_> = repos
            .iter()
            .enumerate()
            .map(|(index, repo)| ContextDelivery {
                repository: None,
                repo: repo.path().to_owned(),
                home: home.path().to_owned(),
                wave: Some(format!("wave-{index}")),
                skill_file: Some(repo.path().join("saved-skill.md")),
                submitted_docs: None,
                references: vec![repo.path().join("task-reference.md")],
            })
            .collect();
        let first = super::codex_config(&contexts[0], None).unwrap();
        assert_eq!(first, super::codex_config(&contexts[1], None).unwrap());
        assert_eq!(
            super::claude_args(&contexts[0]).unwrap(),
            super::claude_args(&contexts[1]).unwrap()
        );
        let mut saved = Vec::new();
        for context in &contexts {
            let mut command = std::process::Command::new("provider");
            context.configure_hooks(&mut command).unwrap();
            let path = command
                .get_envs()
                .find(|(key, _)| *key == "LF_CONTEXT_DELIVERY")
                .unwrap()
                .1
                .unwrap();
            let restored: ContextDelivery =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            assert_eq!(&restored, context);
            saved.push(path.to_owned());
        }
        assert_ne!(saved[0], saved[1]);
        // Later launches never overwrite a retained native resume's sources.
        let restored: ContextDelivery =
            serde_json::from_slice(&std::fs::read(&saved[0]).unwrap()).unwrap();
        assert_eq!(restored, contexts[0]);
    }

    #[test]
    fn codex_trusts_only_loopflow_declarations_after_host_hooks() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let context = ContextDelivery {
            repository: None,
            repo: home.path().to_owned(),
            home: home.path().to_owned(),
            wave: None,
            skill_file: None,
            submitted_docs: None,
            references: Vec::new(),
        };
        let host = serde_json::json!({
            "SessionStart": [{"hooks": [{"type": "command", "command": "host-hook"}]}],
            "state": {
                "host-trust": {"trusted_hash": "unchanged"},
                "/<session-flags>/config.toml:session_start:0:0": {"trusted_hash": "host"},
            },
        });
        let hooks = &super::codex_config(&context, Some(&host)).unwrap()["hooks"];
        assert_eq!(hooks["SessionStart"][0]["hooks"][0]["command"], "host-hook");
        let state = hooks["state"].as_object().unwrap();
        assert_eq!(state["host-trust"]["trusted_hash"], "unchanged");
        assert_eq!(
            state["/<session-flags>/config.toml:session_start:0:0"]["trusted_hash"],
            "host"
        );
        let mut trusted: Vec<_> = state
            .keys()
            .filter(|key| *key != "host-trust")
            .map(String::as_str)
            .collect();
        trusted.sort_unstable();
        assert_eq!(
            trusted,
            [
                "/<session-flags>/config.toml:post_tool_use:0:0",
                "/<session-flags>/config.toml:session_start:0:0",
                "/<session-flags>/config.toml:session_start:1:0",
                "/<session-flags>/config.toml:session_start:2:0",
                "/<session-flags>/config.toml:stop:0:0",
                "/<session-flags>/config.toml:user_prompt_submit:0:0",
            ]
        );
        // Each Loopflow entry trusts the exact declaration at its own index.
        assert_ne!(
            state["/<session-flags>/config.toml:session_start:1:0"],
            state["/<session-flags>/config.toml:session_start:2:0"]
        );
    }

    #[test]
    fn pi_extension_names_lf_without_an_inherited_lf_bin() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let executable = std::env::current_exe().unwrap();
        std::env::set_var("LF_BIN", &executable);

        let args = super::pi_args(None, true).unwrap();

        // A fresh machine's Pi process inherits no LF_BIN; the source carries it.
        let source = std::fs::read_to_string(&args[1]).unwrap();
        assert!(source.contains(&serde_json::to_string(&executable).unwrap()));
        assert!(!source.contains("process.env.LF_BIN"));
    }

    #[test]
    fn opencode_context_preserves_settings_and_existing_plugins() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let repo = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", repo.path().join("machine"));
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let context = ContextDelivery {
            repository: None,
            repo: repo.path().to_owned(),
            home: repo.path().join("machine"),
            wave: None,
            skill_file: None,
            submitted_docs: None,
            references: Vec::new(),
        };
        let original = serde_json::json!({
            "plugins": ["existing-plugin"],
            "permissions": [{"action":"edit","resource":"*","effect":"deny"}],
            "model": "fixture/model"
        });
        let mut config = original.clone();
        add_opencode_context(&mut config, &context, "Fixed instructions").unwrap();
        assert_eq!(config["permissions"], original["permissions"]);
        assert_eq!(config["model"], original["model"]);
        let plugins = config["plugins"].as_array().unwrap();
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0], "existing-plugin");
        let path = plugins[1]
            .as_str()
            .unwrap()
            .strip_prefix("file://")
            .unwrap();
        assert!(
            std::fs::read_to_string(std::path::Path::new(path).join("index.js"))
                .unwrap()
                .contains("Fixed instructions")
        );
    }
}
