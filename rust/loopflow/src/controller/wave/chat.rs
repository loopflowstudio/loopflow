//! Public Wave Chat identity and provenance.
//!
//! A Wave has exactly one active conversation backing. The backing lives on
//! an append-only epoch so clients never need to infer destination from
//! configuration or from individual messages.

use serde::{Deserialize, Serialize};

use crate::chat::turns::ChatTurn;
use crate::controller::wave::journal::DiscordChatBinding;

/// A product action attached to chat state or a rejected write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatAction {
    OpenDiscord { label: String, url: String },
}

impl ChatAction {
    fn open_discord(binding: &DiscordChatBinding) -> Self {
        Self::OpenDiscord {
            label: "Open in Discord".to_string(),
            url: binding.channel_url(),
        }
    }
}

/// The closed set of authorities that may own a Wave conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatBacking {
    Local,
    Discord {
        guild_id: String,
        channel_id: String,
        open: ChatAction,
    },
}

impl ChatBacking {
    pub fn discord(binding: &DiscordChatBinding) -> Self {
        Self::Discord {
            guild_id: binding.guild_id.clone(),
            channel_id: binding.channel_id.clone(),
            open: ChatAction::open_discord(binding),
        }
    }

    pub fn discord_binding(&self) -> Option<DiscordChatBinding> {
        match self {
            Self::Local => None,
            Self::Discord {
                guild_id,
                channel_id,
                ..
            } => Some(DiscordChatBinding {
                guild_id: guild_id.clone(),
                channel_id: channel_id.clone(),
            }),
        }
    }
}

/// One immutable interval during which a single backing owns Wave Chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationEpoch {
    pub id: String,
    pub number: u64,
    pub backing: ChatBacking,
    /// Journal boundary establishing this epoch. Provider projections also
    /// use its timestamp; the sequence keeps local history slices exact.
    pub journal_seq: u64,
    pub started_at: String,
    pub ended_at: Option<String>,
}

/// Durable identity of the authority that committed one rendered message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatMessageSource {
    Local {
        journal_seq: u64,
    },
    Discord {
        guild_id: String,
        channel_id: String,
        message_id: String,
        author_id: String,
        url: String,
    },
}

/// Product chat unit: a turn-shaped message with explicit epoch and source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaveChatMessage {
    pub epoch_id: String,
    pub source: ChatMessageSource,
    pub turn: ChatTurn,
}

/// Availability of the active backing, separate from loop state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ChatBackingHealth {
    Ready,
    Retrying { detail: String },
    Blocked { detail: String },
}

/// Whether the selected epoch's authority could be read for this snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatHistoryState {
    Available,
    Missing,
    Partial,
    Unavailable,
}

/// Stable Wave Chat history envelope consumed by CLI and app surfaces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatHistorySnapshot {
    pub epochs: Vec<ConversationEpoch>,
    pub selected_epoch_id: Option<String>,
    pub state: ChatHistoryState,
    pub detail: Option<String>,
    pub messages: Vec<WaveChatMessage>,
    pub truncated: bool,
}

/// Successful `POST /messages` response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostMessageResponse {
    pub message: Option<WaveChatMessage>,
    pub state: String,
    pub epoch: ConversationEpoch,
}

/// Rejected `POST /messages` response. The active epoch owns any honest
/// alternative write action; the rejection does not duplicate it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostMessageErrorResponse {
    pub error: String,
    pub epoch: ConversationEpoch,
}
