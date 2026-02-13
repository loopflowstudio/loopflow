use serde::{Deserialize, Serialize};
use std::time::Duration;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250929";
const DEFAULT_MAX_TOKENS: u32 = 4096;

// --- Request types ---

#[derive(Debug, Serialize)]
pub struct Request {
    pub model: String,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

// --- Response types ---

#[derive(Debug, Deserialize)]
pub struct Response {
    pub id: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: String,
    pub usage: Usage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// --- API errors ---

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("ANTHROPIC_API_KEY not set")]
    MissingApiKey,
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("network error: {0}")]
    Network(String),
    #[error("failed to parse response: {0}")]
    Parse(String),
}

// --- Client ---

pub fn default_request(
    messages: Vec<Message>,
    tools: Vec<ToolDefinition>,
    system: Option<String>,
) -> Request {
    Request {
        model: DEFAULT_MODEL.to_string(),
        max_tokens: DEFAULT_MAX_TOKENS,
        system,
        messages,
        tools,
    }
}

pub async fn call(request: &Request) -> Result<Response, ApiError> {
    let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| ApiError::MissingApiKey)?;

    let client = reqwest::Client::new();
    let http_response = client
        .post(API_URL)
        .header("x-api-key", &api_key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json")
        .json(request)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;

    let status = http_response.status().as_u16();
    if status >= 400 {
        let body = http_response
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        return Err(ApiError::Http { status, body });
    }

    http_response
        .json::<Response>()
        .await
        .map_err(|e| ApiError::Parse(e.to_string()))
}
