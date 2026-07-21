//! One Discord guild text channel as a presentation surface for Wave chat.
//!
//! Discord owns the company transcript. This adapter polls only messages after
//! the journal's committed cursor and writes source-linked inputs plus outbound
//! delivery receipts through [`WaveRuntime`]. It never owns an inbox or writes
//! the journal directly.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use fs2::FileExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use reqwest::{Client, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::durable::HomeId;
use crate::wave::journal::{DiscordChatBinding, DiscordMessageSource};
use crate::wave::runtime::WaveRuntime;

pub const TOKEN_ENV: &str = "LF_DISCORD_TOKEN";
const API_BASE: &str = "https://discord.com/api/v10";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_CADENCE: Duration = Duration::from_secs(2);
const ERROR_BACKOFF: Duration = Duration::from_secs(10);
const VIEW_CHANNEL: u64 = 1 << 10;
const SEND_MESSAGES: u64 = 1 << 11;
const READ_MESSAGE_HISTORY: u64 = 1 << 16;
const ADMINISTRATOR: u64 = 1 << 3;
const MESSAGE_CONTENT: u64 = 1 << 18;
const MESSAGE_CONTENT_LIMITED: u64 = 1 << 19;

#[derive(Debug, thiserror::Error)]
pub enum DiscordError {
    #[error("{TOKEN_ENV} is required for a configured Discord chat binding")]
    MissingToken,
    #[error("Discord request failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("Discord API returned {status}: {message}")]
    Api { status: StatusCode, message: String },
    #[error("Discord binding is invalid: {0}")]
    Binding(String),
    #[error("Discord Message Content intent is not enabled for this application")]
    MissingMessageContent,
    #[error("Discord channel is missing required permissions: {0}")]
    MissingPermissions(String),
    #[error("Discord returned an invalid permission value: {0}")]
    InvalidPermission(String),
    #[error("Discord chat binding is owned by Home {owner}; current Home is {current}")]
    WrongHome { owner: String, current: String },
    #[error(
        "Discord chat binding {guild_id}/{channel_id} already has a live listener on Home {home_id}"
    )]
    AlreadyOwned {
        guild_id: String,
        channel_id: String,
        home_id: String,
    },
    #[error("failed to claim Discord chat binding: {0}")]
    Lease(#[from] std::io::Error),
}

impl DiscordError {
    fn retryable(&self) -> bool {
        matches!(self, Self::Transport(_))
            || matches!(self, Self::Api { status, .. } if status.is_server_error())
    }
}

/// Process-held ownership for one Discord binding on its configured Home.
///
/// The configured Home prevents a second Home from competing. The advisory
/// lock prevents a second checkout or store on that Home from competing. The
/// file may outlive the process; only the held OS lock carries authority.
#[derive(Debug)]
struct DiscordBindingLease {
    _file: File,
}

impl DiscordBindingLease {
    fn acquire(
        binding: &DiscordChatBinding,
        owner_home_id: &HomeId,
        local_home_id: &HomeId,
    ) -> Result<Self, DiscordError> {
        if owner_home_id != local_home_id {
            return Err(DiscordError::WrongHome {
                owner: owner_home_id.to_string(),
                current: local_home_id.to_string(),
            });
        }
        Self::acquire_at(
            &crate::store::authority_home_dir().join("chat-bindings"),
            binding,
            local_home_id,
        )
    }

    fn acquire_at(
        root: &Path,
        binding: &DiscordChatBinding,
        local_home_id: &HomeId,
    ) -> Result<Self, DiscordError> {
        std::fs::create_dir_all(root)?;
        let mut digest = Sha256::new();
        digest.update(b"discord\0");
        digest.update(binding.guild_id.as_bytes());
        digest.update(b"\0");
        digest.update(binding.channel_id.as_bytes());
        let name = format!("{:x}.lock", digest.finalize());
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join(name))?;
        match FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Self { _file: file }),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                Err(DiscordError::AlreadyOwned {
                    guild_id: binding.guild_id.clone(),
                    channel_id: binding.channel_id.clone(),
                    home_id: local_home_id.to_string(),
                })
            }
            Err(error) => Err(DiscordError::Lease(error)),
        }
    }
}

#[derive(Clone)]
struct DiscordClient {
    http: Client,
    base_url: String,
}

impl std::fmt::Debug for DiscordClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscordClient")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl DiscordClient {
    fn from_env() -> Result<Self, DiscordError> {
        Self::from_token(std::env::var(TOKEN_ENV).ok(), API_BASE)
    }

    fn from_token(token: Option<String>, base_url: &str) -> Result<Self, DiscordError> {
        let token = token
            .filter(|value| !value.trim().is_empty())
            .map(SecretString::new)
            .ok_or(DiscordError::MissingToken)?;
        Self::new(token, base_url)
    }

    fn new(token: SecretString, base_url: &str) -> Result<Self, DiscordError> {
        let mut authorization = HeaderValue::from_str(&format!("Bot {}", token.expose_secret()))
            .map_err(|_| DiscordError::Binding("token contains invalid header bytes".into()))?;
        authorization.set_sensitive(true);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization);
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("Loopflow (https://github.com/loopflowstudio/loopflow)"),
        );
        let http = Client::builder()
            .default_headers(headers)
            .timeout(REQUEST_TIMEOUT)
            .build()?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, DiscordError> {
        self.request(self.http.get(format!("{}{path}", self.base_url)))
            .await
    }

    async fn post<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, DiscordError> {
        self.request(
            self.http
                .post(format!("{}{path}", self.base_url))
                .json(body),
        )
        .await
    }

    async fn request<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, DiscordError> {
        let request = request.build()?;
        loop {
            let response = self
                .http
                .execute(request.try_clone().ok_or_else(|| {
                    DiscordError::Binding("Discord request body was not replayable".into())
                })?)
                .await?;
            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                let retry = response
                    .json::<RateLimit>()
                    .await
                    .map(|limit| limit.retry_after)
                    .unwrap_or(1.0);
                tokio::time::sleep(Duration::from_secs_f64(retry.max(0.05))).await;
                continue;
            }
            if !response.status().is_success() {
                let status = response.status();
                let message = response
                    .json::<ApiError>()
                    .await
                    .map(|error| error.message)
                    .unwrap_or_else(|_| "request rejected".to_string());
                return Err(DiscordError::Api { status, message });
            }
            return Ok(response.json().await?);
        }
    }
}

/// A preflighted channel adapter. Construction performs every permanent
/// binding/intent/permission check before the listener opens the journal.
#[derive(Debug)]
pub struct DiscordAdapter {
    client: DiscordClient,
    binding: DiscordChatBinding,
    bot_user_id: String,
    initial_head: Option<String>,
    _lease: Option<DiscordBindingLease>,
}

impl DiscordAdapter {
    pub async fn preflight(
        binding: DiscordChatBinding,
        owner_home_id: &HomeId,
        local_home_id: &HomeId,
    ) -> Result<Self, DiscordError> {
        let lease = DiscordBindingLease::acquire(&binding, owner_home_id, local_home_id)?;
        let mut adapter = Self::preflight_with_client(DiscordClient::from_env()?, binding).await?;
        adapter._lease = Some(lease);
        Ok(adapter)
    }

    async fn preflight_with_client(
        client: DiscordClient,
        binding: DiscordChatBinding,
    ) -> Result<Self, DiscordError> {
        let bot: User = client.get("/users/@me").await?;
        let application: Application = client.get("/oauth2/applications/@me").await?;
        if application.flags & (MESSAGE_CONTENT | MESSAGE_CONTENT_LIMITED) == 0 {
            return Err(DiscordError::MissingMessageContent);
        }
        let guild: Guild = client.get(&format!("/guilds/{}", binding.guild_id)).await?;
        if guild.id != binding.guild_id {
            return Err(DiscordError::Binding(format!(
                "configured guild {} resolved as {}",
                binding.guild_id, guild.id
            )));
        }
        let member: GuildMember = client
            .get(&format!("/guilds/{}/members/{}", binding.guild_id, bot.id))
            .await?;
        let channel: Channel = client
            .get(&format!("/channels/{}", binding.channel_id))
            .await?;
        if channel.kind != 0 {
            return Err(DiscordError::Binding(format!(
                "channel {} is type {}, expected GUILD_TEXT (0)",
                channel.id, channel.kind
            )));
        }
        if channel.guild_id.as_deref() != Some(binding.guild_id.as_str()) {
            return Err(DiscordError::Binding(format!(
                "channel {} does not belong to guild {}",
                channel.id, binding.guild_id
            )));
        }
        require_permissions(&guild, &member, &channel, &bot.id)?;
        let _: Vec<Message> = client
            .get(&format!(
                "/channels/{}/messages?limit=1",
                binding.channel_id
            ))
            .await?;
        Ok(Self {
            client,
            binding,
            bot_user_id: bot.id,
            initial_head: channel.last_message_id,
            _lease: None,
        })
    }

    pub fn attach(&self, runtime: &WaveRuntime) -> Result<()> {
        runtime
            .try_attach_discord(
                self.binding.clone(),
                self.bot_user_id.clone(),
                self.initial_head.clone(),
            )
            .context("journal Discord attachment")?;
        Ok(())
    }

    pub async fn run(self, runtime: std::sync::Arc<WaveRuntime>) {
        loop {
            match self.sync_once(&runtime).await {
                Ok(()) => tokio::time::sleep(POLL_CADENCE).await,
                Err(error)
                    if error
                        .downcast_ref::<DiscordError>()
                        .is_some_and(DiscordError::retryable) =>
                {
                    tracing::warn!(%error, "Discord chat sync failed; retrying");
                    tokio::time::sleep(ERROR_BACKOFF).await;
                }
                Err(error) => {
                    tracing::error!(%error, "Discord chat sync stopped; restart after correcting the binding or local journal");
                    return;
                }
            }
        }
    }

    async fn sync_once(&self, runtime: &WaveRuntime) -> Result<()> {
        let attachment = runtime
            .discord_snapshot()
            .attachment
            .ok_or_else(|| anyhow!("Discord binding is not attached"))?;
        let channel: Channel = self
            .client
            .get(&format!("/channels/{}", self.binding.channel_id))
            .await?;
        if let Some(head) = channel.last_message_id.as_deref() {
            if attachment.cursor.as_deref() != Some(head) {
                let messages = self
                    .messages_after(attachment.cursor.as_deref(), head)
                    .await?;
                for message in messages {
                    self.accept_message(runtime, message)?;
                }
                runtime
                    .try_advance_discord_cursor(&self.binding, head.to_string())
                    .context("journal Discord cursor")?;
            }
        }
        self.deliver_pending(runtime).await
    }

    async fn messages_after(&self, cursor: Option<&str>, head: &str) -> Result<Vec<Message>> {
        let mut before = increment_snowflake(head)?;
        let cursor = cursor.map(parse_snowflake).transpose()?;
        let mut messages = Vec::new();
        loop {
            let page: Vec<Message> = self
                .client
                .get(&format!(
                    "/channels/{}/messages?limit=100&before={before}",
                    self.binding.channel_id
                ))
                .await?;
            if page.is_empty() {
                break;
            }
            let mut reached_cursor = false;
            for message in &page {
                let id = parse_snowflake(&message.id)?;
                if cursor.is_some_and(|cursor| id <= cursor) {
                    reached_cursor = true;
                    break;
                }
                messages.push(message.clone());
            }
            if reached_cursor || page.len() < 100 {
                break;
            }
            before = page
                .last()
                .map(|message| message.id.clone())
                .expect("non-empty page has a last message");
        }
        messages.sort_by_key(|message| parse_snowflake(&message.id).unwrap_or_default());
        Ok(messages)
    }

    fn accept_message(&self, runtime: &WaveRuntime, message: Message) -> Result<()> {
        if message.author.id == self.bot_user_id {
            self.reconcile_echo(runtime, &message)?;
            return Ok(());
        }
        if message.author.bot == Some(true)
            || message.webhook_id.is_some()
            || message.kind != 0
            || message.content.trim().is_empty()
        {
            return Ok(());
        }
        runtime
            .try_deliver_discord(
                message.content,
                DiscordMessageSource {
                    binding: self.binding.clone(),
                    message_id: message.id,
                    author_id: message.author.id,
                },
            )
            .context("journal Discord input")?;
        Ok(())
    }

    fn reconcile_echo(&self, runtime: &WaveRuntime, message: &Message) -> Result<()> {
        let Some(reply_id) = message
            .message_reference
            .as_ref()
            .and_then(|reference| reference.message_id.as_deref())
        else {
            return Ok(());
        };
        for delivery in runtime.discord_snapshot().deliveries {
            let Some(reply_to) = delivery.reply_to() else {
                continue;
            };
            if reply_to.binding != self.binding {
                continue;
            }
            if reply_to.message_id != reply_id {
                continue;
            }
            if let Some(part) = delivery
                .parts
                .iter()
                .find(|part| !delivery.confirmed.contains_key(&part.part_id))
                .filter(|part| part.content == message.content)
            {
                runtime
                    .try_confirm_discord_part(
                        &delivery.delivery_id,
                        &part.part_id,
                        message.id.clone(),
                    )
                    .context("journal reconciled Discord send")?;
                break;
            }
        }
        Ok(())
    }

    async fn deliver_pending(&self, runtime: &WaveRuntime) -> Result<()> {
        for delivery in runtime.discord_snapshot().deliveries {
            let reply_to = delivery.reply_to().ok_or_else(|| {
                anyhow!(
                    "Discord delivery {} has no authored source",
                    delivery.delivery_id
                )
            })?;
            if reply_to.binding != self.binding {
                continue;
            }
            for part in &delivery.parts {
                if delivery.confirmed.contains_key(&part.part_id) {
                    continue;
                }
                let sent: Message = self
                    .client
                    .post(
                        &format!("/channels/{}/messages", self.binding.channel_id),
                        &CreateMessage {
                            content: &part.content,
                            nonce: &part.nonce,
                            enforce_nonce: true,
                            allowed_mentions: AllowedMentions { parse: Vec::new() },
                            message_reference: MessageReference {
                                message_id: &reply_to.message_id,
                                fail_if_not_exists: false,
                            },
                        },
                    )
                    .await?;
                runtime
                    .try_confirm_discord_part(&delivery.delivery_id, &part.part_id, sent.id)
                    .context("journal Discord send receipt")?;
            }
        }
        Ok(())
    }
}

fn require_permissions(
    guild: &Guild,
    member: &GuildMember,
    channel: &Channel,
    bot_user_id: &str,
) -> Result<(), DiscordError> {
    if guild.owner_id == bot_user_id {
        return Ok(());
    }
    let roles = guild
        .roles
        .iter()
        .map(|role| Ok((role.id.as_str(), parse_permission(&role.permissions)?)))
        .collect::<Result<HashMap<_, _>, DiscordError>>()?;
    let mut permissions = *roles.get(guild.id.as_str()).unwrap_or(&0);
    for role in &member.roles {
        permissions |= roles.get(role.as_str()).copied().unwrap_or(0);
    }
    if permissions & ADMINISTRATOR != 0 {
        return Ok(());
    }
    if let Some(overwrite) = channel
        .permission_overwrites
        .iter()
        .find(|overwrite| overwrite.kind == 0 && overwrite.id == guild.id)
    {
        apply_overwrite(&mut permissions, overwrite)?;
    }
    let mut role_allow = 0;
    let mut role_deny = 0;
    for overwrite in channel.permission_overwrites.iter().filter(|overwrite| {
        overwrite.kind == 0 && member.roles.iter().any(|role| role == &overwrite.id)
    }) {
        role_allow |= parse_permission(&overwrite.allow)?;
        role_deny |= parse_permission(&overwrite.deny)?;
    }
    permissions &= !role_deny;
    permissions |= role_allow;
    if let Some(overwrite) = channel
        .permission_overwrites
        .iter()
        .find(|overwrite| overwrite.kind == 1 && overwrite.id == bot_user_id)
    {
        apply_overwrite(&mut permissions, overwrite)?;
    }
    let required = [
        (VIEW_CHANNEL, "View Channel"),
        (READ_MESSAGE_HISTORY, "Read Message History"),
        (SEND_MESSAGES, "Send Messages"),
    ];
    let missing = required
        .iter()
        .filter_map(|(bit, name)| (permissions & bit == 0).then_some(*name))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(DiscordError::MissingPermissions(missing.join(", ")))
    }
}

fn apply_overwrite(
    permissions: &mut u64,
    overwrite: &PermissionOverwrite,
) -> Result<(), DiscordError> {
    *permissions &= !parse_permission(&overwrite.deny)?;
    *permissions |= parse_permission(&overwrite.allow)?;
    Ok(())
}

fn parse_permission(value: &str) -> Result<u64, DiscordError> {
    value
        .parse()
        .map_err(|_| DiscordError::InvalidPermission(value.to_string()))
}

fn parse_snowflake(value: &str) -> Result<u64> {
    value
        .parse()
        .with_context(|| format!("Discord returned invalid snowflake {value}"))
}

fn increment_snowflake(value: &str) -> Result<String> {
    Ok(parse_snowflake(value)?
        .checked_add(1)
        .ok_or_else(|| anyhow!("Discord snowflake overflow"))?
        .to_string())
}

#[derive(Debug, Deserialize)]
struct RateLimit {
    retry_after: f64,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct User {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bot: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct Application {
    flags: u64,
}

#[derive(Debug, Deserialize)]
struct Guild {
    id: String,
    owner_id: String,
    roles: Vec<Role>,
}

#[derive(Debug, Deserialize)]
struct Role {
    id: String,
    permissions: String,
}

#[derive(Debug, Deserialize)]
struct GuildMember {
    roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Channel {
    id: String,
    guild_id: Option<String>,
    #[serde(rename = "type")]
    kind: u8,
    last_message_id: Option<String>,
    permission_overwrites: Vec<PermissionOverwrite>,
}

#[derive(Debug, Deserialize)]
struct PermissionOverwrite {
    id: String,
    #[serde(rename = "type")]
    kind: u8,
    allow: String,
    deny: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Message {
    id: String,
    author: User,
    content: String,
    #[serde(rename = "type")]
    kind: u8,
    webhook_id: Option<String>,
    message_reference: Option<ReturnedMessageReference>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ReturnedMessageReference {
    message_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateMessage<'a> {
    content: &'a str,
    nonce: &'a str,
    enforce_nonce: bool,
    allowed_mentions: AllowedMentions,
    message_reference: MessageReference<'a>,
}

#[derive(Debug, Serialize)]
struct AllowedMentions {
    parse: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MessageReference<'a> {
    message_id: &'a str,
    fail_if_not_exists: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use axum::body::{to_bytes, Body};
    use axum::extract::State;
    use axum::http::{Request, Response};
    use axum::routing::any;
    use axum::Router;
    use serde_json::json;

    use crate::chat::types::Lifecycle;
    use crate::wave::wire::ResidentDelta;

    #[derive(Debug)]
    struct Fixture {
        application_flags: u64,
        permissions: u64,
        channel_kind: u8,
        messages: Vec<Message>,
        posts: usize,
        lose_next_post_response: bool,
    }

    impl Fixture {
        fn human_message(id: u64) -> Message {
            Message {
                id: id.to_string(),
                author: User {
                    id: format!("human-{id}"),
                    bot: None,
                },
                content: format!("message {id}"),
                kind: 0,
                webhook_id: None,
                message_reference: None,
            }
        }
    }

    async fn fixture_server(fixture: Arc<Mutex<Fixture>>) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture");
        let address = listener.local_addr().expect("fixture address");
        let app = Router::new()
            .route("/{*path}", any(discord_fixture))
            .with_state(fixture);
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve fixture");
        });
        (format!("http://{address}"), task)
    }

    async fn discord_fixture(
        State(fixture): State<Arc<Mutex<Fixture>>>,
        request: Request<Body>,
    ) -> Response<Body> {
        if request
            .headers()
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            != Some("Bot fixture-token")
        {
            return json_response(StatusCode::UNAUTHORIZED, json!({"message": "unauthorized"}));
        }
        let method = request.method().clone();
        let path = request.uri().path().to_string();
        let query = request.uri().query().unwrap_or_default().to_string();
        match (method.as_str(), path.as_str()) {
            ("GET", "/users/@me") => {
                json_response(StatusCode::OK, json!({"id": "bot", "bot": true}))
            }
            ("GET", "/oauth2/applications/@me") => {
                let flags = fixture.lock().expect("fixture").application_flags;
                json_response(StatusCode::OK, json!({"flags": flags}))
            }
            ("GET", "/guilds/guild") => {
                let permissions = fixture.lock().expect("fixture").permissions;
                json_response(
                    StatusCode::OK,
                    json!({
                        "id": "guild",
                        "owner_id": "owner",
                        "roles": [{"id": "guild", "permissions": permissions.to_string()}]
                    }),
                )
            }
            ("GET", "/guilds/guild/members/bot") => {
                json_response(StatusCode::OK, json!({"roles": []}))
            }
            ("GET", "/channels/channel") => {
                let fixture = fixture.lock().expect("fixture");
                json_response(
                    StatusCode::OK,
                    json!({
                        "id": "channel",
                        "guild_id": "guild",
                        "type": fixture.channel_kind,
                        "last_message_id": fixture.messages.last().map(|message| &message.id),
                        "permission_overwrites": []
                    }),
                )
            }
            ("GET", "/channels/channel/messages") => {
                let before = query
                    .split('&')
                    .find_map(|part| part.strip_prefix("before="))
                    .and_then(|value| value.parse::<u64>().ok());
                let limit = query
                    .split('&')
                    .find_map(|part| part.strip_prefix("limit="))
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(50);
                let mut messages = fixture.lock().expect("fixture").messages.clone();
                messages.reverse();
                if let Some(before) = before {
                    messages.retain(|message| message.id.parse::<u64>().unwrap() < before);
                }
                messages.truncate(limit);
                json_response(
                    StatusCode::OK,
                    serde_json::to_value(messages).expect("messages"),
                )
            }
            ("POST", "/channels/channel/messages") => {
                let body = to_bytes(request.into_body(), 16 * 1024)
                    .await
                    .expect("post body");
                let body: serde_json::Value = serde_json::from_slice(&body).expect("message body");
                assert_eq!(body["allowed_mentions"]["parse"], json!([]));
                assert_eq!(body["enforce_nonce"], true);
                assert!(body["nonce"].as_str().expect("nonce").len() <= 25);
                let mut fixture = fixture.lock().expect("fixture");
                fixture.posts += 1;
                let id = fixture
                    .messages
                    .last()
                    .and_then(|message| message.id.parse::<u64>().ok())
                    .unwrap_or(0)
                    + 1;
                let message = Message {
                    id: id.to_string(),
                    author: User {
                        id: "bot".into(),
                        bot: Some(true),
                    },
                    content: body["content"].as_str().expect("content").to_string(),
                    kind: 0,
                    webhook_id: None,
                    message_reference: Some(ReturnedMessageReference {
                        message_id: body["message_reference"]["message_id"]
                            .as_str()
                            .map(str::to_string),
                    }),
                };
                fixture.messages.push(message.clone());
                if std::mem::take(&mut fixture.lose_next_post_response) {
                    Response::builder()
                        .status(StatusCode::OK)
                        .body(Body::from("accepted but response lost"))
                        .expect("response")
                } else {
                    json_response(
                        StatusCode::OK,
                        serde_json::to_value(message).expect("message"),
                    )
                }
            }
            _ => json_response(StatusCode::NOT_FOUND, json!({"message": "not found"})),
        }
    }

    fn json_response(status: StatusCode, value: serde_json::Value) -> Response<Body> {
        Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(Body::from(value.to_string()))
            .expect("response")
    }

    fn binding() -> DiscordChatBinding {
        DiscordChatBinding {
            guild_id: "guild".into(),
            channel_id: "channel".into(),
        }
    }

    fn fixture(messages: Vec<Message>) -> Arc<Mutex<Fixture>> {
        Arc::new(Mutex::new(Fixture {
            application_flags: MESSAGE_CONTENT_LIMITED,
            permissions: VIEW_CHANNEL | READ_MESSAGE_HISTORY | SEND_MESSAGES,
            channel_kind: 0,
            messages,
            posts: 0,
            lose_next_post_response: false,
        }))
    }

    #[test]
    fn discord_chat_requires_a_token_without_reading_or_printing_one() {
        assert!(matches!(
            DiscordClient::from_token(None, "http://unused"),
            Err(DiscordError::MissingToken)
        ));
        let client = DiscordClient::from_token(Some("fixture-token".into()), "http://unused")
            .expect("fixture client");
        assert!(!format!("{client:?}").contains("fixture-token"));
    }

    #[test]
    fn discord_chat_binding_has_one_local_listener() {
        let temp = tempfile::tempdir().expect("tempdir");
        let local = HomeId::new();

        let first = DiscordBindingLease::acquire_at(temp.path(), &binding(), &local)
            .expect("first listener claims binding");
        assert!(matches!(
            DiscordBindingLease::acquire_at(temp.path(), &binding(), &local),
            Err(DiscordError::AlreadyOwned { .. })
        ));
        drop(first);
        DiscordBindingLease::acquire_at(temp.path(), &binding(), &local)
            .expect("binding is released with its listener");
    }

    #[tokio::test]
    async fn discord_chat_rejects_the_wrong_home_before_provider_access() {
        let local = HomeId::new();
        let owner = HomeId::new();

        let error = DiscordAdapter::preflight(binding(), &owner, &local)
            .await
            .expect_err("another Home must not reach token or Discord preflight");

        assert!(matches!(error, DiscordError::WrongHome { .. }));
    }

    #[tokio::test]
    async fn discord_chat_starts_at_head_catches_up_in_order_and_reconciles_a_lost_send() {
        let fixture = fixture(vec![
            Fixture::human_message(99),
            Fixture::human_message(100),
        ]);
        let (base_url, server) = fixture_server(fixture.clone()).await;
        let client = DiscordClient::from_token(Some("fixture-token".into()), &base_url)
            .expect("fixture client");
        let adapter = DiscordAdapter::preflight_with_client(client, binding())
            .await
            .expect("preflight");
        let temp = tempfile::tempdir().expect("tempdir");
        let runtime = WaveRuntime::open("ship".into(), temp.path().to_path_buf()).expect("runtime");
        adapter.attach(&runtime).expect("attach at head");
        adapter.sync_once(&runtime).await.expect("initial sync");
        assert!(
            runtime.pending_messages().is_empty(),
            "history was not imported"
        );

        fixture
            .lock()
            .expect("fixture")
            .messages
            .extend((101..=205).map(Fixture::human_message));
        adapter.sync_once(&runtime).await.expect("paged catch-up");
        let pending = runtime.pending_messages();
        assert_eq!(pending.len(), 105);
        assert!(pending[0].text.ends_with("message 101"));
        assert!(pending[104].text.ends_with("message 205"));
        adapter.sync_once(&runtime).await.expect("duplicate fetch");
        assert_eq!(runtime.pending_messages().len(), 105);

        let answers = pending.iter().map(|message| message.id.0.clone()).collect();
        runtime.apply_resident_delta(ResidentDelta::TurnOpened { answers });
        runtime.apply_resident_delta(ResidentDelta::TurnText {
            text: "x".repeat(2_001),
        });
        runtime.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            cost_usd: None,
            reason: None,
        });
        assert_eq!(
            runtime.discord_snapshot().deliveries[0]
                .reply_to()
                .map(|source| source.message_id.as_str()),
            Some("205"),
            "the ordered source list makes the newest claim the reply target"
        );
        fixture.lock().expect("fixture").lose_next_post_response = true;
        assert!(adapter.deliver_pending(&runtime).await.is_err());
        assert_eq!(fixture.lock().expect("fixture").posts, 1);
        assert!(runtime.discord_snapshot().deliveries[0]
            .confirmed
            .is_empty());

        drop(runtime);
        let client = DiscordClient::from_token(Some("fixture-token".into()), &base_url)
            .expect("restart fixture client");
        let adapter = DiscordAdapter::preflight_with_client(client, binding())
            .await
            .expect("restart preflight");
        let runtime =
            WaveRuntime::open("ship".into(), temp.path().to_path_buf()).expect("restart runtime");
        adapter.attach(&runtime).expect("reattach after restart");
        adapter
            .sync_once(&runtime)
            .await
            .expect("restart reconciles accepted send");
        assert_eq!(
            fixture.lock().expect("fixture").posts,
            2,
            "the accepted first part was reconciled; only the second part posted"
        );
        assert_eq!(runtime.discord_snapshot().deliveries[0].confirmed.len(), 2);
        assert!(
            runtime.pending_messages().is_empty(),
            "self echo is not input"
        );
        server.abort();
    }

    #[tokio::test]
    async fn discord_chat_preflight_rejects_missing_message_content() {
        let fixture = fixture(Vec::new());
        fixture.lock().expect("fixture").application_flags = 0;
        let (base_url, server) = fixture_server(fixture).await;
        let client = DiscordClient::from_token(Some("fixture-token".into()), &base_url)
            .expect("fixture client");
        assert!(matches!(
            DiscordAdapter::preflight_with_client(client, binding()).await,
            Err(DiscordError::MissingMessageContent)
        ));
        server.abort();
    }

    #[tokio::test]
    async fn discord_chat_preflight_rejects_non_text_channels() {
        let fixture = fixture(Vec::new());
        fixture.lock().expect("fixture").channel_kind = 11;
        let (base_url, server) = fixture_server(fixture).await;
        let client = DiscordClient::from_token(Some("fixture-token".into()), &base_url)
            .expect("fixture client");
        let error = DiscordAdapter::preflight_with_client(client, binding())
            .await
            .expect_err("thread channel must fail")
            .to_string();
        assert!(error.contains("GUILD_TEXT"), "{error}");
        server.abort();
    }

    #[test]
    fn discord_chat_permission_overwrites_are_applied_in_discord_order() {
        let guild = Guild {
            id: "1".into(),
            owner_id: "owner".into(),
            roles: vec![
                Role {
                    id: "1".into(),
                    permissions: (VIEW_CHANNEL | READ_MESSAGE_HISTORY).to_string(),
                },
                Role {
                    id: "2".into(),
                    permissions: "0".into(),
                },
            ],
        };
        let member = GuildMember {
            roles: vec!["2".into()],
        };
        let channel = Channel {
            id: "3".into(),
            guild_id: Some("1".into()),
            kind: 0,
            last_message_id: None,
            permission_overwrites: vec![PermissionOverwrite {
                id: "2".into(),
                kind: 0,
                allow: SEND_MESSAGES.to_string(),
                deny: "0".into(),
            }],
        };
        assert!(require_permissions(&guild, &member, &channel, "bot").is_ok());
    }

    #[test]
    fn discord_chat_reports_each_missing_permission() {
        let channel = Channel {
            id: "3".into(),
            guild_id: Some("1".into()),
            kind: 0,
            last_message_id: None,
            permission_overwrites: Vec::new(),
        };
        for (missing, name) in [
            (VIEW_CHANNEL, "View Channel"),
            (READ_MESSAGE_HISTORY, "Read Message History"),
            (SEND_MESSAGES, "Send Messages"),
        ] {
            let guild = Guild {
                id: "1".into(),
                owner_id: "owner".into(),
                roles: vec![Role {
                    id: "1".into(),
                    permissions: ((VIEW_CHANNEL | READ_MESSAGE_HISTORY | SEND_MESSAGES) & !missing)
                        .to_string(),
                }],
            };
            let error =
                require_permissions(&guild, &GuildMember { roles: Vec::new() }, &channel, "bot")
                    .expect_err("permissions must fail")
                    .to_string();
            assert!(error.contains(name), "{error}");
        }
    }
}
