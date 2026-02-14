# Foundation Contract

Lock the chat/runtime contract before heavy implementation so all layers agree on turn semantics.

## Problem

The harness needs a stable contract between the agent runtime and the chat system. Without one, B1 (single turn end-to-end) and A2 (chat powered by harness) will make incompatible assumptions about event shapes, completion rules, and turn boundaries.

The contract is the handshake: what events the harness emits, what a "successful turn" looks like, how the chat system knows the agent is done. Get this wrong and every layer above has to compensate.

## Approach

Types + validation + tests, no runtime behavior. The contract lives in `chat/` and defines:

1. **Event vocabulary** — `AgentEvent` enum with explicit variants (Message, ToolCall, ToolResult, MemoryEdit, Done, Failed)
2. **Phase semantics** — `send_message` carries a phase (progress/final). The harness doesn't stream raw tokens; every user-visible message is an explicit event.
3. **Completion rule** — exactly one `Message { phase: Final }` on successful turns. Zero or more `Progress` messages before it. `validate_turn_completion` enforces this.
4. **Turn payloads** — `ChatTurnRequest` (what the chat system sends) and `ChatTurnResult` (what comes back after a turn completes)
5. **Workspace context** — `WorkspaceSnapshot` captures branch + head SHA at turn start, so the chat system can track what the agent saw

## What exists

All three commit slices (C1, C2, C3) are substantially complete:

**C1 — Types (done).** `contract.rs` has `UserMessagePhase`, `SendMessageArgs`, `WorkspaceSnapshot`, `ChatTurnRequest`, `ChatTurnResult`, `AgentEvent`, `ContextSnapshot`, `MemoryEditLog`, `ToolCallLog`, and `parse_send_message_args`.

**C2 — Validation (done).** `completion.rs` has `validate_turn_completion`, `final_message_count`, `is_user_message`, and `CompletionError` with `MissingFinalMessage` / `MultipleFinalMessages` variants.

**C3 — Tests (partially done).** `contract_test.rs` has 8 tests covering serde round-trips, valid/invalid payloads, and completion rules. `completion.rs` has 1 inline test for message classification helpers. All pass.

### What's missing for "done"

The design doc's done-when says `cargo test -p loopflow chat_contract` — but that filter matches 0 tests because the module path is `chat::contract_test`, not `chat_contract`. Two options:

1. Fix the done-when criterion to `cargo test -p loopflow contract_test` (matches the 8 existing tests)
2. Fix the done-when criterion to `cargo test -p loopflow chat::` (matches all 9 chat tests including completion)

Beyond the naming issue, the test coverage has gaps worth closing before moving to B1:

| Gap | Why it matters |
|-----|---------------|
| No round-trip tests for `ChatTurnRequest` or `ChatTurnResult` | These are the primary payloads crossing the harness→chat boundary. If serialization breaks, everything breaks. |
| No round-trip tests for all `AgentEvent` variants | Only `Message` is tested. `ToolCall`, `ToolResult`, `MemoryEdit`, `Done`, `Failed` all contain `serde_json::Value` or nested types that could silently break. |
| No test for `WorkspaceSnapshot` round-trip | Trivial but proves the type is stable. |
| `CompletionError` has no `Display` impl | Consumers can't format errors for users. Should derive or implement `std::fmt::Display` (currently it derives only `Debug, Clone, PartialEq, Eq`). |
| No failed-turn validation | The completion rule says "no Final on failed turns" but there's no `validate_failed_turn` or equivalent. A turn that emits `Failed { .. }` AND `Message { phase: Final }` should be rejected. |
| `ContextSnapshot` defaults to all zeros | Fine for now, but worth a test proving the default. |

## Key decisions

### Use `thiserror` for `CompletionError`

Currently `CompletionError` has no `Display` impl. Add `thiserror` derives so consumers get human-readable messages. This follows the Rust style guide: "Use `thiserror` for library error types callers need to match on."

### Add failed-turn validation

The roadmap says "enforce no Final on failed turns." The existing `validate_turn_completion` only checks successful turns. Add a sibling function or extend the existing one:

```rust
pub fn validate_turn_completion(events: &[AgentEvent]) -> Result<(), CompletionError>
```

Already handles the happy path. Add a `FinalMessageOnFailedTurn` variant to `CompletionError`, and have the validator check that if any `Failed` event exists, no `Final` message accompanies it.

### Don't add versioning yet

The constraints say "keep schemas stable and versionable." The types are `#[non_exhaustive]` which is sufficient for now — new variants can be added without breaking existing consumers. Explicit version fields (`schema_version: u32`) are premature until we have a second consumer or wire protocol.

### Keep `AgentEvent` model-agnostic

The constraints say "do not bind contract types to Anthropic/OpenAI response shapes." The current `AgentEvent` enum is clean — it uses semantic concepts (Message, ToolCall, Done) not model-specific ones (content_block_delta, tool_use). This is correct. The existing `agent/anthropic.rs` types (`ContentBlock`, `Message`) stay separate from `chat/contract.rs` types.

## Scope

**In scope:**
- Add serde round-trip tests for all contract types (`ChatTurnRequest`, `ChatTurnResult`, `WorkspaceSnapshot`, all `AgentEvent` variants)
- Add `thiserror` derives to `CompletionError`
- Add failed-turn validation (no `Final` alongside `Failed`)
- Add test for empty event stream
- Fix done-when filter to match actual test paths

**Out of scope:**
- Runtime behavior (B1 territory)
- Wire format versioning
- JSONL serialization (that's B2, when events stream to stdout)
- `send_message` tool registration (B2)

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Merge completion validation into `AgentEvent` methods | Keeps validation close to data | Validation is a cross-cutting concern over event *sequences*, not individual events. A standalone function is clearer. |
| Add explicit `schema_version` field now | Future-proofs wire format | Premature — `#[non_exhaustive]` handles variant addition. Version when we have a second consumer. |
| Make `validate_turn_completion` take a `TurnOutcome` enum (Success/Failed) instead of checking events | Cleaner API | The validator should work from events alone — the caller shouldn't have to pre-classify the outcome. |

## Done when

```bash
cargo test -p loopflow chat::
```

Expected: all contract + completion tests pass, including:
- Serde round-trip for every `AgentEvent` variant
- Serde round-trip for `ChatTurnRequest` and `ChatTurnResult`
- Completion validation: exactly-one-final, no-final, multiple-finals, progress-then-final
- Failed-turn validation: `Failed` + `Final` rejected
- Empty event stream rejected
- `CompletionError` has human-readable `Display` output
