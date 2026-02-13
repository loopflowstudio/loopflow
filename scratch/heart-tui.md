# Chat: Memory-First Agent for Waves

A memory-first agent harness for waves. The memory buffer is the primary artifact — visible, editable, persistent. The LLM APIs are message-based, not chat-oriented. We build a memory-centric interface on top: the data model is memory blocks and tool calls, chat is just the UX.

Each chat message spawns an agent process the same way a step does — same executor/container model. But instead of shelling out to `claude` or `codex`, we run our own Rust agent harness.

## What to build

A Rust agent binary + lfd API surface + Python CLI client.

When you send a chat message to a wave, lfd spawns a Rust agent process. The process:
1. Loads current memory blocks + recent history from lfd (HTTP)
2. Builds a prompt (system + memory + history + user message)
3. Calls the LLM API directly (Anthropic first, OpenAI/Gemini later)
4. Dispatches tool calls (memory edits, file ops, shell, send_message)
5. Loops until no more tool calls (Codex/OpenCode termination pattern)
6. Persists updated memory + message log back to lfd
7. Streams events to lfd during execution
8. Exits

## Architecture

```
Python CLI / Swift UI
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

### Inspired by

- **Codex CLI** (Rust): SQ/EQ channel pattern, two-level agent loop, ToolHandler trait + registry, ContextManager as Vec<Item>. But tightly coupled to OpenAI — no model trait. We add provider abstraction.
- **OpenCode** (TypeScript): Vercel AI SDK for provider abstraction, named agents with permission presets, session compaction.
- **MemGPT/Letta**: Memory as tool calls, core memory in system prompt, send_message pattern, memory_replace/insert/rethink operations.
- **pydantic-ai** (Python): 3-node agent loop (UserPrompt → ModelRequest → CallTools → loop/end), Model trait with request()/stream(), structured output via tool-based schema.

We study all four but depend on none.

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
}

pub enum ToolOutput {
    /// Tool produced a result to feed back to the model.
    Result(String),
    /// Tool is send_message — this is the user-visible response.
    UserMessage(String),
    /// Tool modified memory — return confirmation, memory is updated in-place.
    MemoryEdited(String),
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
            // No tool calls — turn is done.
            // If no send_message was called, treat content as the response.
            break;
        }

        let mut needs_follow_up = false;

        for call in &response.tool_calls {
            let output = tools.dispatch(call, ctx).await?;
            tool_call_log.push(call.clone());

            match &output {
                ToolOutput::UserMessage(msg) => {
                    user_messages.push(msg.clone());
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
                ToolOutput::UserMessage(_) => "Message sent.".to_string(),
                ToolOutput::MemoryEdited(s) => s,
            };
            history.push(Message::ToolResult {
                tool_call_id: call.id.clone(),
                content: result_text,
            });
        }

        // If only memory edits and send_message — no need for follow-up.
        // If agent tools (read_file, shell) were called — model needs results.
        if !needs_follow_up {
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
1. The model produces no tool calls (text-only response), OR
2. All tool calls were memory edits or send_message (no results to feed back)

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
    Message { content: String },

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
- Use send_message for every response — your inner reasoning is private.
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
- **send_message is required.** LLM content field is private reasoning. Only send_message reaches the user.
- **Blocks are agent-defined.** No predefined schema. Agent creates whatever blocks make sense.
- **History is expendable.** Compact wipes history. Memory is the durable artifact.
- **Spawned like a step.** Each chat message spawns a process via lfd. Same executor model.
- **Own model layer.** No framework dependency. Anthropic first, add providers by implementing the Model trait.
- **JSON lines on stdout.** Agent → lfd communication. Simple, debuggable, streamable.

## Not in v1

- Streaming model responses (complete responses first)
- OpenAI / Gemini providers (Anthropic only)
- Auto-compact (manual compact only)
- Archival/vector memory (core blocks only)
- MCP tool integration (hardcoded tools first)
- Concurrent chat + step execution (queue behind running step)
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
