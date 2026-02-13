# First Commit Spec: Chat Harness Foundation Contract

## What to build

Add a shared chat harness contract (types + validation + tests) that locks message phases, turn completion rules, and event shapes before runtime implementation.

## User intent (verbatim anchors)

> "I would like there to always be a final message associated with termination, rather than just no tool use"

> "progress messages are just a tool call from teh perspective of the model"

> "we think of it more as explicit messages instead of status"

> "memory is persistnet across turns. file system is not"

> "history bounding has to be token-based"

## Data structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserMessagePhase {
    Progress,
    Final,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageArgs {
    pub content: String,
    pub phase: UserMessagePhase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub branch: String,
    pub head_sha_at_start: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTurnRequest {
    pub wave_id: String,
    pub content: String,
    pub token_history_budget: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTurnResult {
    pub id: String,
    pub response: String,
    pub final_message_seen: bool,
    pub memory_edits: Vec<MemoryEditLog>,
    pub tool_calls: Vec<ToolCallLog>,
    pub context: ContextSnapshot,
    pub snapshot: WorkspaceSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    Message {
        content: String,
        phase: UserMessagePhase, // progress | final
    },
    ToolCall {
        tool: String,
        args: serde_json::Value,
    },
    ToolResult {
        tool: String,
        summary: String,
    },
    MemoryEdit {
        op: String,
        block: String,
        detail: String,
    },
    Done {
        context: ContextSnapshot,
    },
    Failed {
        code: String,
        message: String,
    },
}
```

Validation-only helper types for this commit:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionError {
    MissingFinalMessage,
    MultipleFinalMessages,
}
```

## Key functions

```rust
/// Validate that a successful turn emitted exactly one final message.
pub fn validate_turn_completion(events: &[AgentEvent]) -> Result<(), CompletionError>;

/// Parse and validate send_message tool args from raw JSON.
pub fn parse_send_message_args(raw: &str) -> anyhow::Result<SendMessageArgs>;

/// Return true when the event is a user-visible message.
pub fn is_user_message(event: &AgentEvent) -> bool;

/// Compute final-message count from event stream.
pub fn final_message_count(events: &[AgentEvent]) -> usize;
```

Suggested module layout for this commit:

```text
rust/loopflow/src/chat/
  mod.rs
  contract.rs          # request/response/event structs
  completion.rs        # completion validation helpers
  contract_test.rs     # serde + validation tests
```

## Constraints

- This commit is contract-first only; no model HTTP calls, no process spawning, no endpoints yet.
- `send_message` phases are explicit, not inferred from wording.
- Successful turn completion requires exactly one `final` message.
- Progress messages are allowed at cardinality `[0, infinity)`.
- Keep history policy token-based in type/API shape (`token_history_budget`), even before persistence implementation.
- Keep durability boundary visible in naming/docs:
  - memory durable across turns
  - filesystem side effects ephemeral by default (implemented later)

Non-goals in this commit:

- Implementing `lf-agent`
- Adding SQLite migrations
- Streaming transport wiring
- Compaction algorithm

## Done when

Run:

```bash
cargo test -p loopflow chat::contract
```

Expected:

- all new contract/completion tests pass
- tests cover:
  - one final message => success
  - no final message => `MissingFinalMessage`
  - two finals => `MultipleFinalMessages`
  - serde round-trip for `AgentEvent::Message { phase }`

Optional smoke check:

```bash
cargo test -p loopflow chat::completion -- --nocapture
```

Expected: clear assertion output for completion rule edge cases.
