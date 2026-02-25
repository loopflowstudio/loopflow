# 02: Action Buttons

## Problem

Chat sessions frequently end with the user thinking "what should I do next?" and then typing a command manually. That friction is worst on iPhone, where typing is slow and one-handed use is common.

We need an agent-driven, tappable next-step surface that:
- turns likely follow-up actions into one tap,
- works in the existing session protocol,
- behaves consistently on iPhone, iPad, and Mac.

Who benefits:
- mobile users running many short task loops,
- new users who do not know canonical next commands,
- desktop users who want faster repeat actions.

Why now:
- Stage 01 established shared chat state + platform embedding boundaries,
- this stage can ship value without waiting for discovery or transport work.

## Approach

Build **synthetic tool injection** in the engine, with `suggest_actions` as the first consumer. The mechanism is general — the same pipeline will later carry `memory`, `show_diff`, `request_approval`, and other tools that lfd injects based on execution context.

### Synthetic tools in the engine

A synthetic tool is a tool definition + prompt guidance that the engine injects into `AgentConfig`. The harness decides how to realize it per-provider (Claude: system prompt instructions; Codex: potentially different). The agent sees the tool and calls it like any other tool.

```rust
// engine/synthetic.rs

/// A tool the engine injects into the agent's context.
#[derive(Debug, Clone)]
pub struct SyntheticTool {
    pub name: String,
    pub description: String,
    pub schema: serde_json::Value,
    pub guidance: String,
}

/// What the caller tells the engine about its execution environment.
#[derive(Debug, Clone, Default)]
pub struct ClientContext {
    pub has_ui: bool,
    pub compact: bool,
}

/// Return the synthetic tools appropriate for this context.
pub fn synthetic_tools_for_context(ctx: &ClientContext) -> Vec<SyntheticTool> {
    let mut tools = Vec::new();
    if ctx.has_ui {
        tools.push(suggest_actions_tool(ctx));
    }
    // Future: tools.push(memory_tool()); — injected unconditionally
    tools
}
```

### AgentConfig carries the tools

```rust
pub struct AgentConfig {
    pub system_prompt: String,
    pub task_prompt: String,
    pub model: Option<String>,
    pub max_turns: Option<u32>,
    pub cwd: Option<PathBuf>,
    pub skip_permissions: bool,
    pub synthetic_tools: Vec<SyntheticTool>,  // NEW
}
```

`prepare_launch_prompt` accepts a `ClientContext` (via `LaunchPromptInput`) and calls `synthetic_tools_for_context` to populate the field. Callers provide context:
- Session creation (lfd): `has_ui: true`, `compact` from client metadata.
- Wave executor (lfd): `has_ui: false` (headless batch).
- `lf` CLI: `has_ui: false` (headless, for now).

### Harness realization

Each harness translates `synthetic_tools` into system prompt instructions. No MCP, no formal tool registration. The agent emits structured output using `<lf:tool_name>` tags — a convention we already use throughout the prompt system.

**How it works:** The harness appends guidance for each synthetic tool to the system prompt. The guidance tells the agent to emit tagged output:

```
When you want to suggest next actions, emit:

<lf:suggest_actions>
[{"label": "Land PR", "description": "Merge and clean up"}, {"label": "Run tests"}]
</lf:suggest_actions>
```

The agent emits this as regular text in its response. The stream parser detects the `<lf:...>` tag and extracts the payload. No special provider support needed — any agent that follows system prompt formatting instructions works.

**Why this works across providers:** Claude, Codex, Gemini, OpenCode — all can follow "emit this XML tag with this JSON payload" instructions. The `<lf:...>` namespace is already established in the prompt system for context sections. Using it for tool output is a natural extension.

**Harness responsibility:** Each harness appends the synthetic tool guidance text to the system prompt (or equivalent). For Claude, that's `--append-system-prompt`. For Codex, it goes in the system message. The guidance text is provider-agnostic — it's just "emit this tag with this JSON."

**Stream parsing (Rust):** The existing stream parsers (`claude_mapping::process_line`, `codex_mapping`) deliver `TextDelta` events. A shared `LfTagParser` intercepts text deltas before they reach the event channel:

1. Accumulates text across deltas, watching for `<lf:suggest_actions>` open tags.
2. While inside a tag, buffers content instead of emitting it as `TextDelta` (strips the tag from chat output).
3. On `</lf:suggest_actions>`, parses the buffered JSON and emits `SessionEvent::SuggestedActions { actions }`.
4. Text outside tags passes through as normal `TextDelta` events.

The parser is provider-agnostic — both Claude and Codex mappings feed text through it. Generalizes to future `<lf:memory>`, `<lf:request_approval>`, etc.

New `SessionEvent` variant:

```rust
SuggestedActions {
    turn_id: String,
    actions: Vec<SuggestedActionPayload>,
}
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedActionPayload {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
```

On the Swift side, `ChatState` handles `SuggestedActions` like any other event — updates `suggestedActions`, no text scanning needed.

### ClientContext propagation

Session creation already receives a `SessionConfig` from the HTTP API. Add optional client context fields:

```rust
pub struct SessionConfig {
    // ... existing fields ...
    pub client_has_ui: Option<bool>,
    pub client_compact: Option<bool>,
}
```

The Swift client populates these when creating sessions. `client_has_ui: true` always (Concerto is a UI), `client_compact: true` on iPhone (`horizontalSizeClass == .compact`).

The engine doesn't need to know about iPhones — it just knows `has_ui` and `compact`, and picks the right tool guidance accordingly.

### `suggest_actions` tool definition

```rust
fn suggest_actions_tool(ctx: &ClientContext, action_style: Option<&str>) -> SyntheticTool {
    let max_actions = if ctx.compact { 3 } else { 4 };
    let style_guidance = match action_style {
        Some("procedural") =>
            "Suggest actions that move the workflow forward. Binary choices, \
             clear next steps. Examples: \"Ship it\", \"Fix the test failure\", \
             \"Land PR\", \"Defer to next wave\".",
        Some("exploratory") =>
            "Suggest actions that open interesting paths. Branching choices, \
             guided curiosity. Examples: \"Dig into the auth model\", \
             \"Try a different approach\", \"What if we split this?\".",
        _ =>
            "Prefer concrete actions (\"Land PR\", \"Run tests\") over vague \
             ones (\"Continue\", \"Tell me more\").",
    };
    SyntheticTool {
        name: "suggest_actions".to_string(),
        description: "Suggest next actions the user might want to take.".to_string(),
        schema: json!({ /* ... actions array with label + optional description ... */ }),
        guidance: format!(
            "Use `suggest_actions` to suggest {max_actions} next actions \
             the user might want to take. Call it after completing a task, \
             when waiting for user input, or when presenting results. Each \
             action should be a short phrase that makes sense as a user message. \
             {style_guidance}"
        ),
    }
}
```

### Step-driven action style

Steps declare an `action_style` in frontmatter:

```yaml
---
action_style: procedural   # or: exploratory
---
```

Two modes:

| Mode | Steps | Flavor |
|------|-------|--------|
| `procedural` | review, gate, ci-fix, implement | Forward momentum. "What's the next thing in the workflow?" |
| `exploratory` | design, refine, explore, review-design | Branching paths. "What's interesting from here?" |

Steps without `action_style` get generic guidance. The engine reads the frontmatter and passes it through to `suggest_actions_tool`.

Steps can also shape suggestions through their body prose — no special heading or convention needed. If a step like `review-design` says "end with a clear verdict: ship, iterate, or rethink" in its instructions, the agent will naturally suggest those as actions. The frontmatter sets the base; the prose adds nuance.
```

### Shared state and parsing (LoopflowCore)

1. Add `SuggestedAction` model (`label`, optional `description`, UUID id).
2. Add `suggestedActions: [SuggestedAction]` to `ChatState`.
3. Rust stream parser detects `<lf:suggest_actions>` tags in text deltas, accumulates content between open/close tags, parses the JSON payload, and emits a `SessionEvent::SuggestedActions` event. Swift `ChatState` handles this event like any other — no tag parsing on the client side.
4. Payload handling rules:
   - keep only actions with non-empty `label`,
   - cap to 4 actions,
   - latest valid tool call replaces prior actions,
   - invalid payload yields no update.
5. Clear actions on:
   - user send,
   - agent turn start,
   - user begins manual typing,
   - session end/replay reset.

### UI behavior

1. Build shared `ActionButtonsView` in LoopflowCore.
2. Render above composer in `WaveChatView` (shared surface; iOS entry point remains `MobileWaveDetailView`).
3. Layout:
   - compact width (iPhone): vertical full-width cards with optional subtitle,
   - regular width (iPad/Mac): horizontal wrapping pills.
4. Accessibility:
   - 44pt min touch target on iPhone,
   - visible focus ring on keyboard platforms,
   - VoiceOver reads label + description.
5. Interaction:
   - tap calls `state.send(label)` — exact same path as typed input,
   - actions disappear immediately after tap.

### Wild success target

Users begin treating the strip as the default continuation path: most follow-up turns after an assistant result are one tap, not manual typing.

The synthetic tool mechanism becomes the standard way to add agent-side UI conventions. `memory` ships next using the same pipeline.

### Wild failure to design against

Six months later we remove the feature because actions feel stale/noisy and users mistrust them. Mitigations:
- aggressive clearing rules,
- strict payload sanitization,
- no fallback UI when agent does not suggest actions.

The synthetic tool mechanism becomes over-engineered dead code because we only ever ship one tool. Mitigations:
- keep the mechanism minimal (one function, one struct, one vec on AgentConfig),
- `suggest_actions` is useful on its own regardless of whether more tools follow.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Concerto-side prompt injection | Fastest to buttons on screen | Throwaway work; every future synthetic tool redoes injection; no headless path |
| MCP tool registration | Clean `tool_use` blocks | Too much machinery; adds MCP server dependency; not portable across providers |
| Client-generated suggestions (heuristics) | No agent tool use needed | Often wrong/out of context; hides model intent; harder to trust |
| Text-only suggestions inside messages | Zero protocol work | Not tappable; no structured parsing; weak mobile benefit |

## Key decisions

- **Synthetic tool injection lives in the engine, carried on `AgentConfig`.** One mechanism for all synthetic tools. Harness realizes per-provider.
- **`ClientContext` is the injection signal.** The engine doesn't know about iPhones — it knows `has_ui` and `compact`. Platform details stay in the client.
- **Harness realizes via system prompt + `<lf:>` tags.** No MCP, no tool registration. The agent emits tagged text, the parser extracts it. Works across all providers.
- **Keep parsing in shared `ChatState`.** One behavior path for iOS + macOS avoids drift.
- **Latest call wins.** Action sets are ephemeral guidance, not history.
- **Clear aggressively.** Better to hide too early than show stale actions.

## Scope

In scope:
- `SyntheticTool` struct + `ClientContext` in engine
- `synthetic_tools_for_context` with `suggest_actions` as first tool
- `synthetic_tools: Vec<SyntheticTool>` on `AgentConfig`
- `ClientContext` propagation through `LaunchPromptInput` and `SessionConfig`
- `action_style` frontmatter field on steps; parsed and passed to tool guidance
- `action_style` added to interactive + code steps that ship with loopflow
- Harness realizes synthetic tools via system prompt guidance + `<lf:>` tag convention
- `LfTagParser` in Rust stream layer: detects `<lf:tool_name>` tags across text deltas
- `SessionEvent::SuggestedActions` variant; Swift `ChatState` handles it as a typed event
- `ClientContext` on session creation API
- Shared `SuggestedAction` model + `ChatState` parsing/clearing
- Shared `ActionButtonsView`
- Embedding in iOS/macOS chat surfaces via `WaveChatView`
- Tap-to-send behavior parity with typed input
- Swift client sends client context on session creation

Out of scope:
- Codex harness realization (no Codex sessions in Concerto yet)
- `memory` tool (next PR, same pipeline)
- Headless `lf` convergence toward sessions
- Action discovery/ranking systems (Stage 04)
- New transport/wire formats (Stage 03)
- Provider-specific realization beyond system prompt (MCP, JSON-RPC tool defs)
- Persistence/analytics for suggested actions

## Done when

Observable behavior:
1. lfd injects `suggest_actions` guidance into sessions created with `has_ui: true` in client context.
2. Agent emits a `suggest_actions` tool call and action buttons render in chat on iPhone, iPad, and Mac.
3. Tapping a button sends its label as a user message and clears buttons immediately.
4. Buttons clear when typing manually or when a new agent turn starts.
5. Sessions created without `client_has_ui` (or with `false`) do not get `suggest_actions` injection.

Verification:
- `cargo test -p loopflow synthetic` — synthetic tool selection based on ClientContext
- `cargo test -p loopflow lf_tag` — LfTagParser strips tags, emits structured events
- `cargo test -p loopflow claude` — harness appends synthetic tool guidance to system prompt
- `swift test --package-path swift` — ChatState handles SuggestedActions event; ActionButtonsView rendering
- `uv run python scripts/concerto-dev.py run-ios` (manual iPhone/iPad)
- `uv run python scripts/concerto-dev.py run-debug` (manual macOS)
