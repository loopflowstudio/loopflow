# Gate review: mobile action buttons (Stage 02)

Branch: `jack-heart.mobile.20260225_1122`

## What was implemented

One-tap action buttons for chat sessions. The agent suggests next steps after completing a turn; the user taps one instead of typing. The full pipeline:

1. **Engine** (`structured_reply.rs`): `StructuredReply` + `ClientContext` determine which replies to inject based on whether the client has UI and whether it's compact (iPhone).
2. **Prompt guidance**: Harnesses append `<lf:structured_replies>` to the system prompt with per-step style (procedural/exploratory/default).
3. **Stream parsing** (`lf_tag.rs`): `LfTagParser` strips `<lf:suggest_actions>` tags from streaming text deltas and emits `SessionEvent::SuggestedActions`.
4. **Swift state** (`SessionState`): Receives suggested actions, sanitizes (caps count, drops empty labels), clears on user send/typing/new turn/session end.
5. **Swift UI** (`ActionButtonsView` + `WaveSessionView`): Renders action buttons above the composer with hit-testing delay. Tap sends as a user message.

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| Prompt guidance over MCP tool registration | Provider-agnostic; works with Claude, Codex, any LLM | MCP requires per-provider registration, adds complexity |
| Text-tag protocol (`<lf:suggest_actions>`) | Embeds in existing text stream; no new transport | Separate event channel would need protocol changes |
| `StructuredReply` naming (not `SyntheticTool`) | Describes what it is (a reply structure), not mechanism | `SyntheticTool` confused readers about whether it's a real tool |
| `SessionState` naming (not `ChatState`) | Aligns with the engine concept (sessions, not chats) | `ChatState` was UI-centric naming leaking into state layer |
| Drop user text blocks in `claude_mapping` | Claude echoes user input as text blocks; emitting them duplicated messages | Could have filtered at UI layer, but wrong data should be fixed at source |

## How it fits together

```
Engine (structured_reply.rs)
  → generates StructuredReply for UI contexts
  → harness appends guidance to system prompt

LLM response stream
  → LfTagParser strips <lf:suggest_actions> tags
  → emits SessionEvent::SuggestedActions

Swift SessionState
  → receives SuggestedActions events
  → sanitizes and stores in suggestedActions
  → ActionButtonsView renders above composer
  → tap → sendSuggestedAction → clears actions
```

## Risks and bottlenecks

- **Prompt compliance**: Suggestion quality depends on the model following prompt guidance. No enforcement mechanism exists. A model that ignores or misformats the tag silently produces no suggestions (safe failure, but invisible).
- **Tag parsing robustness**: `LfTagParser` handles split chunks and malformed JSON gracefully (tested). Tags are only matched at line start or after whitespace, so inline references (e.g. inside backtick code spans) are ignored. Edge case: a tag on its own line inside a fenced code block could still be parsed. Low probability — prompt guidance explicitly tells the model not to wrap lf tags in backticks or mention them in prose.
- **No persistence**: Suggestions are ephemeral. If the user navigates away and back, they're gone. Acceptable for Stage 02; persistence is out of scope.

## What's not included

- Additional structured replies beyond `suggest_actions` (e.g., `memory`, `confirm`).
- Persistence or analytics for which actions users select.
- MCP-style tool registration (prompt guidance is sufficient for now).
- Full-screen interleaving polish (item 5 in the design doc — visual refinement, not functional).

## Renames in this branch

These renames affect the entire surface area (files, structs, functions, fields, tests, docs):

| Before | After | Scope |
|--------|-------|-------|
| `synthetic.rs` | `structured_reply.rs` | Rust engine |
| `SyntheticTool` | `StructuredReply` | Rust struct + all references |
| `synthetic_tools` | `structured_replies` | Rust field + functions |
| `render_synthetic_guidance` | `render_structured_reply_guidance` | Rust function |
| `ChatState` | `SessionState` | Swift class + all references |
| `WaveChatView` | `WaveSessionView` | Swift view + all references |
| `ChatStateTests` | `SessionStateTests` | Swift test suite |
| `chatState(for:)` | `sessionState(for:)` | Swift RepoState method |
| `shouldShowInteractiveChat` | `shouldShowInteractiveSession` | Swift RepoState method |
| `MockChatService` | `MockSessionService` | Swift test mock |

## Bug fix: auto-send duplicate messages

Root cause: `process_user_message()` in `claude_mapping.rs` emitted text blocks from Claude's "user" type events as `SessionItem::Message`. These are Claude echoing back user input the client already displayed — not new user actions. Fix: drop text blocks from user message processing; only process `tool_result` blocks.

## Validation

All passing locally:

| Check | Result |
|-------|--------|
| `cargo fmt --all -- --check` | Clean |
| `cargo clippy --all-targets -- -D warnings` | Clean |
| `cargo test --all` (skip docker) | All passed |
| `uv run pytest python/tests/` | 47 passed |
| `swift test --package-path swift` | 151 passed |

Environment-limited: Docker-socket tests and `xcodebuild` UI tests not runnable locally (no Docker socket, Xcode linker issue).

## Post-review polish (gate pass)

Changes made during the gate pass after the initial review:

- **Inline tag filtering**: `LfTagParser.find_tag_start` only matches `<lf:suggest_actions>` at line start or after whitespace, preventing false matches on inline code examples.
- **Prompt hardening**: Guidance now tells the model to emit the tag on its own lines and not wrap lf tags in backticks.
- **Hit-testing simplification**: `ActionButtonsView` uses `.task(id:)` instead of manual `Task` + `onAppear/onChange/onDisappear`.
- **Reduced clearing triggers**: Removed `composerTextDidChange` (suggestions persist until user sends) and `clearSuggestedActions` from `turnStarted` (new suggestions replace old ones via `applySuggestedActions`).
