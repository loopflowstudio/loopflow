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

/// Stable declarations only. The provider owns review and persisted trust.
pub(crate) fn codex_config(
    context: &ContextDelivery,
    session_hooks: Option<&Value>,
) -> Result<Value> {
    // Thread configuration replaces the app-server's session-flags layer. Keep
    // wrapper declarations at their original indices and preserve existing trust.
    let mut hooks = session_hooks.cloned().unwrap_or_else(|| json!({}));
    let additions = context.hook_settings()?;
    for event in ["SessionStart", "Stop", "PostToolUse", "UserPromptSubmit"] {
        let Some(groups) = additions[event].as_array() else {
            continue;
        };
        let mut combined = hooks
            .get(event)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        combined.extend(groups.iter().cloned());
        hooks[event] = json!(combined);
    }
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
        let first = super::codex_config(&contexts[0], None).unwrap();
        assert_eq!(first, super::codex_config(&contexts[1], None).unwrap());
        assert!(first["hooks"]["state"].is_null());
        assert!(!first.to_string().contains("trusted_hash"));
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
