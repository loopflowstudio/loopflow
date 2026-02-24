# 02: Claude Adapter

Second adapter using `claude -p --resume` with NDJSON streaming. Validates that the adapter abstraction works for a non-JSON-RPC provider.

## What exists after this

Claude interactive sessions work through the same session API as Codex. lfd spawns Claude with `-p --output-format stream-json`, translates NDJSON events into canonical SessionEvents, and persists everything. Second turn onwards uses `--resume` with the persisted provider session id.

Detailed design: `scratch/agentapi-claude-adapter.md`

## What Phase 01 taught us

**Adapter wiring is trivial.** `CreateAdapterFn` is a function pointer — adding Claude means adding one match arm and implementing the three-method trait. No factory ceremony.

**Event normalization is the real work.** The Codex adapter spent most of its 630 lines normalizing JSON-RPC payloads. Claude's NDJSON events are better documented but need similar mapping — especially tool name → item type inference (`Bash` → `Command`, `Edit`/`Write` → `File`, everything else → `Tool`).

**Item schema is ready.** The normalized types (`Command`, `File`, `Message`, `Thought`, `Tool`) were designed with Claude in mind. No schema changes expected.

**Single in-flight turn is already enforced.** The session manager rejects concurrent input. Claude's process-per-turn model fits naturally — each `send_input()` spawns a process, and the next can't start until it exits.

## What to build

- Claude adapter implementing `SessionAdapter` trait
- NDJSON stream parser (`stream_event.event` unwrapping)
- Event mapping: content deltas, tool lifecycle, turn boundaries
- Provider session id persistence for `--resume` continuity
- Config passthrough: `yolo_mode`, `max_turns`, `system_prompt`, `model`, `cwd`

## Open questions

- Does `--resume` preserve full conversation context across many turns, or does it degrade?
- What's the process-per-turn startup latency? Is it acceptable for interactive use?
- Does `--append-system-prompt` keep Claude conversational in `-p` mode?

## Done when

- `provider: "claude"` session can be created and reaches `active`
- Input spawns Claude process and events appear via SSE replay/live stream
- Second turn resumes with persisted provider session id
- Concurrent input while turn is active returns conflict/busy
- `DELETE /sessions/{id}` stops any in-flight Claude process
- `cargo test --all` and `cargo clippy -- -D warnings` pass
