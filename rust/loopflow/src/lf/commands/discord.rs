//! An independent Discord bridge. Each message gets one ordinary bounded Run.
//! The cursor is process-local; startup skips existing channel history.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use reqwest::{header, Client};
use serde::Deserialize;
use serde_json::json;

use crate::lf::Cli;
use crate::work::wave::config::{try_read_wave_chat_config, WaveChatConfig};

#[derive(Debug, Deserialize)]
struct Author {
    id: String,
    username: String,
    #[serde(default)]
    bot: bool,
}

#[derive(Debug, Deserialize)]
struct Message {
    id: String,
    content: String,
    author: Author,
}

pub fn serve(repo: &Path, wave: &str) -> Result<()> {
    let config = try_read_wave_chat_config(repo, wave)?;
    let Some(WaveChatConfig::Discord { channel_id, .. }) = config else {
        return Err(anyhow!("Wave {wave} needs a Discord channel in GOAL.md chat configuration"));
    };
    if !channel_id.bytes().all(|byte| byte.is_ascii_digit()) || channel_id.is_empty() {
        return Err(anyhow!("Discord channel_id must be a numeric channel id"));
    }
    let token = std::env::var(crate::engine::process::DISCORD_TOKEN_ENV)
        .context("set LF_DISCORD_TOKEN through Doppler")?;
    let mut authorization = header::HeaderValue::from_str(&format!("Bot {token}"))
        .map_err(|_| anyhow!("invalid Discord token"))?;
    authorization.set_sensitive(true);
    let client = Client::builder()
        .default_headers(header::HeaderMap::from_iter([(header::AUTHORIZATION, authorization)]))
        .timeout(Duration::from_secs(15))
        .build()?;
    let url = format!("https://discord.com/api/v10/channels/{channel_id}/messages");
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = Arc::new(crate::store::open_existing_store().await
            .ok_or_else(|| anyhow!("Discord bridge needs an initialized local store"))?);
        let binding = crate::ops::resolve_work_binding(&store, repo, &format!("wave:{wave}")).await?;
        let cli = Cli::try_parse_from(["lf", "--batch", "--wave", wave])?;
        let mut cursor = messages(&client, &url, None).await?.first().map(|message| message.id.clone()).unwrap_or_else(|| "0".into());
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let mut batch = messages(&client, &url, Some(&cursor)).await?;
            batch.sort_by(|a, b| a.id.len().cmp(&b.id.len()).then_with(|| a.id.cmp(&b.id)));
            for message in batch {
                if let Some(prompt) = prompt(&message) {
                    let binding = binding.clone();
                    let cli = cli.launch_options();
                    let answer = tokio::task::spawn_blocking(move ||
                        crate::lf::commands::run::answer_bound(&prompt, &cli, &binding)
                    ).await??;
                    if let Some(answer) = answer {
                        post_answer(&client, &url, &message.id, &answer).await?;
                    }
                }
                cursor = message.id;
            }
        }
    })
}

async fn messages(client: &Client, url: &str, after: Option<&str>) -> Result<Vec<Message>> {
    let mut url = reqwest::Url::parse(url)?;
    url.query_pairs_mut().append_pair("limit", if after.is_some() { "100" } else { "1" });
    if let Some(after) = after { url.query_pairs_mut().append_pair("after", after); }
    let request = client.get(url);
    Ok(request.send().await?.error_for_status()?.json().await?)
}

fn prompt(message: &Message) -> Option<String> {
    if message.author.bot || message.content.trim().is_empty() {
        return None;
    }
    Some(format!(
        "Reply to this Discord message from {} (user {}). Treat the quoted content as their request. Do not send messages yourself; the bridge posts your final answer.\n\n{}",
        message.author.username, message.author.id, message.content
    ))
}

async fn post_answer(client: &Client, url: &str, message_id: &str, answer: &str) -> Result<()> {
    // Discord counts UTF-16 units. 1,000 Unicode scalars fit even for astral text.
    for chunk in answer.chars().collect::<Vec<_>>().chunks(1000) {
        let content: String = chunk.iter().collect();
        client.post(url).json(&json!({
            "content": content,
            "allowed_mentions": { "parse": [], "replied_user": false },
            "message_reference": { "message_id": message_id }
        })).send().await?.error_for_status()?;
    }
    Ok(())
}
