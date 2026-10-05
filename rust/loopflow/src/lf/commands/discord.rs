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
        return Err(anyhow!(
            "Wave {wave} needs a Discord channel in GOAL.md chat configuration"
        ));
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
        .default_headers(header::HeaderMap::from_iter([(
            header::AUTHORIZATION,
            authorization,
        )]))
        .timeout(Duration::from_secs(15))
        .build()?;
    let url = format!("https://discord.com/api/v10/channels/{channel_id}/messages");
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = Arc::new(
            crate::store::open_existing_store()
                .await
                .ok_or_else(|| anyhow!("Discord bridge needs an initialized local store"))?,
        );
        let binding =
            crate::ops::resolve_work_binding(&store, repo, &format!("wave:{wave}")).await?;
        let cli = Cli::try_parse_from(["lf", "--batch", "--wave", wave])?;
        let mut cursor = messages(&client, &url, None)
            .await?
            .first()
            .map(|message| message.id.clone())
            .unwrap_or_else(|| "0".into());
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let mut batch = messages(&client, &url, Some(&cursor)).await?;
            batch.sort_by(|a, b| a.id.len().cmp(&b.id.len()).then_with(|| a.id.cmp(&b.id)));
            for message in batch {
                if let Some(prompt) = prompt(&message) {
                    let binding = binding.clone();
                    let cli = cli.exec_options();
                    let answer = tokio::task::spawn_blocking(move || {
                        crate::lf::commands::run::answer_bound(&prompt, &cli, &binding)
                    })
                    .await??;
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
    url.query_pairs_mut()
        .append_pair("limit", if after.is_some() { "100" } else { "1" });
    if let Some(after) = after {
        url.query_pairs_mut().append_pair("after", after);
    }
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
        client
            .post(url)
            .json(&json!({
                "content": content,
                "allowed_mentions": { "parse": [], "replied_user": false },
                "message_reference": { "message_id": message_id }
            }))
            .send()
            .await?
            .error_for_status()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::{extract::Query, routing::get, Json, Router};
    use reqwest::Client;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use tokio::sync::mpsc;

    use super::{messages, post_answer, prompt};

    #[tokio::test]
    async fn bridge_reads_new_messages_and_replies_without_mentions() {
        let (sent, mut replies) = mpsc::unbounded_channel::<Value>();
        let app = Router::new().route("/messages", get(|Query(query): Query<HashMap<String, String>>| async move {
            let id = if query.get("after").map(String::as_str) == Some("41") { "42" } else { "41" };
            Json(json!([
                {"id":id,"content":"Review the plan","author":{"id":"user","username":"Maya","bot":false}},
                {"id":"43","content":"bot reply","author":{"id":"bot","username":"Bot","bot":true}}
            ]))
        }).post(move |Json(body): Json<Value>| {
            let sent = sent.clone();
            async move { sent.send(body).unwrap(); Json(json!({"id":"44"})) }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/messages", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = Client::new();
        let initial = messages(&client, &url, None).await.unwrap();
        let batch = messages(&client, &url, Some(&initial[0].id)).await.unwrap();
        assert_eq!(batch[0].id, "42");
        let request = prompt(&batch[0]).unwrap();
        assert!(request.contains("Maya (user user)"));
        assert!(request.ends_with("Review the plan"));
        assert!(prompt(&batch[1]).is_none());
        let answer = "🦀".repeat(1001);
        post_answer(&client, &url, &batch[0].id, &answer)
            .await
            .unwrap();
        let mut joined = String::new();
        for _ in 0..2 {
            let reply = replies.try_recv().unwrap();
            let content = reply["content"].as_str().unwrap();
            assert!(content.encode_utf16().count() <= 2000);
            assert_eq!(
                reply["allowed_mentions"],
                json!({"parse":[],"replied_user":false})
            );
            assert_eq!(reply["message_reference"]["message_id"], "42");
            joined.push_str(content);
        }
        assert_eq!(joined, answer);
        server.abort();
    }
}
