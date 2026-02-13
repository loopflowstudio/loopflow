# Chat: Memory-First Agent for Waves

A memory-first agent harness for waves. The memory buffer is the primary artifact — visible, editable, persistent. The LLM APIs are message-based, not chat-oriented. We build a memory-centric interface on top: the data model is memory blocks and tool calls, chat is just the UX.

Each chat message spawns an agent process the same way a step does — same executor/container model. But instead of shelling out to `claude` or `codex`, we run our own Rust agent harness.

## What to build

A Rust agent binary + lfd API surface + Python CLI client.

## Design decisions captured in conversation

> "there is no  'chat' mode to the agent harness. HOWEVER, there is then a separate chat app that sits on top of it"

> "I would like there to always be a final message associated with termination, rather than just no tool use"

> "we think of it more as explicit messages instead of status"

- Prompt input default is memory blocks + current user message + bounded harness history (with compaction).
- Harness history remains secondary to memory, but is included for reasoning continuity.
- History bounding is token-based (not turn-count based).
- Turn termination requires a final user-visible message event; "no tool calls" alone is not a valid completion condition.
- "At some point" is not enough; every successful turn must end with exactly one terminal/final message.
- Per-turn user messages are explicit agent-authored messages: `0..∞` progress messages are allowed, followed by exactly one required final message.
- Progress messages are streamed live to the chat UI as they are emitted.
- Memory edits are auto-applied and persisted on each memory tool call (not required per turn).

When you send a chat message to a wave, lfd spawns a Rust agent process. The process:
1. Resolves wave workspace snapshot (branch + HEAD SHA) at turn start
2. Loads current memory blocks + bounded harness history from lfd (HTTP)
3. Builds a prompt (system + memory + token-bounded history + current user message)
4. Calls the LLM API directly (Anthropic first, OpenAI/Gemini later)
5. Dispatches tool calls (memory edits, file ops, shell, send_message)
6. Loops until completion contract is met (final `send_message` + no pending follow-up tool results)
7. Persists updated memory + message log back to lfd
8. Streams events to lfd during execution
9. Exits

## Architecture

```
Python client (local testing) / Swift client + UI (product surface)
        │
        ▼  HTTP
lfd  /v0/waves/:id/chat/*     ← API + persistence + process lifecycle (Rust)
        │
        ▼  spawns process, intermediates messages
lf-agent                       ← Rust binary, agent loop
        │
        ├── Model trait (Anthropic first, OpenAI/Gemini later)
        ├── Tool dispatch (ToolHandler trait + ToolRegistry)
        ├── Memory blocks (loaded from / written back to lfd)
        ├── Agent tools (wave-scoped: files, shell)
        └── Events (streamed back to lfd via stdout JSON lines)
```

Chat is always part of a wave. The wave provides the sandbox (repo, worktree, area).
Chat execution is independent of wave step runs: chat turns run in their own executor lane (container or process, matching executor type).

### Inspired by

- **Codex CLI** (Rust): SQ/EQ channel pattern, two-level agent loop, ToolHandler trait + registry, ContextManager as Vec<Item>. But tightly coupled to OpenAI — no model trait. We add provider abstraction.
- **OpenCode** (TypeScript): Vercel AI SDK for provider abstraction, named agents with permission presets, session compaction.
- **MemGPT/Letta**: Memory as tool calls, core memory in system prompt, send_message pattern, memory_replace/insert/rethink operations.
- **pydantic-ai** (Python): 3-node agent loop (UserPrompt → ModelRequest → CallTools → loop/end), Model trait with request()/stream(), structured output via tool-based schema.

We study all four but depend on none.

## Compaction strategy (MemGPT/Letta-informed)

Compaction is about reducing prompt tokens while preserving durable working context.

### Invariants to preserve

- User preferences and standing instructions
- Stable project facts and decisions
- Active plans and unresolved tasks
- Recent turn outcomes that changed memory or execution intent

### Compaction flow

1. Compute current token load for memory + bounded history.
2. If above threshold, run compaction pass:
   - summarize bounded history into structured "recent context" memory blocks
   - deduplicate overlapping memory lines
   - preserve unresolved tasks as explicit checklist entries
3. Keep original message log for observability, but feed only compacted subset into future prompts.
4. Emit a compaction event with before/after token counts.

### Design intent

- Prefer targeted `memory_replace` updates for local changes.
- Use `memory_rethink` for full block reorganization.
- Keep memory human-editable and transparent even after compaction.

## Rust Agent Harness: Core Traits

### Model trait

```rust
/// Provider-agnostic model interface. Anthropic first, others later.
/// Inspired by pydantic-ai's Model and rig-core's CompletionModel,
/// but thinner — just two methods.
#[async_trait]
pub trait Model: Send + Sync {
    async fn request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system: &str,
    ) -> Result<ModelResponse>;

    // Streaming deferred to v2.
    // async fn stream(...) -> Result<impl Stream<Item = StreamEvent>>;
}

/// What comes back from one model call.
#[derive(Debug)]
pub struct ModelResponse {
    pub content: Option<String>,       // assistant text (inner reasoning)
    pub tool_calls: Vec<ToolCallRequest>,
    pub usage: Usage,
}

#[derive(Debug)]
pub struct ToolCallRequest {
    pub id: String,
    pub name: String,
    pub arguments: String,  // JSON string
}

#[derive(Debug, Default)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}
```

Anthropic implementation is ~300 lines: build the Messages API request with `reqwest`, parse the response, extract tool_use blocks. No SDK dependency — just reqwest + serde against the documented API.

### ToolDefinition

```rust
/// Sent to the LLM so it knows what tools are available.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,  // JSON Schema object
}
```

### ToolHandler trait

```rust
/// Inspired by Codex's ToolHandler. Each tool implements this.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    fn name(&self) -> &str;
    fn definition(&self) -> ToolDefinition;

    async fn execute(
        &self,
        arguments: &str,        // raw JSON string from the model
        ctx: &ToolContext,      // wave worktree, wave config, etc.
    ) -> Result<ToolOutput>;
}

pub struct ToolContext {
    pub worktree: PathBuf,
    pub wave_id: String,
    pub branch: String,
    pub head_sha_at_start: String,
}

pub enum ToolOutput {
    /// Tool produced a result to feed back to the model.
    Result(String),
    /// Tool is send_message — this is the user-visible response.
    UserMessage {
        content: String,
        phase: UserMessagePhase, // progress | final
    },
    /// Tool modified memory — return confirmation, memory is updated in-place.
    MemoryEdited(String),
}

pub enum UserMessagePhase {
    Progress,
    Final,
}

// send_message tool payload (model-facing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageArgs {
    pub content: String,
    pub phase: UserMessagePhase, // "progress" | "final"
}
```

### ToolRegistry

```rust
/// Codex pattern: HashMap<String, Arc<dyn ToolHandler>>.
pub struct ToolRegistry {
    handlers: HashMap<String, Arc<dyn ToolHandler>>,
}

impl ToolRegistry {
    pub fn register(&mut self, handler: Arc<dyn ToolHandler>) {
        self.handlers.insert(handler.name().to_string(), handler);
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.handlers.values().map(|h| h.definition()).collect()
    }

    pub async fn dispatch(
        &self,
        call: &ToolCallRequest,
        ctx: &ToolContext,
    ) -> Result<ToolOutput> {
        let handler = self.handlers.get(&call.name)
            .ok_or_else(|| anyhow!("unknown tool: {}", call.name))?;
        handler.execute(&call.arguments, ctx).await
    }
}
```

### Message types

```rust
/// Conversation history. Simpler than Codex's ResponseItem —
/// we don't need OpenAI-specific variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "user")]
    User { content: String },

    #[serde(rename = "assistant")]
    Assistant {
        content: Option<String>,
        #[serde(default)]
        tool_calls: Vec<ToolCallRequest>,
    },

    #[serde(rename = "tool")]
    ToolResult {
        tool_call_id: String,
        content: String,
    },
}
```

### Memory

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBlock {
    pub label: String,
    pub content: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub blocks: Vec<MemoryBlock>,
}

impl Memory {
    pub fn render(&self) -> String {
        // Render blocks with labels and line numbers for the system prompt
    }

    pub fn apply_replace(&mut self, block: &str, old: &str, new: &str) -> Result<String> { .. }
    pub fn apply_insert(&mut self, block: &str, content: &str) -> Result<String> { .. }
    pub fn apply_rethink(&mut self, block: &str, new_content: &str) -> Result<String> { .. }
    pub fn apply_delete(&mut self, block: &str) -> Result<String> { .. }
}
```

## Agent loop

Two-level loop inspired by Codex. Outer loop handles compaction. Inner loop streams one model response and dispatches tools.
`history` in this loop starts with token-bounded cross-turn harness history and then appends current-turn assistant/tool exchanges.

```rust
pub async fn run_turn(
    model: &dyn Model,
    memory: &mut Memory,
    history: &mut Vec<Message>,
    user_message: &str,
    tools: &ToolRegistry,
    ctx: &ToolContext,
) -> Result<TurnResult> {
    history.push(Message::User { content: user_message.to_string() });

    let mut user_messages = Vec::new();
    let mut has_final_message = false;
    let mut memory_edits = Vec::new();
    let mut tool_call_log = Vec::new();

    loop {
        // Build system prompt with current memory
        let system = build_system_prompt(memory);
        let tool_defs = tools.definitions();

        // Call model
        let response = model.request(history, &tool_defs, &system).await?;

        // Record assistant message in history
        history.push(Message::Assistant {
            content: response.content.clone(),
            tool_calls: response.tool_calls.clone(),
        });

        if response.tool_calls.is_empty() {
            // No tool calls is not sufficient for completion.
            // Every successful turn must include a terminal send_message.
            if has_final_message {
                break;
            }
            return Err(anyhow!("missing_final_message"));
        }

        let mut needs_follow_up = false;

        for call in &response.tool_calls {
            let output = tools.dispatch(call, ctx).await?;
            tool_call_log.push(call.clone());

            match &output {
                ToolOutput::UserMessage { content, phase } => {
                    user_messages.push(content.clone());
                    if matches!(phase, UserMessagePhase::Final) {
                        has_final_message = true;
                    }
                }
                ToolOutput::MemoryEdited(confirmation) => {
                    memory_edits.push(call.clone());
                }
                ToolOutput::Result(_) => {
                    needs_follow_up = true;
                }
            }

            // Feed result back to model
            let result_text = match output {
                ToolOutput::Result(s) => s,
                ToolOutput::UserMessage { .. } => "Message sent.".to_string(),
                ToolOutput::MemoryEdited(s) => s,
            };
            history.push(Message::ToolResult {
                tool_call_id: call.id.clone(),
                content: result_text,
            });
        }

        // If only memory edits and send_message — no need for follow-up.
        // If agent tools (read_file, shell) were called — model needs results.
        if !needs_follow_up && has_final_message {
            break;
        }
    }

    Ok(TurnResult {
        response: user_messages.join("\n"),
        memory_edits,
        tool_calls: tool_call_log,
        context: ContextSnapshot {
            memory_tokens: estimate_tokens(&build_system_prompt(memory)),
            history_tokens: estimate_tokens_messages(history),
            total_tokens: 0, // filled from usage
        },
    })
}
```

The loop terminates when:
1. A terminal/final send_message has been emitted, AND
2. There are no pending tool results that require model follow-up.

Agent tools (read_file, shell, write_file) set `needs_follow_up = true` because the model needs to see the result before responding.

## Concrete tool implementations

```rust
// Memory tools — operate on the in-process Memory struct
pub struct SendMessageTool;
pub struct MemoryReplaceTool { memory: Arc<Mutex<Memory>> }
pub struct MemoryInsertTool { memory: Arc<Mutex<Memory>> }
pub struct MemoryRethinkTool { memory: Arc<Mutex<Memory>> }
pub struct MemoryDeleteTool { memory: Arc<Mutex<Memory>> }

// Agent tools — wave-scoped, operate on the worktree
pub struct ReadFileTool;
pub struct WriteFileTool;
pub struct ShellTool;
```

Each implements `ToolHandler`. Memory tools mutate the shared `Memory` and return `ToolOutput::MemoryEdited`. Agent tools return `ToolOutput::Result`. SendMessage returns `ToolOutput::UserMessage`.

## Anthropic Model implementation

```rust
pub struct AnthropicModel {
    api_key: String,
    model: String,      // "claude-sonnet-4-5-20250929"
    client: reqwest::Client,
}

#[async_trait]
impl Model for AnthropicModel {
    async fn request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system: &str,
    ) -> Result<ModelResponse> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 8192,
            "system": system,
            "messages": convert_messages(messages),
            "tools": convert_tools(tools),
        });

        let resp = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let data: AnthropicResponse = resp.json().await?;
        Ok(parse_response(data))
    }
}
```

~300 lines total including the request/response serde types and message conversion. No SDK needed.

## Communication: lf-agent ↔ lfd

The agent process communicates with lfd via JSON lines on stdout (inspired by Codex's event stream pattern, simplified):

```rust
// Agent → lfd (stdout, one JSON object per line)
#[derive(Serialize)]
#[serde(tag = "type")]
enum AgentEvent {
    #[serde(rename = "message")]
    Message {
        content: String,
        phase: String, // "progress" | "final"
    },

    #[serde(rename = "memory_edit")]
    MemoryEdit { op: String, block: String, detail: String },

    #[serde(rename = "tool_call")]
    ToolCall { tool: String, args: serde_json::Value },

    #[serde(rename = "tool_result")]
    ToolResult { tool: String, summary: String },

    #[serde(rename = "done")]
    Done {
        memory: Memory,
        context: ContextSnapshot,
    },
}
```

lfd reads these events, streams relevant ones to the client (via WebSocket or HTTP response), and persists the final state when it receives `Done`.

## lfd API: `/v0/waves/:wave_id/chat`

```
GET    /chat              → current state (memory blocks + recent messages)
POST   /chat/messages     → send a user message, spawn agent, get response
POST   /chat/compact      → LLM-summarize memory, clear history
PATCH  /chat/memory       → manual memory edit (user edits a block directly)
DELETE /chat              → reset (clear memory + history)
```

**POST /chat/messages** — the core endpoint:

```json
// Request
{ "content": "What's the status of the auth refactor?" }

// Response
{
  "id": "msg_01abc",
  "response": "The auth refactor is about 70% done...",
  "memory_edits": [
    { "op": "replace", "block": "project", "old": "auth: planning", "new": "auth: 70% complete" }
  ],
  "tool_calls": [
    { "tool": "read_file", "args": { "path": "src/auth/middleware.rs" }, "result_summary": "148 lines" }
  ],
  "context": {
    "memory_tokens": 1200,
    "history_tokens": 3400,
    "total_tokens": 8600
  }
}
```

## System prompt

```
You are an agent attached to wave "{wave_name}" in repo {repo}.

You have a persistent memory organized into named blocks. You decide what blocks
to create and maintain. Memory persists across conversations — treat it as your
working notes.

## Current Memory
{rendered memory blocks with labels and line numbers}

## Rules
- Update memory with important context, decisions, and state changes.
- Use send_message for every user-visible message (progress and final) — your inner reasoning is private.
- End every turn with exactly one send_message where phase is "final".
- Keep blocks focused. Create new blocks for new topics.
- When information changes, use memory_replace to update specific parts.
- Use memory_rethink only when a block needs wholesale reorganization.
```

## Persistence (lfd store)

```sql
CREATE TABLE chat_memory_blocks (
    wave_id TEXT NOT NULL REFERENCES waves(id),
    label TEXT NOT NULL,
    content TEXT NOT NULL,
    read_only BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (wave_id, label)
);

CREATE TABLE chat_messages (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id),
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    tool_calls TEXT,          -- JSON
    context_snapshot TEXT,     -- JSON
    created_at TEXT NOT NULL
);
```

## Python client (v1 deliverable)

```python
import loopflow.api as loopflow

wave = loopflow.wave("engbot")

# Send a message
result = loopflow.chat_send(wave.id, "What's the status of the auth work?")
print(result.response)
print(result.memory_edits)

# View memory
memory = loopflow.chat_memory(wave.id)
for block in memory.blocks:
    print(f"[{block.label}]")
    print(block.content)

# Edit memory directly
loopflow.chat_memory_edit(wave.id, block="project", content="new content")

# Compact
loopflow.chat_compact(wave.id)

# Interactive REPL
loopflow.chat_repl(wave.id)
```

## Crate structure

```
rust/
  lf-agent/           ← new binary crate
    src/
      main.rs          ← CLI entry point, loads state, runs loop, outputs events
      loop.rs          ← run_turn agent loop
      model/
        mod.rs         ← Model trait
        anthropic.rs   ← Anthropic Messages API implementation (~300 lines)
      tools/
        mod.rs         ← ToolHandler trait, ToolRegistry, ToolDefinition
        memory.rs      ← send_message, memory_replace/insert/rethink/delete
        files.rs       ← read_file, write_file
        shell.rs       ← shell command execution
      memory.rs        ← Memory, MemoryBlock, render/apply operations
      messages.rs      ← Message enum, ContextSnapshot
      events.rs        ← AgentEvent (stdout JSON lines)
    Cargo.toml         ← deps: reqwest, serde, serde_json, tokio, anyhow, async-trait
```

Total estimated size: ~2000 lines for v1. Dependencies: reqwest, serde, serde_json, tokio, anyhow, async-trait. No framework dependency.

## Constraints

- **Always part of a wave.** Chat doesn't exist without a wave. The wave provides repo, worktree, and tool sandbox.
- **Memory edits are tool calls.** Same function-calling mechanism as agent tools.
- **send_message is the only user-output mechanism.** Progress/final messages are explicit `send_message` tool calls from the model.
- **Final message is required.** Every successful turn must end with exactly one `send_message(phase=\"final\")`.
- **Blocks are agent-defined.** No predefined schema. Agent creates whatever blocks make sense.
- **History is bounded by tokens and compacted.** Memory remains the durable artifact.
- **Memory writes persist per edit.** Each memory tool call is applied and durably saved immediately.
- **Spawned like a step.** Each chat message spawns a process via lfd. Same executor model.
- **Own model layer.** No framework dependency. Anthropic first, add providers by implementing the Model trait.
- **JSON lines on stdout.** Agent → lfd communication. Simple, debuggable, streamable.

## Open design questions (intentional experiments)

- **Failure closure path:** prefer client-side graceful handling first. Optional future enhancement: lfd can emit a synthetic final error message when the agent fails before `phase="final"`.
- **Workspace mutability across turns:** v1 behavior is ephemeral write/shell effects (isolated container/workspace copy per turn/lane), with optional future mode for persistent branch writes.
- **Commit/push lifecycle:** preserve theoretical capability from day one, but gate actual branch mutation behind an explicit, auditable process (not implicit in normal chat turns).

## Future extension: explicit branch mutation flow

Default chat-turn behavior remains ephemeral for filesystem changes.  
If/when enabled, persistent code mutation should be explicit:

1. Agent proposes a patch/commit plan in chat.
2. System runs explicit "apply/commit" action (separate from normal turn loop).
3. Commit metadata is recorded in run/chat logs.
4. Optional explicit "push" action executes after commit.

## Fundamental parts we should not drop

- **Memory-first contract:** memory is durable and explicit; users can view/edit/compact it.
- **Durability boundary:** memory persists across turns; filesystem changes do not (by default).
- **Bounded history in prompt:** include bounded/compacted harness history for continuity, but keep memory as primary.
- **Explicit message tool path:** user-visible output only via `send_message` tool calls.
- **Progress + final shape:** allow `0..∞` progress messages and require exactly one final message per successful turn.
- **Tool-call loop architecture:** model call → tool dispatch → tool results fed back → repeat until completion contract.
- **Codex-derived runtime patterns:** two-level loop shape, tool registry/handler abstraction, and structured event stream.
- **Wave-scoped tools in v1:** include `read_file`, `write_file`, and `shell`.
- **JSONL event stream:** agent emits structured events to lfd for live UI streaming and persistence.
- **Loop safety guardrails:** copy Codex-style hard stops (iteration cap + wall-clock timeout) to prevent runaway turns.
- **Concurrent lane model:** chat agent runs alongside wave runs, in its own executor lane (container for container executors, process for process executors).
- **Branch snapshot at turn start:** each chat turn launches against the latest branch state available when that turn starts.

## Ordering and scope (triangular vertical slice)

Design intent from conversation:

> "vertical slice, but triangular shape -- the UI is the thinnest, then the python client, and the lf-agent is the thickest to start"

Implementation order for first draft:

1. **lf-agent (thickest layer)**
   - Build the core turn loop, tool dispatch, memory mutation semantics, and final-message termination contract.
   - Implement Anthropic provider and `send_message` + memory tools first.
2. **Python client (thin middle layer)**
   - Expose minimal wrappers: `chat_send`, `chat_memory`, `chat_memory_edit`, `chat_compact`.
   - Keep client logic light; pass through rich payloads from lfd.
3. **UI surface (thinnest layer)**
   - Render streamed explicit messages (`progress`/`final`) and failure states gracefully.
   - No heavy business logic in UI.

### Initial scope choices (locked for first draft)

- **lf-agent:** include `send_message`, memory tools, and wave tools (`read_file`, `write_file`, `shell`) from day one.
- **Python client:** API methods only (no REPL in first draft), used for local testing.
- **UI:** Swift client + UI stream and render messages + memory **viewer** (no memory editor yet).

## Not in v1

- Streaming model responses (complete responses first)
- OpenAI / Gemini providers (Anthropic only)
- Auto-compact (manual compact only)
- Archival/vector memory (core blocks only)
- MCP tool integration (hardcoded tools first)
- Swift UI (API is designed for it)

## Done when

```bash
# Create a wave, send a message, see memory update
uv run python -c "
import loopflow.api as lf
w = lf.create_wave('test-chat', '.')
r = lf.chat_send(w.id, 'Hello, remember that I prefer Rust over Python')
print(r.response)
print(r.memory_edits)
m = lf.chat_memory(w.id)
print(m.blocks)
lf.delete_wave(w.id)
"
```

Memory persists across messages. Compact produces tighter memory and clears history. Manual memory edits are reflected in the next LLM call.
