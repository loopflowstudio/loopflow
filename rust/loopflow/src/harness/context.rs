//! Provider-owned conversation hooks. Settings are launch-scoped; no user files
//! or trust decisions are replaced. Saved sources survive driver replacement.
use std::path::Path;

use anyhow::Result;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::engine::context_block::ContextDelivery;
use crate::engine::prompt::write_prompt_log;

pub(crate) fn claude_args(context: &ContextDelivery) -> Result<Vec<String>> {
    let settings = json!({"hooks": context.hook_settings()?});
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
/// Protocol: codex-rs/hooks/src/engine/discovery.rs (0.161.0).
pub(crate) fn codex_config(context: Option<&ContextDelivery>, capture: bool) -> Result<Value> {
    let mut hooks = match context {
        Some(context) => context.hook_settings()?,
        None => json!({"SessionStart": []}),
    };
    if capture {
        let executable = crate::engine::process::resolve_lf_binary();
        let command = format!(
            "{} __provider-session",
            crate::engine::process::shell_escape(&executable.to_string_lossy())
        );
        hooks["SessionStart"]
            .as_array_mut()
            .expect("hook groups")
            .push(json!({
                "matcher": "startup", "hooks": [{"type":"command", "command":command, "timeout":5}]
            }));
    }
    let mut states = serde_json::Map::new();
    for (index, group) in hooks["SessionStart"]
        .as_array()
        .expect("hook groups")
        .iter()
        .enumerate()
    {
        let mut handler = group["hooks"][0].clone();
        handler["async"] = json!(false);
        let mut identity =
            json!({"event_name":"session_start", "matcher":group["matcher"], "hooks":[handler]});
        identity.sort_all_objects();
        let hash = format!(
            "sha256:{}",
            hex::encode(Sha256::digest(serde_json::to_vec(&identity)?))
        );
        states.insert(
            format!("/<session-flags>/config.toml:session_start:{index}:0"),
            json!({"trusted_hash":hash}),
        );
    }
    hooks["state"] = Value::Object(states);
    Ok(json!({"hooks":hooks}))
}

pub(crate) fn codex_args(context: Option<&ContextDelivery>, capture: bool) -> Result<Vec<String>> {
    let config = codex_config(context, capture)?;
    let hooks = inline_toml(&config["hooks"]);
    Ok(vec!["-c".into(), format!("hooks={hooks}")])
}

// Only the non-null JSON values in our owned hook declarations enter this encoder.
fn inline_toml(value: &Value) -> String {
    match value {
        Value::Object(fields) => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|(key, value)| format!(
                    "{} = {}",
                    serde_json::to_string(key).expect("key serializes"),
                    inline_toml(value)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(inline_toml)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Null => unreachable!("hook declarations contain no null values"),
        value => value.to_string(),
    }
}

pub(crate) fn opencode_config(context: &ContextDelivery, instructions: &str) -> Result<Value> {
    let spec = json!({"executable":crate::engine::process::resolve_lf_binary(), "delivery":context, "instructions":instructions});
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
    Ok(json!({"plugin": [format!("file://{}", plugin.display())]}))
}

pub(crate) fn merge_opencode_config(
    command: &mut std::process::Command,
    config: Value,
) -> Result<()> {
    let existing = command
        .get_envs()
        .find(|(key, _)| *key == "OPENCODE_CONFIG_CONTENT")
        .and_then(|(_, value)| value.map(|v| v.to_string_lossy().into_owned()))
        .or_else(|| std::env::var("OPENCODE_CONFIG_CONTENT").ok());
    let mut merged: Value = existing
        .map(|text| serde_json::from_str(&text))
        .transpose()?
        .unwrap_or_else(|| json!({}));
    for (key, value) in config.as_object().expect("plugin config object") {
        if key == "plugin" {
            merged
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("OpenCode configuration must be an object"))?
                .entry(key.clone())
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .ok_or_else(|| anyhow::anyhow!("OpenCode plugin config must be an array"))?
                .extend(value.as_array().expect("plugin list").iter().cloned());
        } else {
            merged[key] = value.clone();
        }
    }
    command.env("OPENCODE_CONFIG_CONTENT", serde_json::to_string(&merged)?);
    Ok(())
}

pub(crate) fn native_args(
    harness: &str,
    context: Option<&ContextDelivery>,
    capture: bool,
) -> Result<Vec<String>> {
    match (harness, context) {
        ("claude", Some(context)) => claude_args(context),
        ("codex", _) if context.is_some() || capture => codex_args(context, capture),
        _ => Ok(Vec::new()),
    }
}

pub(crate) fn read_instructions(path: Option<&Path>) -> Result<String> {
    Ok(path
        .map(std::fs::read_to_string)
        .transpose()?
        .unwrap_or_default())
}
