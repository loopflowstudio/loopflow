//! Stable provider hook declarations and separately saved launch context. No user files
//! or trust decisions are replaced. Saved sources survive driver replacement.
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
    let directory = context.home.join("hook-settings");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!(
        "claude-{}.json",
        hex::encode(Sha256::digest(&bytes))
    ));
    if !path.exists() {
        let file = tempfile::NamedTempFile::new_in(&directory)?;
        serde_json::to_writer(file.as_file(), &settings)?;
        file.persist(&path)?;
    }
    Ok(vec!["--settings".into(), path.display().to_string()])
}

<<<<<<< HEAD
/// Stable declarations only. The provider owns review and persisted trust.
pub(crate) fn codex_config(context: Option<&ContextDelivery>, capture: bool) -> Result<Value> {
    let mut hooks = match context {
        Some(context) => context.hook_settings()?,
        None => json!({"SessionStart": []}),
    };
    if capture {
        let executable = crate::os_process::resolve_lf_binary();
        let command = format!(
            "{} __provider-session",
            crate::os_process::shell_escape(&executable.to_string_lossy())
        );
        hooks["SessionStart"]
            .as_array_mut()
            .expect("hook groups")
            .push(json!({
                "matcher": "startup", "hooks": [{"type":"command", "command":command, "timeout":5}]
            }));
    }
||||||| e3c3a038aa
/// Codex hashes normalized hook declarations as sorted JSON. Trust only these
/// exact commands in the session-flags layer, never all hooks on the machine.
/// Protocol: codex-rs/hooks/src/engine/discovery.rs (0.161.0).
pub(crate) fn codex_config(context: Option<&ContextDelivery>, capture: bool) -> Result<Value> {
    let mut hooks = match context {
        Some(context) => context.hook_settings()?,
        None => json!({"SessionStart": []}),
    };
    if capture {
        let executable = crate::os_process::resolve_lf_binary();
        let command = format!(
            "{} __provider-session",
            crate::os_process::shell_escape(&executable.to_string_lossy())
        );
        hooks["SessionStart"]
            .as_array_mut()
            .expect("hook groups")
            .push(json!({
                "matcher": "startup", "hooks": [{"type":"command", "command":command, "timeout":5}]
            }));
    }
    let mut states = serde_json::Map::new();
    for (event, event_name) in [
        ("SessionStart", "session_start"),
        ("Stop", "stop"),
        ("PostToolUse", "post_tool_use"),
        ("UserPromptSubmit", "user_prompt_submit"),
    ] {
        let Some(groups) = hooks[event].as_array() else {
            continue;
        };
        for (index, group) in groups.iter().enumerate() {
            let mut handler = group["hooks"][0].clone();
            handler["async"] = json!(false);
            let mut identity =
                json!({"event_name":event_name, "matcher":group["matcher"], "hooks":[handler]});
            identity.sort_all_objects();
            let hash = format!(
                "sha256:{}",
                hex::encode(Sha256::digest(serde_json::to_vec(&identity)?))
            );
            states.insert(
                format!("/<session-flags>/config.toml:{event_name}:{index}:0"),
                json!({"trusted_hash":hash}),
            );
        }
    }
    hooks["state"] = Value::Object(states);
=======
/// Codex hashes normalized hook declarations as sorted JSON. Trust only these
/// exact commands in the session-flags layer, never all hooks on the machine.
/// Protocol: codex-rs/hooks/src/engine/discovery.rs (0.162.1).
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
        let offset = combined.len();
        for (index, group) in groups.iter().enumerate() {
            let index = offset + index;
            let mut handler = group["hooks"][0].clone();
            handler["async"] = json!(false);
            let mut identity = json!({"event_name":event_name, "hooks":[handler]});
            if let Some(matcher) = group.get("matcher") {
                identity["matcher"] = matcher.clone();
            }
            identity.sort_all_objects();
            let hash = format!(
                "sha256:{}",
                hex::encode(Sha256::digest(serde_json::to_vec(&identity)?))
            );
            states.insert(
                format!("/<session-flags>/config.toml:{event_name}:{index}:0"),
                json!({"trusted_hash":hash}),
            );
        }
        combined.extend(groups.iter().cloned());
        hooks[event] = json!(combined);
    }
    hooks["state"] = Value::Object(states);
>>>>>>> f9a16d6f84fde87b6650bab34af375bfb4981508
    Ok(json!({"hooks":hooks}))
}

pub(crate) fn opencode_config(
    context: &ContextDelivery,
    instructions: &str,
    existing: Option<&str>,
) -> Result<String> {
    let spec = json!({"executable":crate::os_process::resolve_lf_binary(), "delivery":context, "instructions":instructions});
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
    let plugin = path.with_extension("mjs");
    std::fs::rename(path, &plugin)?;
    let inherited = std::env::var("OPENCODE_CONFIG_CONTENT").ok();
    let mut config: Value = existing
        .or(inherited.as_deref())
        .map(serde_json::from_str)
        .transpose()?
        .unwrap_or_else(|| json!({}));
    config
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("OpenCode configuration must be an object"))?
        .entry("plugin")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("OpenCode plugin config must be an array"))?
        .push(json!(format!("file://{}", plugin.display())));
    Ok(serde_json::to_string(&config)?)
}

pub(crate) fn configure_opencode(
    command: &mut std::process::Command,
    context: &ContextDelivery,
    instructions: &str,
) -> Result<()> {
    let existing = command
        .get_envs()
        .find(|(key, _)| *key == "OPENCODE_CONFIG_CONTENT")
        .and_then(|(_, value)| value.map(|v| v.to_string_lossy().into_owned()));
    let config = opencode_config(context, instructions, existing.as_deref())?;
    command.env("OPENCODE_CONFIG_CONTENT", config);
    Ok(())
}

pub(crate) fn native_args(harness: &str, context: Option<&ContextDelivery>) -> Result<Vec<String>> {
    match (harness, context) {
        ("claude", Some(context)) => claude_args(context),
        _ => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::opencode_config;
    use crate::context_block::ContextDelivery;

    #[test]
    fn hook_identity_is_shared_across_tasks_and_sessions_without_granting_trust() {
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_BIN"]);
        let home = tempfile::tempdir().unwrap();
        let repos = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
        let contexts: Vec<_> = repos
            .iter()
            .enumerate()
            .map(|(index, repo)| ContextDelivery {
                repo: repo.path().to_owned(),
                home: home.path().to_owned(),
                wave: Some(format!("wave-{index}")),
                skill_file: Some(repo.path().join("saved-skill.md")),
                references: vec![repo.path().join("task-reference.md")],
            })
            .collect();
        for capture in [true, false] {
            let first = super::codex_config(Some(&contexts[0]), capture).unwrap();
            assert_eq!(
                first,
                super::codex_config(Some(&contexts[1]), capture).unwrap()
            );
            assert!(first["hooks"]["state"].is_null());
            assert!(!first.to_string().contains("trusted_hash"));
        }
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
    fn opencode_context_preserves_settings_and_existing_plugins() {
        let repo = tempfile::tempdir().unwrap();
        let context = ContextDelivery {
            repo: repo.path().to_owned(),
            home: repo.path().join("machine"),
            wave: None,
            skill_file: None,
            references: Vec::new(),
        };
        let original = serde_json::json!({
            "plugin": ["existing-plugin"],
            "permission": {"edit": "deny"},
            "model": "fixture/model"
        });
        let config: serde_json::Value = serde_json::from_str(
            &opencode_config(&context, "Fixed instructions", Some(&original.to_string())).unwrap(),
        )
        .unwrap();
        assert_eq!(config["permission"], original["permission"]);
        assert_eq!(config["model"], original["model"]);
        let plugins = config["plugin"].as_array().unwrap();
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0], "existing-plugin");
        let path = plugins[1]
            .as_str()
            .unwrap()
            .strip_prefix("file://")
            .unwrap();
        assert!(std::fs::read_to_string(path)
            .unwrap()
            .contains("Fixed instructions"));
    }
}
