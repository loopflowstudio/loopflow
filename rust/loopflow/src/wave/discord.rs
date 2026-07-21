//! One Discord guild text channel as a presentation surface for Wave chat.
//!
//! Discord owns the company transcript. This adapter polls only messages after
//! the journal's committed cursor and writes source-linked inputs plus outbound
//! delivery receipts through [`WaveRuntime`]. It never owns an inbox or writes
//! the journal directly.

use std::collections::{HashMap, HashSet};
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
use tokio::sync::watch;

use crate::chat::turns::{ChatRole, ChatTurn};
use crate::durable::HomeId;
use crate::wave::chat::{ChatBackingHealth, ChatMessageSource, ConversationEpoch, WaveChatMessage};
use crate::wave::journal::{DiscordChatBinding, DiscordMessageSource, MessageOp};
use crate::wave::runtime::WaveRuntime;

pub const TOKEN_ENV: &str = "LF_DISCORD_TOKEN";
const API_BASE: &str = "https://discord.com/api/v10";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_CADENCE: Duration = Duration::from_secs(2);
const ERROR_BACKOFF: Duration = Duration::from_secs(10);
const MESSAGE_LIMIT: usize = 2_000;
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
    #[error("Discord messages are limited to {limit} characters; this message has {actual}")]
    MessageTooLong { limit: usize, actual: usize },
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
        Self::from_secret(token.map(SecretString::new), base_url)
    }

    fn from_secret(token: Option<SecretString>, base_url: &str) -> Result<Self, DiscordError> {
        let token = token
            .filter(|value| !value.expose_secret().trim().is_empty())
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
    health: watch::Sender<ChatBackingHealth>,
    _lease: Option<DiscordBindingLease>,
}

/// Cloneable, read-only provider projection used by product API reads.
#[derive(Debug, Clone)]
pub(crate) struct DiscordProjection {
    client: DiscordClient,
    binding: DiscordChatBinding,
    bot_user_id: String,
    health: watch::Receiver<ChatBackingHealth>,
}

impl DiscordAdapter {
    pub async fn preflight(
        binding: DiscordChatBinding,
        owner_home_id: &HomeId,
        local_home_id: &HomeId,
        token: Option<SecretString>,
    ) -> Result<Self, DiscordError> {
        let lease = DiscordBindingLease::acquire(&binding, owner_home_id, local_home_id)?;
        let client = match token {
            Some(token) => DiscordClient::from_secret(Some(token), API_BASE)?,
            None => DiscordClient::from_env()?,
        };
        let mut adapter = Self::preflight_with_client(client, binding).await?;
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
        let (health, _) = watch::channel(ChatBackingHealth::Ready);
        Ok(Self {
            client,
            binding,
            bot_user_id: bot.id,
            initial_head: channel.last_message_id,
            health,
            _lease: None,
        })
    }

    pub fn projection(&self) -> DiscordProjection {
        DiscordProjection {
            client: self.client.clone(),
            binding: self.binding.clone(),
            bot_user_id: self.bot_user_id.clone(),
            health: self.health.subscribe(),
        }
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
                Ok(()) => {
                    self.health.send_replace(ChatBackingHealth::Ready);
                    tokio::time::sleep(POLL_CADENCE).await;
                }
                Err(error)
                    if error
                        .downcast_ref::<DiscordError>()
                        .is_some_and(DiscordError::retryable) =>
                {
                    tracing::warn!(%error, "Discord chat sync failed; retrying");
                    self.health.send_replace(ChatBackingHealth::Retrying {
                        detail: error.to_string(),
                    });
                    tokio::time::sleep(ERROR_BACKOFF).await;
                }
                Err(error) => {
                    tracing::error!(%error, "Discord chat sync stopped; restart after correcting the binding or local journal");
                    self.health.send_replace(ChatBackingHealth::Blocked {
                        detail: error.to_string(),
                    });
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
        if !message_in_epoch(&message, &runtime.active_conversation_epoch())
            .context("validate Discord input epoch")?
        {
            return Ok(());
        }
        if message.author.id == self.bot_user_id {
            if self.reconcile_echo(runtime, &message)? {
                return Ok(());
            }
            let Some((op, text)) = parse_authored_content(runtime.name(), &message.content) else {
                return Ok(());
            };
            runtime
                .try_deliver_discord_authored(
                    text,
                    DiscordMessageSource {
                        binding: self.binding.clone(),
                        message_id: message.id,
                        author_id: message.author.id,
                    },
                    op,
                )
                .context("journal Discord app input")?;
            return Ok(());
        }
        if message.author.bot == Some(true)
            || message.webhook_id.is_some()
            || !matches!(message.kind, 0 | 19)
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

    fn reconcile_echo(&self, runtime: &WaveRuntime, message: &Message) -> Result<bool> {
        let reply_id = message
            .message_reference
            .as_ref()
            .and_then(|reference| reference.message_id.as_deref());
        for delivery in runtime.discord_snapshot().deliveries {
            if delivery.binding != self.binding {
                continue;
            }
            if delivery.reply_to().map(|source| source.message_id.as_str()) != reply_id {
                continue;
            }
            if delivery
                .confirmed
                .values()
                .any(|provider_id| provider_id == &message.id)
            {
                return Ok(true);
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
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn deliver_pending(&self, runtime: &WaveRuntime) -> Result<()> {
        for delivery in runtime.discord_snapshot().deliveries {
            if delivery.binding != self.binding {
                continue;
            }
            let reply_to = delivery.reply_to();
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
                            message_reference: reply_to.map(|source| MessageReference {
                                message_id: &source.message_id,
                                fail_if_not_exists: false,
                            }),
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

impl DiscordProjection {
    pub fn health(&self) -> ChatBackingHealth {
        self.health.borrow().clone()
    }

    pub(crate) async fn post_authored(
        &self,
        runtime: &WaveRuntime,
        op: MessageOp,
        text: &str,
        request_id: &str,
    ) -> Result<WaveChatMessage, DiscordError> {
        let epoch = runtime.active_conversation_epoch();
        if epoch.backing.discord_binding().as_ref() != Some(&self.binding) {
            return Err(DiscordError::Binding(format!(
                "active chat epoch {} is not backed by channel {}/{}",
                epoch.id, self.binding.guild_id, self.binding.channel_id
            )));
        }
        let content = authored_content(runtime.name(), op, text);
        let actual = content.chars().count();
        if actual > MESSAGE_LIMIT {
            return Err(DiscordError::MessageTooLong {
                limit: MESSAGE_LIMIT,
                actual,
            });
        }
        let nonce = authored_nonce(request_id);
        let path = format!("/channels/{}/messages", self.binding.channel_id);
        let body = CreateMessage {
            content: &content,
            nonce: &nonce,
            enforce_nonce: true,
            allowed_mentions: AllowedMentions { parse: Vec::new() },
            message_reference: None,
        };
        let sent: Message = match self.client.post(&path, &body).await {
            Err(error) if error.retryable() => self.client.post(&path, &body).await?,
            result => result?,
        };
        if sent.author.id != self.bot_user_id {
            return Err(DiscordError::Binding(format!(
                "authored message {} was returned with unexpected author {}",
                sent.id, sent.author.id
            )));
        }
        let Some((sent_op, sent_text)) = parse_authored_content(runtime.name(), &sent.content)
        else {
            return Err(DiscordError::Binding(format!(
                "authored message {} did not retain its Loopflow header",
                sent.id
            )));
        };
        if sent_op != op || sent_text != text {
            return Err(DiscordError::Binding(format!(
                "request id already committed a different Discord message {}",
                sent.id
            )));
        }
        project_message(&self.binding, &epoch, sent, ChatRole::User, sent_text)
            .map_err(|error| DiscordError::Binding(error.to_string()))
    }

    pub async fn history(
        &self,
        runtime: &WaveRuntime,
        epoch: &ConversationEpoch,
        limit: Option<usize>,
    ) -> Result<Vec<WaveChatMessage>> {
        if epoch.backing.discord_binding().as_ref() != Some(&self.binding) {
            return Err(anyhow!(
                "chat epoch {} is not backed by Discord channel {}/{}",
                epoch.id,
                self.binding.guild_id,
                self.binding.channel_id
            ));
        }
        let requested = limit.unwrap_or(12);
        if requested == 0 {
            return Ok(Vec::new());
        }
        let confirmed = runtime
            .discord_snapshot()
            .deliveries
            .into_iter()
            .filter(|delivery| delivery.binding == self.binding)
            .flat_map(|delivery| delivery.confirmed.into_values())
            .collect::<HashSet<_>>();
        // Discord pages may contain system or unrelated bot messages. Read at
        // least one full provider page so those records cannot starve a small
        // product history request.
        let messages = self.latest_messages(requested.max(100)).await?;
        let mut projected = messages
            .into_iter()
            .filter(|message| message_in_epoch(message, epoch).unwrap_or(false))
            .filter_map(|message| {
                if message.webhook_id.is_some()
                    || !matches!(message.kind, 0 | 19)
                    || message.content.trim().is_empty()
                {
                    return None;
                }
                let (role, text) = if message.author.id == self.bot_user_id {
                    if confirmed.contains(&message.id) {
                        (ChatRole::Assistant, message.content.clone())
                    } else {
                        let (_, text) = parse_authored_content(runtime.name(), &message.content)?;
                        (ChatRole::User, text)
                    }
                } else {
                    if message.author.bot == Some(true) {
                        return None;
                    }
                    (ChatRole::User, message.content.clone())
                };
                project_message(&self.binding, epoch, message, role, text).ok()
            })
            .collect::<Vec<_>>();
        if projected.len() > requested {
            projected.drain(..projected.len() - requested);
        }
        Ok(projected)
    }

    async fn latest_messages(&self, limit: usize) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut before: Option<String> = None;
        while messages.len() < limit {
            let page_limit = (limit - messages.len()).min(100);
            let before_query = before
                .as_deref()
                .map(|id| format!("&before={id}"))
                .unwrap_or_default();
            let page: Vec<Message> = self
                .client
                .get(&format!(
                    "/channels/{}/messages?limit={page_limit}{before_query}",
                    self.binding.channel_id
                ))
                .await?;
            if page.is_empty() {
                break;
            }
            before = page.last().map(|message| message.id.clone());
            let page_len = page.len();
            messages.extend(page);
            if page_len < page_limit {
                break;
            }
        }
        messages.reverse();
        Ok(messages)
    }
}

fn message_in_epoch(message: &Message, epoch: &ConversationEpoch) -> Result<bool> {
    let timestamp = snowflake_timestamp(&message.id)?;
    let started_at = time::OffsetDateTime::parse(
        &epoch.started_at,
        &time::format_description::well_known::Rfc3339,
    )?;
    let ended_at = epoch
        .ended_at
        .as_deref()
        .map(|value| {
            time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
        })
        .transpose()?;
    Ok(timestamp >= started_at && ended_at.is_none_or(|ended_at| timestamp < ended_at))
}

fn authored_content(wave: &str, op: MessageOp, text: &str) -> String {
    format!("{}\n{text}", authored_header(wave, op))
}

fn authored_header(wave: &str, op: MessageOp) -> String {
    let action = match op {
        MessageOp::Message => String::new(),
        MessageOp::Steer => " · steer".to_string(),
        MessageOp::Interrupt => " · interrupt".to_string(),
    };
    format!("**[{wave} · Loopflow app{action}]**")
}

fn parse_authored_content(wave: &str, content: &str) -> Option<(MessageOp, String)> {
    for op in [MessageOp::Message, MessageOp::Steer, MessageOp::Interrupt] {
        let header = authored_header(wave, op);
        if let Some(text) = content
            .strip_prefix(&header)
            .and_then(|rest| rest.strip_prefix('\n'))
        {
            return Some((op, text.to_string()));
        }
    }
    None
}

fn authored_nonce(request_id: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"loopflow-discord-authored\0");
    digest.update(request_id.as_bytes());
    format!("lf-u-{}", &format!("{:x}", digest.finalize())[..16])
}

fn project_message(
    binding: &DiscordChatBinding,
    epoch: &ConversationEpoch,
    message: Message,
    role: ChatRole,
    text: String,
) -> Result<WaveChatMessage> {
    let created_at =
        snowflake_timestamp(&message.id)?.format(&time::format_description::well_known::Rfc3339)?;
    let mut turn = ChatTurn::user(format!("discord-{}", message.id), text);
    turn.role = role;
    turn.created_at = created_at;
    Ok(WaveChatMessage {
        epoch_id: epoch.id.clone(),
        source: ChatMessageSource::Discord {
            guild_id: binding.guild_id.clone(),
            channel_id: binding.channel_id.clone(),
            message_id: message.id.clone(),
            author_id: message.author.id,
            url: binding.message_url(&message.id),
        },
        turn,
    })
}

fn snowflake_timestamp(value: &str) -> Result<time::OffsetDateTime> {
    const DISCORD_EPOCH_MILLIS: i128 = 1_420_070_400_000;
    let snowflake = parse_snowflake(value)?;
    let millis = i128::from(snowflake >> 22) + DISCORD_EPOCH_MILLIS;
    time::OffsetDateTime::from_unix_timestamp_nanos(millis * 1_000_000).map_err(anyhow::Error::from)
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
    #[serde(skip_serializing_if = "Option::is_none")]
    message_reference: Option<MessageReference<'a>>,
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
    use crate::wave::chat::ChatBacking;
    use crate::wave::wire::ResidentDelta;

    #[derive(Debug)]
    struct Fixture {
        application_flags: u64,
        permissions: u64,
        channel_kind: u8,
        messages: Vec<Message>,
        nonces: HashMap<String, Message>,
        posts: usize,
        lose_next_post_response: bool,
        channel_status: Option<u16>,
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
                if let Some(status) = fixture.channel_status {
                    return json_response(
                        StatusCode::from_u16(status).expect("fixture status"),
                        json!({"message": "fixture channel failure"}),
                    );
                }
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
                let nonce = body["nonce"].as_str().expect("nonce").to_string();
                assert!(nonce.len() <= 25);
                let mut fixture = fixture.lock().expect("fixture");
                fixture.posts += 1;
                if let Some(message) = fixture.nonces.get(&nonce) {
                    return json_response(
                        StatusCode::OK,
                        serde_json::to_value(message).expect("message"),
                    );
                }
                let id = fixture
                    .messages
                    .last()
                    .and_then(|message| message.id.parse::<u64>().ok())
                    .map(|id| id + 1)
                    .unwrap_or_else(|| snowflake_at_offset(1, fixture.posts as u64));
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
                fixture.nonces.insert(nonce, message.clone());
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
            nonces: HashMap::new(),
            posts: 0,
            lose_next_post_response: false,
            channel_status: None,
        }))
    }

    fn snowflake_at_offset(seconds: i64, increment: u64) -> u64 {
        const DISCORD_EPOCH_MILLIS: i128 = 1_420_070_400_000;
        let timestamp = time::OffsetDateTime::now_utc() + time::Duration::seconds(seconds);
        let millis = timestamp.unix_timestamp_nanos() / 1_000_000;
        (((millis - DISCORD_EPOCH_MILLIS) as u64) << 22) | increment
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

        let error = DiscordAdapter::preflight(binding(), &owner, &local, None)
            .await
            .expect_err("another Home must not reach token or Discord preflight");

        assert!(matches!(error, DiscordError::WrongHome { .. }));
    }

    #[tokio::test]
    async fn discord_chat_starts_at_head_catches_up_in_order_and_reconciles_a_lost_send() {
        let fixture = fixture(vec![
            Fixture::human_message(snowflake_at_offset(-1, 99)),
            Fixture::human_message(snowflake_at_offset(-1, 100)),
        ]);
        let (base_url, server) = fixture_server(fixture.clone()).await;
        let client = DiscordClient::from_token(Some("fixture-token".into()), &base_url)
            .expect("fixture client");
        let adapter = DiscordAdapter::preflight_with_client(client, binding())
            .await
            .expect("preflight");
        let temp = tempfile::tempdir().expect("tempdir");
        let runtime = WaveRuntime::open_with_backing(
            "ship".into(),
            temp.path().to_path_buf(),
            crate::wave::chat::ChatBacking::discord(&binding()),
        )
        .expect("runtime");
        adapter.attach(&runtime).expect("attach at head");
        adapter.sync_once(&runtime).await.expect("initial sync");
        assert!(
            runtime.pending_messages().is_empty(),
            "history was not imported"
        );

        let catch_up_ids: Vec<_> = (101..=205)
            .map(|increment| snowflake_at_offset(1, increment))
            .collect();
        let first_id = catch_up_ids.first().expect("first catch-up id").to_string();
        let newest_id = catch_up_ids.last().expect("newest catch-up id").to_string();
        fixture
            .lock()
            .expect("fixture")
            .messages
            .extend(catch_up_ids.into_iter().map(Fixture::human_message));
        adapter.sync_once(&runtime).await.expect("paged catch-up");
        let pending = runtime.pending_messages();
        assert_eq!(pending.len(), 105);
        assert!(pending[0].text.ends_with(&format!("message {first_id}")));
        assert!(pending[104].text.ends_with(&format!("message {newest_id}")));
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
            Some(newest_id.as_str()),
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
        let runtime = WaveRuntime::open_with_backing(
            "ship".into(),
            temp.path().to_path_buf(),
            crate::wave::chat::ChatBacking::discord(&binding()),
        )
        .expect("restart runtime");
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
    async fn discord_projection_reads_normal_and_reply_messages_without_copying_history() {
        let fixture = fixture(Vec::new());
        let (base_url, server) = fixture_server(fixture.clone()).await;
        let client = DiscordClient::from_token(Some("fixture-token".into()), &base_url)
            .expect("fixture client");
        let adapter = DiscordAdapter::preflight_with_client(client, binding())
            .await
            .expect("preflight");
        let projection = adapter.projection();
        let temp = tempfile::tempdir().expect("tempdir");
        let runtime = WaveRuntime::open_with_backing(
            "ship".into(),
            temp.path().to_path_buf(),
            crate::wave::chat::ChatBacking::discord(&binding()),
        )
        .expect("runtime");
        adapter.attach(&runtime).expect("attach");

        let normal_id = snowflake_at_offset(1, 1);
        let reply_id = snowflake_at_offset(2, 2);
        let unrelated_bot_id = snowflake_at_offset(3, 3);
        let mut reply = Fixture::human_message(reply_id);
        reply.kind = 19;
        fixture.lock().expect("fixture").messages.extend([
            Fixture::human_message(normal_id),
            reply,
            Message {
                id: unrelated_bot_id.to_string(),
                author: User {
                    id: "another-bot".into(),
                    bot: Some(true),
                },
                content: "not this Wave".into(),
                kind: 0,
                webhook_id: None,
                message_reference: None,
            },
        ]);

        adapter
            .sync_once(&runtime)
            .await
            .expect("ingest provider head");
        assert_eq!(
            runtime.pending_messages().len(),
            2,
            "normal and reply messages both reach the resident inbox"
        );
        adapter
            .sync_once(&runtime)
            .await
            .expect("repeat sync stays idempotent");
        assert_eq!(
            runtime.pending_messages().len(),
            2,
            "provider messages reach the resident exactly once"
        );
        let journal_before = crate::wave::journal::read_events(
            &crate::wave::journal::journal_path(temp.path(), "ship"),
        );
        let epoch = runtime.active_conversation_epoch();
        let messages = projection
            .history(&runtime, &epoch, Some(12))
            .await
            .expect("project provider history");
        let journal_after = crate::wave::journal::read_events(&crate::wave::journal::journal_path(
            temp.path(),
            "ship",
        ));

        assert_eq!(
            journal_before, journal_after,
            "history reads append nothing"
        );
        assert_eq!(messages.len(), 2, "unrelated bot speech stays out");
        assert_eq!(messages[0].turn.id, format!("discord-{normal_id}"));
        assert_eq!(messages[1].turn.id, format!("discord-{reply_id}"));
        assert!(messages
            .iter()
            .all(|message| matches!(message.source, ChatMessageSource::Discord { .. })));

        let answers = runtime
            .pending_messages()
            .into_iter()
            .map(|message| message.id.0)
            .collect();
        runtime.apply_resident_delta(ResidentDelta::TurnOpened { answers });
        runtime.apply_resident_delta(ResidentDelta::TurnText {
            text: "provider-committed answer".into(),
        });
        runtime.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            cost_usd: None,
            reason: None,
        });
        assert_eq!(
            projection
                .history(&runtime, &epoch, Some(12))
                .await
                .expect("history before provider receipt")
                .len(),
            2,
            "the internal assistant turn is not a chat preview"
        );
        adapter
            .deliver_pending(&runtime)
            .await
            .expect("provider accepts answer");
        let committed = projection
            .history(&runtime, &epoch, Some(12))
            .await
            .expect("history after provider receipt");
        assert_eq!(committed.len(), 3);
        assert_eq!(
            committed.last().expect("assistant message").turn.role,
            ChatRole::Assistant
        );
        assert_eq!(
            committed.last().expect("assistant message").turn.text,
            "provider-committed answer"
        );
        server.abort();
    }

    #[tokio::test]
    async fn native_compose_posts_to_discord_and_reenters_as_a_steer() {
        let fixture = fixture(Vec::new());
        let (base_url, discord_server) = fixture_server(fixture.clone()).await;
        let client = DiscordClient::from_token(Some("fixture-token".into()), &base_url)
            .expect("fixture client");
        let adapter = DiscordAdapter::preflight_with_client(client, binding())
            .await
            .expect("preflight");
        let projection = adapter.projection();
        let temp = tempfile::tempdir().expect("tempdir");
        let runtime = WaveRuntime::open_with_backing(
            "ship".into(),
            temp.path().to_path_buf(),
            ChatBacking::discord(&binding()),
        )
        .expect("runtime");
        adapter.attach(&runtime).expect("attach");
        fixture.lock().expect("fixture").lose_next_post_response = true;
        let app = crate::wave::server::router_with_chat_projection(
            runtime.clone(),
            crate::wave::server::ResidentDoor::new("resident"),
            Arc::new(crate::wave::registry::ObserverSlot::new(
                runtime.clone(),
                None,
            )),
            None,
            crate::wave::server::ShutdownDoor::new(),
            Some(projection.clone()),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind Wave listener");
        let address = listener.local_addr().expect("Wave listener address");
        let wave_server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve Wave");
        });

        let client = reqwest::Client::new();
        let url = format!("http://{address}/messages");
        let body = json!({
            "id": "request-1",
            "op": "steer",
            "text": "favor reliability"
        });
        let response = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .expect("post native message");
        assert_eq!(response.status(), StatusCode::OK);
        let posted: crate::wave::chat::PostMessageResponse =
            response.json().await.expect("posted response");
        let posted = posted.message.expect("provider-backed message");
        assert_eq!(posted.turn.role, ChatRole::User);
        assert_eq!(posted.turn.text, "favor reliability");
        assert!(matches!(&posted.source, ChatMessageSource::Discord { .. }));
        assert_eq!(
            fixture.lock().expect("fixture").messages[0].content,
            "**[ship · Loopflow app · steer]**\nfavor reliability"
        );
        let retried: crate::wave::chat::PostMessageResponse = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .expect("retry native message")
            .json()
            .await
            .expect("retry response");
        assert_eq!(
            retried.message.expect("retried message").source,
            posted.source
        );
        assert_eq!(fixture.lock().expect("fixture").messages.len(), 1);
        assert_eq!(fixture.lock().expect("fixture").posts, 3);

        adapter.sync_once(&runtime).await.expect("ingest bot echo");
        let pending = runtime.pending_messages();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].op, MessageOp::Steer);
        assert!(pending[0].text.ends_with("favor reliability"));
        adapter
            .sync_once(&runtime)
            .await
            .expect("repeat sync stays idempotent");
        assert_eq!(runtime.pending_messages().len(), 1);
        let history = projection
            .history(&runtime, &runtime.active_conversation_epoch(), Some(12))
            .await
            .expect("native provider history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].turn.role, ChatRole::User);
        assert_eq!(history[0].turn.text, "favor reliability");

        wave_server.abort();
        discord_server.abort();
    }

    #[tokio::test]
    async fn discord_input_starts_at_the_durable_epoch_boundary() {
        let fixture = fixture(Vec::new());
        let (base_url, server) = fixture_server(fixture.clone()).await;
        let client = DiscordClient::from_token(Some("fixture-token".into()), &base_url)
            .expect("fixture client");
        let adapter = DiscordAdapter::preflight_with_client(client, binding())
            .await
            .expect("preflight");
        let projection = adapter.projection();
        let temp = tempfile::tempdir().expect("tempdir");
        let runtime = WaveRuntime::open_with_backing(
            "ship".into(),
            temp.path().to_path_buf(),
            ChatBacking::discord(&binding()),
        )
        .expect("runtime");
        adapter.attach(&runtime).expect("attach");

        let before_epoch = snowflake_at_offset(-1, 1);
        fixture
            .lock()
            .expect("fixture")
            .messages
            .push(Fixture::human_message(before_epoch));
        adapter
            .sync_once(&runtime)
            .await
            .expect("advance past pre-epoch input");
        assert!(
            runtime.pending_messages().is_empty(),
            "a provider message before the durable epoch is not resident input"
        );
        assert!(
            projection
                .history(&runtime, &runtime.active_conversation_epoch(), Some(12))
                .await
                .expect("project history")
                .is_empty(),
            "the inbox and provider projection share one epoch boundary"
        );

        let inside_epoch = snowflake_at_offset(1, 2);
        fixture
            .lock()
            .expect("fixture")
            .messages
            .push(Fixture::human_message(inside_epoch));
        adapter
            .sync_once(&runtime)
            .await
            .expect("ingest active-epoch input");
        assert_eq!(runtime.pending_messages().len(), 1);
        let messages = projection
            .history(&runtime, &runtime.active_conversation_epoch(), Some(12))
            .await
            .expect("project active history");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].turn.id, format!("discord-{inside_epoch}"));
        server.abort();
    }

    #[tokio::test]
    async fn discord_adapter_publishes_retrying_and_blocked_health() {
        async fn wait_for(
            projection: &DiscordProjection,
            expected: fn(&ChatBackingHealth) -> bool,
        ) {
            for _ in 0..100 {
                if expected(&projection.health()) {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            panic!("Discord health did not reach the expected state");
        }

        let retry_fixture = fixture(Vec::new());
        let (base_url, retry_server) = fixture_server(retry_fixture.clone()).await;
        let client = DiscordClient::from_token(Some("fixture-token".into()), &base_url)
            .expect("fixture client");
        let adapter = DiscordAdapter::preflight_with_client(client, binding())
            .await
            .expect("preflight");
        let projection = adapter.projection();
        let retry_temp = tempfile::tempdir().expect("tempdir");
        let runtime = WaveRuntime::open_with_backing(
            "ship".into(),
            retry_temp.path().to_path_buf(),
            ChatBacking::discord(&binding()),
        )
        .expect("runtime");
        adapter.attach(&runtime).expect("attach");
        retry_fixture.lock().expect("fixture").channel_status = Some(500);
        let retry_task = tokio::spawn(adapter.run(runtime));
        wait_for(&projection, |health| {
            matches!(health, ChatBackingHealth::Retrying { .. })
        })
        .await;
        retry_task.abort();
        retry_server.abort();

        let blocked_fixture = fixture(Vec::new());
        let (base_url, blocked_server) = fixture_server(blocked_fixture.clone()).await;
        let client = DiscordClient::from_token(Some("fixture-token".into()), &base_url)
            .expect("fixture client");
        let adapter = DiscordAdapter::preflight_with_client(client, binding())
            .await
            .expect("preflight");
        let projection = adapter.projection();
        let blocked_temp = tempfile::tempdir().expect("tempdir");
        let runtime = WaveRuntime::open_with_backing(
            "ship".into(),
            blocked_temp.path().to_path_buf(),
            ChatBacking::discord(&binding()),
        )
        .expect("runtime");
        adapter.attach(&runtime).expect("attach");
        blocked_fixture.lock().expect("fixture").channel_status = Some(403);
        adapter.run(runtime).await;
        wait_for(&projection, |health| {
            matches!(health, ChatBackingHealth::Blocked { .. })
        })
        .await;
        assert!(matches!(
            projection.health(),
            ChatBackingHealth::Blocked { .. }
        ));
        blocked_server.abort();
    }

    #[tokio::test]
    async fn discord_adapter_never_sends_another_epochs_delivery() {
        let fixture = fixture(Vec::new());
        let (base_url, server) = fixture_server(fixture.clone()).await;
        let client = DiscordClient::from_token(Some("fixture-token".into()), &base_url)
            .expect("fixture client");
        let adapter = DiscordAdapter::preflight_with_client(client, binding())
            .await
            .expect("preflight");
        let other_binding = DiscordChatBinding {
            guild_id: "other-guild".into(),
            channel_id: "other-channel".into(),
        };
        let temp = tempfile::tempdir().expect("tempdir");
        let runtime = WaveRuntime::open_with_backing(
            "ship".into(),
            temp.path().to_path_buf(),
            ChatBacking::discord(&other_binding),
        )
        .expect("runtime");
        runtime.apply_resident_delta(ResidentDelta::TurnOpened {
            answers: Vec::new(),
        });
        runtime.apply_resident_delta(ResidentDelta::TurnText {
            text: "belongs to the earlier channel".into(),
        });
        runtime.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            cost_usd: None,
            reason: None,
        });

        adapter
            .deliver_pending(&runtime)
            .await
            .expect("foreign delivery is ignored");
        assert_eq!(fixture.lock().expect("fixture").posts, 0);
        assert_eq!(
            runtime.discord_snapshot().deliveries[0].binding,
            other_binding
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
