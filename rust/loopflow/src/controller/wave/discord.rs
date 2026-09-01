//! One Discord guild text channel as a presentation surface for Wave chat.
//!
//! Discord owns the company transcript. This adapter receives new messages from
//! the Gateway, catches up from the journal's committed cursor after reconnect,
//! and writes source-linked inputs plus outbound delivery receipts through
//! [`WaveRuntime`]. It never owns an inbox or writes the journal directly.

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use fs2::FileExt;
use futures_util::{SinkExt, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use reqwest::{Client, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::chat::turns::{ChatRole, ChatTurn};
use crate::controller::wave::chat::{
    ChatBackingHealth, ChatMessageSource, ConversationEpoch, WaveChatMessage,
};
use crate::controller::wave::journal::{DiscordChatBinding, DiscordMessageSource, MessageOp};
use crate::controller::wave::runtime::WaveRuntime;

pub const TOKEN_ENV: &str = crate::engine::process::DISCORD_TOKEN_ENV;
const API_BASE: &str = "https://discord.com/api/v10";
const GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// How often the gateway-driven loop flushes pending outbound sends. Inbound is
/// push (gateway events); only outbound needs its own cadence.
const OUTBOUND_TICK: Duration = Duration::from_secs(1);
const ERROR_BACKOFF: Duration = Duration::from_secs(10);
/// Discord asks apps to wait a short beat before re-IDENTIFYing after an
/// INVALID_SESSION, so a repeated invalid session isn't a tight reconnect loop.
const REIDENTIFY_BACKOFF: Duration = Duration::from_secs(2);
/// Gateway close codes that will never succeed on retry (bad token, invalid or
/// disallowed — including the privileged MESSAGE_CONTENT — intents, invalid
/// shard/version). These surface as Blocked instead of looping forever.
const FATAL_CLOSE_CODES: &[u16] = &[4004, 4010, 4011, 4012, 4013, 4014];
const MESSAGE_LIMIT: usize = 2_000;
const VIEW_CHANNEL: u64 = 1 << 10;
const SEND_MESSAGES: u64 = 1 << 11;
const READ_MESSAGE_HISTORY: u64 = 1 << 16;
const ADMINISTRATOR: u64 = 1 << 3;
const MESSAGE_CONTENT: u64 = 1 << 18;
const MESSAGE_CONTENT_LIMITED: u64 = 1 << 19;

// Gateway intents: the bot receives guild message events plus their content.
const INTENT_GUILD_MESSAGES: u64 = 1 << 9;
const INTENT_MESSAGE_CONTENT: u64 = 1 << 15;

// Gateway opcodes (https://discord.com/developers/docs/topics/gateway-events).
const OP_DISPATCH: u64 = 0;
const OP_HEARTBEAT: u64 = 1;
const OP_IDENTIFY: u64 = 2;
const OP_RESUME: u64 = 6;
const OP_RECONNECT: u64 = 7;
const OP_INVALID_SESSION: u64 = 9;
const OP_HELLO: u64 = 10;
const OP_HEARTBEAT_ACK: u64 = 11;

// Instant-ack reactions (Devin/Cursor pattern): the channel feels responsive
// because pickup is acknowledged in <1s, well before the reply is ready.
const ACK_EMOJI: &str = "👀";
const DONE_EMOJI: &str = "✅";

/// Percent-encode an emoji for a Discord reaction path (every UTF-8 byte).
fn percent_encode_emoji(emoji: &str) -> String {
    emoji.bytes().map(|byte| format!("%{byte:02X}")).collect()
}

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
    #[error("Discord chat binding {guild_id}/{channel_id} already has a live listener here")]
    AlreadyOwned {
        guild_id: String,
        channel_id: String,
    },
    #[error("failed to claim Discord chat binding: {0}")]
    Lease(#[from] std::io::Error),
    #[error("Discord gateway connection failed: {0}")]
    Gateway(String),
    #[error("Discord gateway rejected the connection fatally: {0}")]
    GatewayFatal(String),
}

impl DiscordError {
    fn retryable(&self) -> bool {
        matches!(self, Self::Transport(_))
            || matches!(self, Self::Api { status, .. } if status.is_server_error())
            // A dropped or closed gateway is normal; reconnect and RESUME.
            // `GatewayFatal` (bad token, disallowed intents) is deliberately not
            // here — it must surface as Blocked, not loop forever.
            || matches!(self, Self::Gateway(_))
    }
}

/// Local single-owner authority for one Discord binding: an advisory OS lock on
/// a file keyed by guild+channel. Only the held lock carries authority (the file
/// may outlive the process). This is all the core needs — it prevents a second
/// checkout or store *on this machine* from competing. Which machine runs a
/// Discord-bound Wave at all is the general Wave-placement layer, not this lock.
#[derive(Debug)]
struct DiscordBindingLease {
    _file: File,
}

impl DiscordBindingLease {
    fn acquire(binding: &DiscordChatBinding) -> Result<Self, DiscordError> {
        Self::acquire_at(
            &crate::store::authority_home_dir().join("chat-bindings"),
            binding,
        )
    }

    fn acquire_at(root: &Path, binding: &DiscordChatBinding) -> Result<Self, DiscordError> {
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
    /// Retained so the gateway can authenticate its IDENTIFY. Never logged:
    /// the `Debug` impl omits it and the header carrying it is marked sensitive.
    token: SecretString,
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
            token,
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

    /// PUT/DELETE with no response body (Discord reaction endpoints answer 204).
    /// Shares the rate-limit retry and error mapping; ignores the empty body.
    async fn send_empty(&self, request: reqwest::RequestBuilder) -> Result<(), DiscordError> {
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
            return Ok(());
        }
    }

    async fn put_empty(&self, path: &str) -> Result<(), DiscordError> {
        self.send_empty(self.http.put(format!("{}{path}", self.base_url)))
            .await
    }

    async fn delete(&self, path: &str) -> Result<(), DiscordError> {
        self.send_empty(self.http.delete(format!("{}{path}", self.base_url)))
            .await
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
    /// The gateway to dial for inbound events. Defaults to Discord's; tests
    /// point it at a local websocket fixture.
    gateway_url: String,
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
        token: Option<SecretString>,
    ) -> Result<Self, DiscordError> {
        let lease = DiscordBindingLease::acquire(&binding)?;
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
            gateway_url: GATEWAY_URL.to_string(),
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

    /// Ingest through the Discord gateway (a persistent websocket): messages
    /// arrive as `MESSAGE_CREATE` pushes instead of a 2s poll. Each session
    /// catches up over REST from the journal cursor on connect (the gateway does
    /// not replay history), then streams live events; a dropped connection
    /// reconnects and RESUMEs. Outbound sends still go over REST on a short tick.
    pub async fn run(self, runtime: std::sync::Arc<WaveRuntime>) {
        // `resume` is threaded by `&mut` so a session's freshest identity/seq
        // survives *any* exit — a clean RECONNECT, a zombie, or a transport drop
        // — and the next connection RESUMEs instead of re-IDENTIFYing.
        let mut resume: Option<ResumeState> = None;
        loop {
            match self.run_gateway_session(&runtime, &mut resume).await {
                Ok(Reconnect::Now) => {}
                Ok(Reconnect::AfterBackoff) => tokio::time::sleep(REIDENTIFY_BACKOFF).await,
                Err(error)
                    if error
                        .downcast_ref::<DiscordError>()
                        .is_some_and(DiscordError::retryable) =>
                {
                    tracing::warn!(%error, "Discord gateway dropped; reconnecting");
                    self.health.send_replace(ChatBackingHealth::Retrying {
                        detail: error.to_string(),
                    });
                    tokio::time::sleep(ERROR_BACKOFF).await;
                }
                Err(error) => {
                    tracing::error!(%error, "Discord gateway stopped; correct the binding, token, or intents");
                    self.health.send_replace(ChatBackingHealth::Blocked {
                        detail: error.to_string(),
                    });
                    return;
                }
            }
        }
    }

    /// One gateway connection, from handshake to disconnect. Updates `*resume`
    /// live (READY establishes it; every sequenced frame advances it) so the
    /// caller always holds the freshest resume state however the session ends.
    async fn run_gateway_session(
        &self,
        runtime: &WaveRuntime,
        resume: &mut Option<ResumeState>,
    ) -> Result<Reconnect> {
        let url = resume
            .as_ref()
            .map(|state| state.resume_url.clone())
            .unwrap_or_else(|| self.gateway_url.clone());
        let (stream, _) = tokio_tungstenite::connect_async(&url)
            .await
            .map_err(gateway_error)?;
        let (mut write, mut read) = stream.split();

        let hello = next_frame(&mut read).await?;
        let GatewayEvent::Hello {
            heartbeat_interval_ms,
        } = classify_frame(&hello)?
        else {
            // A handshake that doesn't open with HELLO is a protocol violation,
            // not a fatal config error: reconnect (retryable), don't Block.
            return Err(DiscordError::Gateway("gateway did not open with HELLO".into()).into());
        };

        let resuming = resume.is_some();
        if let Some(state) = resume.as_ref() {
            // Health re-arms to Ready only once the RESUMED frame confirms the
            // resume — sending the payload isn't proof it was accepted (Discord
            // can answer with INVALID_SESSION), so we don't report Ready early.
            send_json(
                &mut write,
                resume_payload(self.client.token.expose_secret(), state),
            )
            .await?;
        } else {
            send_json(
                &mut write,
                identify_payload(self.client.token.expose_secret()),
            )
            .await?;
        }

        let mut seq = resume.as_ref().map(|state| state.seq);
        // A resumed session replays missed dispatches, but a REST catch-up from
        // the cursor is idempotent and closes any gap either way; do it once.
        let mut caught_up = resuming && {
            self.catch_up_inbound(runtime).await?;
            true
        };
        let mut awaiting_ack = false;

        let mut heartbeat = tokio::time::interval(Duration::from_millis(heartbeat_interval_ms));
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        heartbeat.tick().await; // consume the immediate first tick
        let mut outbound = tokio::time::interval(OUTBOUND_TICK);

        loop {
            tokio::select! {
                frame = next_frame(&mut read) => {
                    let frame = frame?;
                    if let Some(s) = frame.s {
                        seq = Some(s);
                        if let Some(state) = resume.as_mut() {
                            state.seq = s;
                        }
                    }
                    match classify_frame(&frame)? {
                        GatewayEvent::Ready { session_id, resume_gateway_url } => {
                            // Discord's resume_gateway_url is a bare host; carry
                            // the same version/encoding query the initial connect used.
                            *resume = Some(ResumeState {
                                session_id,
                                resume_url: format!("{resume_gateway_url}/?v=10&encoding=json"),
                                seq: seq.unwrap_or(0),
                            });
                            if !caught_up {
                                self.catch_up_inbound(runtime).await?;
                                caught_up = true;
                            }
                            self.health.send_replace(ChatBackingHealth::Ready);
                        }
                        GatewayEvent::Resumed => {
                            if !caught_up {
                                self.catch_up_inbound(runtime).await?;
                                caught_up = true;
                            }
                            self.health.send_replace(ChatBackingHealth::Ready);
                        }
                        GatewayEvent::MessageCreate(message) => {
                            if !caught_up {
                                self.catch_up_inbound(runtime).await?;
                                caught_up = true;
                            }
                            if message.channel_id.as_deref() == Some(self.binding.channel_id.as_str()) {
                                let id = message.id.clone();
                                let human_input = self.accept_message(runtime, *message)?;
                                runtime
                                    .try_advance_discord_cursor(&self.binding, id.clone())
                                    .context("journal Discord cursor")?;
                                // Ack a live human message instantly (<1s), long
                                // before the reply is ready — the responsiveness
                                // the channel actually feels.
                                if human_input {
                                    self.react(&id, ACK_EMOJI).await;
                                }
                            }
                        }
                        GatewayEvent::HeartbeatRequest => {
                            send_json(&mut write, heartbeat_payload(seq)).await?;
                            awaiting_ack = true;
                        }
                        GatewayEvent::HeartbeatAck => awaiting_ack = false,
                        // *resume already holds the freshest state; just reconnect.
                        GatewayEvent::Reconnect => return Ok(Reconnect::Now),
                        GatewayEvent::InvalidSession { resumable } => {
                            if !resumable {
                                *resume = None;
                            }
                            return Ok(Reconnect::AfterBackoff);
                        }
                        GatewayEvent::Hello { .. } | GatewayEvent::Ignored => {}
                    }
                }
                _ = heartbeat.tick() => {
                    if awaiting_ack {
                        // A missed ACK means a zombie connection: drop and resume.
                        return Ok(Reconnect::Now);
                    }
                    send_json(&mut write, heartbeat_payload(seq)).await?;
                    awaiting_ack = true;
                }
                _ = outbound.tick() => {
                    self.deliver_pending(runtime).await?;
                }
            }
        }
    }

    /// REST catch-up from the journal cursor to the channel head. Shared by the
    /// gateway's on-connect sync and by [`Self::sync_once`] (the tested path).
    async fn catch_up_inbound(&self, runtime: &WaveRuntime) -> Result<()> {
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
        Ok(())
    }

    /// The REST catch-up + outbound flush as one step. Production drives these
    /// separately through the gateway loop; the tests exercise the shared REST
    /// path through this helper.
    #[cfg(test)]
    async fn sync_once(&self, runtime: &WaveRuntime) -> Result<()> {
        self.catch_up_inbound(runtime).await?;
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

    /// Add a bot reaction to a message. Best-effort: an ack is cosmetic, so a
    /// failure is logged and never breaks ingestion or delivery.
    async fn react(&self, message_id: &str, emoji: &str) {
        let path = format!(
            "/channels/{}/messages/{message_id}/reactions/{}/@me",
            self.binding.channel_id,
            percent_encode_emoji(emoji)
        );
        if let Err(error) = self.client.put_empty(&path).await {
            tracing::warn!(%error, "Discord reaction failed");
        }
    }

    /// Remove a bot reaction. Best-effort (a 404 when it was never there is fine).
    async fn remove_reaction(&self, message_id: &str, emoji: &str) {
        let path = format!(
            "/channels/{}/messages/{message_id}/reactions/{}/@me",
            self.binding.channel_id,
            percent_encode_emoji(emoji)
        );
        if let Err(error) = self.client.delete(&path).await {
            tracing::debug!(%error, "Discord reaction removal failed");
        }
    }

    /// Returns `true` when the message was a human chat input (delivered to the
    /// runtime) — the caller acks live human input with a pickup reaction.
    fn accept_message(&self, runtime: &WaveRuntime, message: Message) -> Result<bool> {
        if !message_in_epoch(&message, &runtime.active_conversation_epoch())
            .context("validate Discord input epoch")?
        {
            return Ok(false);
        }
        if message.author.id == self.bot_user_id {
            if self.reconcile_echo(runtime, &message)? {
                return Ok(false);
            }
            let Some((op, text)) = parse_authored_content(runtime.name(), &message.content) else {
                return Ok(false);
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
            return Ok(false);
        }
        if message.author.bot == Some(true)
            || message.webhook_id.is_some()
            || !matches!(message.kind, 0 | 19)
            || message.content.trim().is_empty()
        {
            return Ok(false);
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
        Ok(true)
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
                .any(|provider_message_id| provider_message_id == &message.id)
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
            let ack_source = reply_to.map(|source| source.message_id.clone());
            let mut posted_any = false;
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
                posted_any = true;
            }
            // Once the reply to a human message is delivered, flip its pickup
            // reaction (👀) to done (✅). Best-effort; the send already landed.
            if posted_any {
                if let Some(source_id) = &ack_source {
                    self.react(source_id, DONE_EMOJI).await;
                    self.remove_reaction(source_id, ACK_EMOJI).await;
                }
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

// ---------------------------------------------------------------------------
// Gateway (persistent websocket ingestion)
// ---------------------------------------------------------------------------

/// Enough to RESUME a dropped gateway session instead of re-IDENTIFYing.
#[derive(Debug, Clone)]
struct ResumeState {
    session_id: String,
    resume_url: String,
    seq: u64,
}

/// How a finished gateway session asks the run loop to come back. The resume
/// state itself lives in the caller's `&mut Option<ResumeState>`, updated live.
#[derive(Debug, PartialEq, Eq)]
enum Reconnect {
    /// Reconnect immediately (RECONNECT opcode, zombie connection).
    Now,
    /// Wait a beat first (INVALID_SESSION — Discord asks for a short delay).
    AfterBackoff,
}

/// One raw gateway frame. `s`/`t` are null on non-dispatch frames.
#[derive(Debug, Deserialize)]
struct GatewayFrame {
    op: u64,
    s: Option<u64>,
    t: Option<String>,
    #[serde(default)]
    d: serde_json::Value,
}

/// A gateway frame classified into the events the listener acts on.
#[derive(Debug)]
enum GatewayEvent {
    Hello {
        heartbeat_interval_ms: u64,
    },
    Ready {
        session_id: String,
        resume_gateway_url: String,
    },
    Resumed,
    MessageCreate(Box<Message>),
    HeartbeatRequest,
    HeartbeatAck,
    Reconnect,
    InvalidSession {
        resumable: bool,
    },
    /// Any dispatch or control frame the listener does not act on.
    Ignored,
}

fn classify_frame(frame: &GatewayFrame) -> Result<GatewayEvent> {
    Ok(match frame.op {
        OP_HELLO => GatewayEvent::Hello {
            heartbeat_interval_ms: frame
                .d
                .get("heartbeat_interval")
                .and_then(serde_json::Value::as_u64)
                // Structural handshake miss → retryable (reconnect), not fatal.
                .ok_or_else(|| DiscordError::Gateway("HELLO missing heartbeat_interval".into()))?,
        },
        OP_HEARTBEAT => GatewayEvent::HeartbeatRequest,
        OP_HEARTBEAT_ACK => GatewayEvent::HeartbeatAck,
        OP_RECONNECT => GatewayEvent::Reconnect,
        OP_INVALID_SESSION => GatewayEvent::InvalidSession {
            resumable: frame.d.as_bool().unwrap_or(false),
        },
        OP_DISPATCH => match frame.t.as_deref() {
            Some("READY") => GatewayEvent::Ready {
                session_id: frame
                    .d
                    .get("session_id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| DiscordError::Gateway("READY missing session_id".into()))?
                    .to_string(),
                resume_gateway_url: frame
                    .d
                    .get("resume_gateway_url")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        DiscordError::Gateway("READY missing resume_gateway_url".into())
                    })?
                    .to_string(),
            },
            Some("RESUMED") => GatewayEvent::Resumed,
            // A malformed message is skipped, never fatal: one odd payload must
            // not kill ingestion for the channel.
            Some("MESSAGE_CREATE") => match serde_json::from_value(frame.d.clone()) {
                Ok(message) => GatewayEvent::MessageCreate(Box::new(message)),
                Err(error) => {
                    tracing::warn!(%error, "skipping malformed Discord MESSAGE_CREATE");
                    GatewayEvent::Ignored
                }
            },
            _ => GatewayEvent::Ignored,
        },
        _ => GatewayEvent::Ignored,
    })
}

fn identify_payload(token: &str) -> serde_json::Value {
    serde_json::json!({
        "op": OP_IDENTIFY,
        "d": {
            "token": token,
            "intents": INTENT_GUILD_MESSAGES | INTENT_MESSAGE_CONTENT,
            "properties": { "os": "linux", "browser": "loopflow", "device": "loopflow" },
        },
    })
}

fn resume_payload(token: &str, state: &ResumeState) -> serde_json::Value {
    serde_json::json!({
        "op": OP_RESUME,
        "d": { "token": token, "session_id": state.session_id, "seq": state.seq },
    })
}

fn heartbeat_payload(seq: Option<u64>) -> serde_json::Value {
    serde_json::json!({ "op": OP_HEARTBEAT, "d": seq })
}

fn gateway_error(error: tokio_tungstenite::tungstenite::Error) -> DiscordError {
    DiscordError::Gateway(error.to_string())
}

/// Read the next JSON frame, skipping websocket control frames. A close or a
/// dropped connection is a retryable [`DiscordError::Gateway`] so the caller
/// reconnects.
async fn next_frame<S>(read: &mut S) -> Result<GatewayFrame>
where
    S: futures_util::Stream<Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
{
    loop {
        match read.next().await {
            Some(Ok(WsMessage::Text(text))) => {
                return serde_json::from_str(text.as_str())
                    .map_err(|error| anyhow!("Discord gateway frame: {error}"));
            }
            // tungstenite queues an automatic Pong that flushes on our next send.
            Some(Ok(WsMessage::Ping(_) | WsMessage::Pong(_) | WsMessage::Frame(_))) => continue,
            Some(Ok(WsMessage::Binary(_))) => continue,
            Some(Ok(WsMessage::Close(frame))) => {
                return Err(close_error(frame.as_ref().map(|frame| u16::from(frame.code))).into());
            }
            None => return Err(DiscordError::Gateway("gateway connection closed".into()).into()),
            Some(Err(error)) => return Err(gateway_error(error).into()),
        }
    }
}

/// Map a gateway close code to a retryable drop or a fatal (Blocked) rejection.
/// Fatal codes (bad token, invalid/disallowed intents) never succeed on retry.
fn close_error(code: Option<u16>) -> DiscordError {
    match code {
        Some(code) if FATAL_CLOSE_CODES.contains(&code) => {
            DiscordError::GatewayFatal(format!("gateway closed with fatal code {code}"))
        }
        Some(code) => DiscordError::Gateway(format!("gateway closed (code {code})")),
        None => DiscordError::Gateway("gateway connection closed".into()),
    }
}

async fn send_json<S>(write: &mut S, value: serde_json::Value) -> Result<()>
where
    S: futures_util::Sink<WsMessage, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    write
        .send(WsMessage::text(value.to_string()))
        .await
        .map_err(gateway_error)?;
    Ok(())
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
    /// Present on gateway MESSAGE_CREATE (events span every channel the bot
    /// sees, so the listener filters on it). Absent on REST reads scoped to one
    /// channel — `None` there means "already the bound channel".
    #[serde(default)]
    channel_id: Option<String>,
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
    use crate::controller::wave::chat::ChatBacking;
    use crate::controller::wave::wire::ResidentDelta;

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
        /// Recorded reaction ops as `"<METHOD> <message_id> <raw path emoji>"`.
        reactions: Vec<String>,
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
                channel_id: None,
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
                    channel_id: Some("channel".into()),
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
            (method @ ("PUT" | "DELETE"), path) if path.contains("/reactions/") => {
                // .../messages/<id>/reactions/<emoji>/@me
                let mut segments = path.split('/').skip_while(|s| *s != "messages").skip(1);
                let message_id = segments.next().unwrap_or_default().to_string();
                let emoji = segments.nth(1).unwrap_or_default().to_string();
                fixture
                    .lock()
                    .expect("fixture")
                    .reactions
                    .push(format!("{method} {message_id} {emoji}"));
                json_response(StatusCode::NO_CONTENT, json!(null))
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
            reactions: Vec::new(),
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

    fn frame(value: serde_json::Value) -> GatewayFrame {
        serde_json::from_value(value).expect("gateway frame")
    }

    #[test]
    fn gateway_frames_classify_into_actionable_events() {
        assert!(matches!(
            classify_frame(&frame(
                json!({"op": 10, "d": {"heartbeat_interval": 41250}})
            ))
            .unwrap(),
            GatewayEvent::Hello {
                heartbeat_interval_ms: 41250
            }
        ));
        assert!(matches!(
            classify_frame(&frame(json!({"op": 11}))).unwrap(),
            GatewayEvent::HeartbeatAck
        ));
        assert!(matches!(
            classify_frame(&frame(json!({"op": 1}))).unwrap(),
            GatewayEvent::HeartbeatRequest
        ));
        assert!(matches!(
            classify_frame(&frame(json!({"op": 7}))).unwrap(),
            GatewayEvent::Reconnect
        ));
        assert!(matches!(
            classify_frame(&frame(json!({"op": 9, "d": true}))).unwrap(),
            GatewayEvent::InvalidSession { resumable: true }
        ));
        assert!(matches!(
            classify_frame(&frame(json!({"op": 9, "d": false}))).unwrap(),
            GatewayEvent::InvalidSession { resumable: false }
        ));

        let ready = classify_frame(&frame(json!({
            "op": 0, "s": 1, "t": "READY",
            "d": {"session_id": "sess", "resume_gateway_url": "wss://resume.discord.gg"}
        })))
        .unwrap();
        let GatewayEvent::Ready {
            session_id,
            resume_gateway_url,
        } = ready
        else {
            panic!("expected READY");
        };
        assert_eq!(session_id, "sess");
        assert_eq!(resume_gateway_url, "wss://resume.discord.gg");

        let created = classify_frame(&frame(json!({
            "op": 0, "s": 2, "t": "MESSAGE_CREATE",
            "d": {"id": "42", "channel_id": "chan", "type": 0,
                  "content": "hi", "author": {"id": "human"}}
        })))
        .unwrap();
        let GatewayEvent::MessageCreate(message) = created else {
            panic!("expected MESSAGE_CREATE");
        };
        assert_eq!(message.id, "42");
        assert_eq!(message.channel_id.as_deref(), Some("chan"));

        // A RESUMED dispatch re-arms health (see run loop); classified distinctly.
        assert!(matches!(
            classify_frame(&frame(json!({"op": 0, "s": 3, "t": "RESUMED", "d": {}}))).unwrap(),
            GatewayEvent::Resumed
        ));
        // An unhandled dispatch (e.g. TYPING_START) is ignored, not an error.
        assert!(matches!(
            classify_frame(&frame(
                json!({"op": 0, "s": 3, "t": "TYPING_START", "d": {}})
            ))
            .unwrap(),
            GatewayEvent::Ignored
        ));
        // A malformed MESSAGE_CREATE is skipped, never fatal.
        assert!(matches!(
            classify_frame(&frame(
                json!({"op": 0, "s": 4, "t": "MESSAGE_CREATE", "d": {"garbage": true}})
            ))
            .unwrap(),
            GatewayEvent::Ignored
        ));
        // A structural HELLO miss is retryable (a DiscordError), not a fatal panic.
        let hello_miss = classify_frame(&frame(json!({"op": 10, "d": {}}))).unwrap_err();
        assert!(hello_miss
            .downcast_ref::<DiscordError>()
            .is_some_and(DiscordError::retryable));
    }

    #[test]
    fn fatal_close_codes_block_while_transient_ones_retry() {
        // Disallowed intents (4014) / bad token (4004) never succeed on retry.
        assert!(!close_error(Some(4014)).retryable());
        assert!(!close_error(Some(4004)).retryable());
        // An ordinary drop reconnects.
        assert!(close_error(Some(1006)).retryable());
        assert!(close_error(None).retryable());
    }

    #[test]
    fn discord_chat_binding_has_one_local_listener() {
        let temp = tempfile::tempdir().expect("tempdir");

        let first = DiscordBindingLease::acquire_at(temp.path(), &binding())
            .expect("first listener claims binding");
        assert!(matches!(
            DiscordBindingLease::acquire_at(temp.path(), &binding()),
            Err(DiscordError::AlreadyOwned { .. })
        ));
        drop(first);
        DiscordBindingLease::acquire_at(temp.path(), &binding())
            .expect("binding is released with its listener");
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
            crate::controller::wave::chat::ChatBacking::discord(&binding()),
        )
        .expect("runtime");
        adapter.attach(&runtime).expect("attach at head");
        adapter.sync_once(&runtime).await.expect("initial sync");
        assert!(
            runtime.read_channel(None).is_empty(),
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
        let observed = runtime.read_channel(None);
        assert_eq!(observed.len(), 105);
        assert!(observed[0]
            .content
            .ends_with(&format!("message {first_id}")));
        assert!(observed[104]
            .content
            .ends_with(&format!("message {newest_id}")));
        adapter.sync_once(&runtime).await.expect("duplicate fetch");
        assert_eq!(runtime.read_channel(None).len(), 105);
        let reply_to = runtime
            .unanswered_chat_tail()
            .into_iter()
            .next()
            .expect("newest chat trigger")
            .id
            .0;

        runtime.apply_resident_delta(ResidentDelta::TurnOpened {
            answers: Vec::new(),
        });
        runtime.apply_resident_delta(ResidentDelta::TurnReplyTo {
            message_id: reply_to,
        });
        runtime.apply_resident_delta(ResidentDelta::TurnText {
            text: "x".repeat(2_001),
        });
        runtime.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            reason: None,
        });
        assert_eq!(
            runtime.discord_snapshot().deliveries[0]
                .reply_to()
                .map(|source| source.message_id.as_str()),
            Some(newest_id.as_str()),
            "the explicit channel relation names the reply target"
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
            crate::controller::wave::chat::ChatBacking::discord(&binding()),
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

    #[test]
    fn discord_chat_reconciliation_does_not_reuse_a_confirmed_echo() {
        let temp = tempfile::tempdir().expect("tempdir");
        let runtime = WaveRuntime::open_with_backing(
            "ship".into(),
            temp.path().to_path_buf(),
            crate::controller::wave::chat::ChatBacking::discord(&binding()),
        )
        .expect("runtime");
        runtime
            .try_attach_discord(binding(), "bot".into(), None)
            .expect("attach");
        runtime
            .try_deliver_discord(
                "question".into(),
                DiscordMessageSource {
                    binding: binding(),
                    message_id: "101".into(),
                    author_id: "human".into(),
                },
            )
            .expect("deliver");
        runtime.apply_resident_delta(ResidentDelta::TurnOpened {
            answers: Vec::new(),
        });
        let reply_to = runtime
            .unanswered_chat_tail()
            .into_iter()
            .next()
            .expect("chat trigger")
            .id
            .0;
        runtime.apply_resident_delta(ResidentDelta::TurnReplyTo {
            message_id: reply_to,
        });
        runtime.apply_resident_delta(ResidentDelta::TurnText {
            text: "x".repeat(4_000),
        });
        runtime.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            reason: None,
        });
        let delivery = runtime.discord_snapshot().deliveries[0].clone();
        assert_eq!(delivery.parts.len(), 2);
        assert_eq!(delivery.parts[0].content, delivery.parts[1].content);
        runtime
            .try_confirm_discord_part(
                &delivery.delivery_id,
                &delivery.parts[0].part_id,
                "provider-1".into(),
            )
            .expect("confirm first part");

        let client = DiscordClient::from_token(Some("fixture-token".into()), "http://unused")
            .expect("fixture client");
        let (health, _) = watch::channel(ChatBackingHealth::Ready);
        let adapter = DiscordAdapter {
            client,
            binding: binding(),
            bot_user_id: "bot".into(),
            initial_head: None,
            health,
            gateway_url: GATEWAY_URL.to_string(),
            _lease: None,
        };
        let echo = |id: &str| Message {
            id: id.into(),
            author: User {
                id: "bot".into(),
                bot: Some(true),
            },
            content: delivery.parts[1].content.clone(),
            kind: 0,
            webhook_id: None,
            message_reference: Some(ReturnedMessageReference {
                message_id: Some("101".into()),
            }),
            channel_id: Some("channel".into()),
        };

        adapter
            .reconcile_echo(&runtime, &echo("provider-1"))
            .expect("ignore already confirmed echo");
        assert_eq!(runtime.discord_snapshot().deliveries[0].confirmed.len(), 1);
        adapter
            .reconcile_echo(&runtime, &echo("provider-2"))
            .expect("confirm second echo");
        let resumed = &runtime.discord_snapshot().deliveries[0];
        assert_eq!(
            resumed.confirmed.get(&delivery.parts[1].part_id),
            Some(&"provider-2".to_string())
        );
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
            crate::controller::wave::chat::ChatBacking::discord(&binding()),
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
                channel_id: None,
            },
        ]);

        adapter
            .sync_once(&runtime)
            .await
            .expect("ingest provider head");
        assert_eq!(
            runtime.read_channel(None).len(),
            2,
            "normal and reply messages both reach the channel"
        );
        adapter
            .sync_once(&runtime)
            .await
            .expect("repeat sync stays idempotent");
        assert_eq!(
            runtime.read_channel(None).len(),
            2,
            "provider messages reach the channel exactly once"
        );
        let journal_before = crate::controller::wave::journal::read_events(
            &crate::controller::wave::journal::journal_path(temp.path(), "ship"),
        );
        let epoch = runtime.active_conversation_epoch();
        let messages = projection
            .history(&runtime, &epoch, Some(12))
            .await
            .expect("project provider history");
        let journal_after = crate::controller::wave::journal::read_events(
            &crate::controller::wave::journal::journal_path(temp.path(), "ship"),
        );

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

        runtime.apply_resident_delta(ResidentDelta::TurnOpened {
            answers: Vec::new(),
        });
        runtime.apply_resident_delta(ResidentDelta::TurnText {
            text: "provider-committed answer".into(),
        });
        runtime.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
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
        let app = crate::controller::wave::server::router_with_chat_projection(
            runtime.clone(),
            crate::controller::wave::server::ResidentDoor::new("resident"),
            Arc::new(crate::controller::wave::registry::ObserverSlot::new(
                runtime.clone(),
                None,
            )),
            None,
            crate::controller::wave::server::ShutdownDoor::new(),
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
        let posted: crate::controller::wave::chat::PostMessageResponse =
            response.json().await.expect("posted response");
        let posted = posted.message.expect("provider-backed message");
        assert_eq!(posted.turn.role, ChatRole::User);
        assert_eq!(posted.turn.text, "favor reliability");
        assert!(matches!(&posted.source, ChatMessageSource::Discord { .. }));
        assert_eq!(
            fixture.lock().expect("fixture").messages[0].content,
            "**[ship · Loopflow app · steer]**\nfavor reliability"
        );
        let retried: crate::controller::wave::chat::PostMessageResponse = client
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
        let observed = runtime.read_channel(None);
        assert_eq!(observed.len(), 1);
        assert!(observed[0].content.ends_with("favor reliability"));
        adapter
            .sync_once(&runtime)
            .await
            .expect("repeat sync stays idempotent");
        assert_eq!(runtime.read_channel(None).len(), 1);
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
            runtime.read_channel(None).is_empty(),
            "a provider message before the durable epoch is not channel input"
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
        assert_eq!(runtime.read_channel(None).len(), 1);
        let messages = projection
            .history(&runtime, &runtime.active_conversation_epoch(), Some(12))
            .await
            .expect("project active history");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].turn.id, format!("discord-{inside_epoch}"));
        server.abort();
    }

    #[tokio::test]
    async fn a_delivered_reply_flips_the_pickup_reaction_to_done() {
        let fixture = fixture(Vec::new());
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
            ChatBacking::discord(&binding()),
        )
        .expect("runtime");
        adapter.attach(&runtime).expect("attach");

        // The pickup ack the gateway arm posts on a live human message.
        adapter.react("101", ACK_EMOJI).await;

        // A human message is answered by the resident, then delivered.
        runtime
            .try_deliver_discord(
                "question".into(),
                DiscordMessageSource {
                    binding: binding(),
                    message_id: "101".into(),
                    author_id: "human".into(),
                },
            )
            .expect("deliver human message");
        runtime.apply_resident_delta(ResidentDelta::TurnOpened {
            answers: Vec::new(),
        });
        let reply_to = runtime
            .unanswered_chat_tail()
            .into_iter()
            .next()
            .expect("chat trigger")
            .id
            .0;
        runtime.apply_resident_delta(ResidentDelta::TurnReplyTo {
            message_id: reply_to,
        });
        runtime.apply_resident_delta(ResidentDelta::TurnText {
            text: "here you go".into(),
        });
        runtime.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            reason: None,
        });
        adapter
            .deliver_pending(&runtime)
            .await
            .expect("deliver reply");

        let reactions = fixture.lock().expect("fixture").reactions.clone();
        let ack = percent_encode_emoji(ACK_EMOJI);
        let done = percent_encode_emoji(DONE_EMOJI);
        assert!(
            reactions.contains(&format!("PUT 101 {ack}")),
            "pickup ack on the human message: {reactions:?}"
        );
        assert!(
            reactions.contains(&format!("PUT 101 {done}")),
            "done ack once the reply is delivered: {reactions:?}"
        );
        assert!(
            reactions.contains(&format!("DELETE 101 {ack}")),
            "pickup ack cleared after done: {reactions:?}"
        );
        server.abort();
    }

    /// A local websocket gateway fixture that closes each connection with
    /// `close_code`. `None` (or a transient code) is a retryable drop → Retrying;
    /// a fatal code (e.g. 4014 disallowed intents) → Blocked.
    async fn ws_gateway_fixture(close_code: Option<u16>) -> (String, tokio::task::JoinHandle<()>) {
        use tokio_tungstenite::tungstenite::protocol::CloseFrame;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ws fixture");
        let address = listener.local_addr().expect("ws fixture address");
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await else {
                    continue;
                };
                let frame = close_code.map(|code| CloseFrame {
                    code: code.into(),
                    reason: "".into(),
                });
                let _ = ws.send(WsMessage::Close(frame)).await;
            }
        });
        (format!("ws://{address}"), task)
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

        // A gateway that drops the connection is retryable → Retrying.
        let (retry_url, retry_server) = ws_gateway_fixture(None).await;
        let (base_url, rest_server) = fixture_server(fixture(Vec::new())).await;
        let client = DiscordClient::from_token(Some("fixture-token".into()), &base_url)
            .expect("fixture client");
        let mut adapter = DiscordAdapter::preflight_with_client(client, binding())
            .await
            .expect("preflight");
        adapter.gateway_url = retry_url;
        let projection = adapter.projection();
        let retry_temp = tempfile::tempdir().expect("tempdir");
        let runtime = WaveRuntime::open_with_backing(
            "ship".into(),
            retry_temp.path().to_path_buf(),
            ChatBacking::discord(&binding()),
        )
        .expect("runtime");
        adapter.attach(&runtime).expect("attach");
        let retry_task = tokio::spawn(adapter.run(runtime));
        wait_for(&projection, |health| {
            matches!(health, ChatBackingHealth::Retrying { .. })
        })
        .await;
        retry_task.abort();
        retry_server.abort();
        rest_server.abort();

        // A fatal close code (4014 disallowed intents) is not retryable → Blocked,
        // and `run` returns instead of looping.
        let (blocked_url, blocked_server) = ws_gateway_fixture(Some(4014)).await;
        let (base_url, rest_server) = fixture_server(fixture(Vec::new())).await;
        let client = DiscordClient::from_token(Some("fixture-token".into()), &base_url)
            .expect("fixture client");
        let mut adapter = DiscordAdapter::preflight_with_client(client, binding())
            .await
            .expect("preflight");
        adapter.gateway_url = blocked_url;
        let projection = adapter.projection();
        let blocked_temp = tempfile::tempdir().expect("tempdir");
        let runtime = WaveRuntime::open_with_backing(
            "ship".into(),
            blocked_temp.path().to_path_buf(),
            ChatBacking::discord(&binding()),
        )
        .expect("runtime");
        adapter.attach(&runtime).expect("attach");
        adapter.run(runtime).await;
        assert!(matches!(
            projection.health(),
            ChatBackingHealth::Blocked { .. }
        ));
        blocked_server.abort();
        rest_server.abort();
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
