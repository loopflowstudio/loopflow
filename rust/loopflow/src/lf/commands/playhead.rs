use std::path::Path;

use anyhow::{anyhow, Result};

use crate::ops::util::resolve_wave_name;
use crate::wave::playhead::PlayheadView;
use crate::wave::server;

pub fn enqueue(repo: &Path, wave: Option<&str>, flow: &str) -> Result<()> {
    post(
        repo,
        wave,
        "enqueue",
        Some(serde_json::json!({ "flow": flow })),
    )
}

pub fn skip(repo: &Path, wave: Option<&str>) -> Result<()> {
    post(repo, wave, "skip", None)
}

/// POST one playhead verb to the running wave and print the returned cursor.
fn post(
    repo: &Path,
    wave: Option<&str>,
    verb: &str,
    body: Option<serde_json::Value>,
) -> Result<()> {
    let endpoint = endpoint(repo, wave)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let view = runtime.block_on(async {
        let mut request = reqwest::Client::new().post(format!("http://{endpoint}/playhead/{verb}"));
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await?.error_for_status()?;
        Ok::<PlayheadView, anyhow::Error>(response.json().await?)
    })?;
    if let Some(now) = &view.now {
        println!("now  {} / {}", now.flow, now.step);
    }
    if let Some(next) = &view.next {
        println!("next {} / {}", next.flow, next.step);
    }
    Ok(())
}

fn endpoint(repo: &Path, wave: Option<&str>) -> Result<String> {
    let wave = resolve_wave_name(repo, wave)
        .ok_or_else(|| anyhow!("cannot determine wave; pass --wave <name>"))?;
    let origin = crate::engine::wave_context::wave_origin(repo);
    std::fs::read_to_string(server::endpoint_path(&origin, &wave))
        .map(|value| value.trim().to_string())
        .map_err(|_| anyhow!("wave '{wave}' is not running; start it with `lf loop {wave}`"))
}
