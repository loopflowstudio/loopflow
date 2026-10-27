//! OpenCode keeps history in its own store; `opencode session export` writes
//! one conversation as JSON. This is the only reader that runs a subprocess.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use super::{text_of, Launch, RequestSource, Turn, Turns};

pub(super) fn export(cwd: &Path, id: &str) -> Result<Value> {
    let output = Command::new("opencode")
        .args(["session", "export", id, "--standalone"])
        .current_dir(cwd)
        .output()
        .context("run opencode session export (is opencode installed?)")?;
    if !output.status.success() {
        bail!(
            "opencode session export {id} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).context("parse opencode session export")
}

/// Turns of an export: messages, each with text and tool parts.
pub(crate) fn turns(export: &Value) -> Vec<Turn> {
    let mut turns = Turns::default();
    for message in export["messages"].as_array().into_iter().flatten() {
        let id = message["id"].as_str().unwrap_or_default();
        if message["type"] == "user" {
            let text = message["text"].as_str().unwrap_or_default();
            let at = message["time"]["created"].as_i64().map(|ms| ms / 1000);
            if Launch::parse(text).is_some() {
                turns.request(at, text.to_owned(), RequestSource::Injected);
            } else {
                let typed = text.split("<lf:").next().unwrap_or_default();
                if !typed.trim().is_empty() {
                    turns.request(at, typed.to_owned(), RequestSource::Person);
                }
            }
        }
        if message["type"] != "assistant" {
            continue;
        }
        for part in message["content"].as_array().into_iter().flatten() {
            match part["type"].as_str() {
                Some("text") => turns.message(id, part["text"].as_str().unwrap_or("")),
                Some("reasoning") => turns.thought(id, part["text"].as_str().unwrap_or("")),
                Some("tool") => {
                    let state = &part["state"];
                    let call = part["id"].as_str().unwrap_or_default();
                    turns.tool(Some(call), part["name"].as_str(), state["input"].clone());
                    match state["status"].as_str() {
                        Some("error") => turns.resolve(call, true, text_of(&state["error"])),
                        Some("completed") => {
                            let output = match state["output"].as_str() {
                                Some(output) => output.to_owned(),
                                None => text_of(&state["content"]),
                            };
                            turns.resolve(call, false, output);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
    turns.0
}
