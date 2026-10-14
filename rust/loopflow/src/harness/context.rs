//! Provider-owned conversation hooks. Settings are launch-scoped; no user files
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
    let path = write_prompt_log(
        &context.repo,
        &serde_json::to_string(&settings)?,
        "claude-hooks",
        None,
    )?;
    Ok(vec!["--settings".into(), path.display().to_string()])
}

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
    Ok(json!({"hooks":hooks}))
}

pub(crate) fn add_opencode_context(
    config: &mut Value,
    context: &ContextDelivery,
    instructions: &str,
) -> Result<()> {
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
    config
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("OpenCode configuration must be an object"))?
        .entry("plugin")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("OpenCode plugin config must be an array"))?
        .push(json!(format!("file://{}", plugin.display())));
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
    use super::add_opencode_context;
    use crate::context_block::ContextDelivery;

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
        let mut config = original.clone();
        add_opencode_context(&mut config, &context, "Fixed instructions").unwrap();
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
