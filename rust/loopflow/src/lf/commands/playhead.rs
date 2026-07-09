use std::path::Path;

use anyhow::{anyhow, Result};

use crate::ops::util::resolve_wave_name;
use crate::wave::playhead::PlayheadView;
use crate::wave::server;

pub fn enqueue(repo: &Path, wave: Option<&str>, flow: &str) -> Result<()> {
    let endpoint = endpoint(repo, wave)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let view = runtime.block_on(async {
        let response = reqwest::Client::new()
            .post(format!("http://{endpoint}/playhead/enqueue"))
            .json(&serde_json::json!({ "flow": flow }))
            .send()
            .await?
            .error_for_status()?;
        Ok::<PlayheadView, anyhow::Error>(response.json().await?)
    })?;
    print_view(&view);
    Ok(())
}

pub fn skip(repo: &Path, wave: Option<&str>) -> Result<()> {
    let endpoint = endpoint(repo, wave)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let view = runtime.block_on(async {
        let response = reqwest::Client::new()
            .post(format!("http://{endpoint}/playhead/skip"))
            .send()
            .await?
            .error_for_status()?;
        Ok::<PlayheadView, anyhow::Error>(response.json().await?)
    })?;
    print_view(&view);
    Ok(())
}

fn endpoint(repo: &Path, wave: Option<&str>) -> Result<String> {
    let wave = resolve_wave_name(repo, wave)
        .ok_or_else(|| anyhow!("cannot determine wave; pass --wave <name>"))?;
    std::fs::read_to_string(server::endpoint_path(repo, &wave))
        .map(|value| value.trim().to_string())
        .map_err(|_| anyhow!("wave '{wave}' is not running; start it with `lf loop {wave}`"))
}

fn print_view(view: &PlayheadView) {
    if let Some(now) = &view.now {
        println!("now  {} / {}", now.flow, now.step);
    }
    if let Some(next) = &view.next {
        println!("next {} / {}", next.flow, next.step);
    }
}
