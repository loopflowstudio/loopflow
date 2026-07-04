# Open questions — jack-heart.concerto-wavechat

## WaveChat first surface: data source (resolved by best judgment)

The brief assumed `lf wave` writes per-pass logs under `wave/<name>/streams/`
that WaveChat could parse into turns. That capture was **removed** ("inherit
terminal instead of teeing passes"): nothing read the streams, and each inner
`lf -b goal` pass already writes durable logs under the agent's log dir. So
there is no `streams/` source to parse.

Decision taken (the brief's documented fallback): back `WaveChatView` with a
small local `WaveChatSource` that reconstructs two turns from the two-file wave
surface `lf wave` maintains — `GOAL.md` (the operator's directive → opening turn)
and `MEMORY.md` (the wave's working memory → its reply). A "latest state"
snapshot, not a live transcript.

## Base branch note

The brief said to open this PR against `jack-heart.lf-loop` (PR #778). By the
time the slice was ready, #778 had been squash-merged into `main` and its branch
deleted, so this PR targets `main` instead — which now contains #778.

## Follow-ons (separate PRs, not built here)

- Rust **codex harness** for the wave's inner agent.
- **Per-wave in-process chat server** exposing streamed `ChatTurn`s.
- **Live turn updates** in `WaveChatView` (replace the two-file reconstruction).
- Chat **input/composer** to message a running wave.

`WaveChatSource` is the single seam to swap when the server lands.

## WaveChat backend landed (this PR extends #786)

The Rust half of the follow-ons above is now built:

- **Restored** the harness engine (codex/claude/opencode) + conversation `types`
  + `opencode_runtime` + conformance tests under
  `rust/loopflow/src/lfd/conversations/`. Analytics (`usage.rs`, usage routes)
  was intentionally **not** restored.
- **Built** the per-wave in-process chat server (`conversations/server.rs`) and a
  `StreamEvent → ChatTurn` builder (`conversations/turns.rs`). `lf wave` now hosts
  it (`lf/commands/loop.rs`), discovery via `wave/<name>/.chat-endpoint`.
- **Codex-wired**: inner passes append raw `codex exec --json` to a per-wave sink
  (`LF_WAVE_EVENT_SINK`, hook in `engine/agent.rs`); a tailer folds it into turns.
- `POST /chat` → `wave/<name>/MAILBOX.md`, drained into the next pass's prompt
  (`lf/commands/goal.rs`, `<lf:mailbox>` tag).

### Executive decision: dropped the daemon-era round-trip test

`tests/session_input_round_trip.rs` tested the removed central
`/v0/conversations` daemon (ConversationManager, HttpState.conversations, store
persistence). Reviving it would mean reviving the exact central daemon the
lf-loop model replaced, so it was **not** restored. In its place:
`tests/wave_chat_server.rs` round-trips the new per-wave contract (discovery,
`/health`, `/chat`, `POST /chat`, `/chat/stream` SSE, codex ingestion).

### Still stubbed / follow-ons

- Only **codex** is wired to the live sink (claude/opencode harnesses are
  restored + conformance-tested but not driven by `lf wave` yet).
- Turn items are summarized from `StreamEvent::ToolUse` (command/file/tool); the
  richer codex **app-server** item mapping in `codex_mapping.rs` is restored but
  reserved for a future app-server driver.
- No cross-pass persistence: turns live in memory for the server's lifetime.
