//! What Claude Code reports in both its live stream and its saved transcript.

use loopflow_model::chat::types::TurnUsage;

/// Claude's token fields, shared by live stream and saved transcript decoding.
pub fn usage(value: &serde_json::Value) -> TurnUsage {
    let tokens = |key: &str| value.get(key).and_then(serde_json::Value::as_u64);
    let input_tokens = tokens("input_tokens");
    let cache_read_tokens = tokens("cache_read_input_tokens");
    let cache_write_tokens = tokens("cache_creation_input_tokens");
    TurnUsage {
        input_tokens,
        total_input_tokens: input_tokens
            .map(|input| input + cache_read_tokens.unwrap_or(0) + cache_write_tokens.unwrap_or(0)),
        cache_read_tokens,
        cache_write_tokens,
        output_tokens: tokens("output_tokens"),
        reasoning_tokens: tokens("reasoning_tokens"),
        ..TurnUsage::default()
    }
}
