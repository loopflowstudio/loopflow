# OpenCode adapter (current state)

OpenCode is now the third sessions harness in `lfd`, proving the session API works across three transport models:

- Claude: NDJSON over stdio
- Codex: JSON-RPC over stdio
- OpenCode: HTTP + SSE via `opencode serve`

## What is implemented

- `OpenCodeHarness` spawns one long-lived `opencode serve` process per session on an ephemeral local port.
- Harness communication is HTTP-only (session create/message/permission/abort/delete) with SSE subscription to `/event`.
- OpenCode bus events are mapped into canonical `SessionEvent` values in `opencode_mapping.rs`.
- `harness: "opencode"` is now accepted by session resolution and harness creation.
- Terminal harness error classification includes `opencode_disconnected`.
- `docs/lfd.md` lists OpenCode as a supported sessions harness.

## Mapping and lifecycle behavior

- Turn boundaries are inferred from `session.status` transitions (`idle -> active -> idle`).
- `message.part.updated` maps text/reasoning/tool parts to deltas and item lifecycle events.
- `permission.asked` is auto-approved to preserve non-interactive flow.
- `session.diff` maps to `DiffUpdated`.
- `session.error` emits `Error` and fails any active turn.
- Session-id parsing is defensive across payload shape variants (`id`, `sessionID`, `sessionId`, nested forms).

## Validation completed

- `cargo fmt --all`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow lfd::sessions:: -- --nocapture`
- `cargo test -p loopflow opencode -- --nocapture`

Note: `cargo test --all` in this environment still fails unrelated docker startup tests due to missing `/var/run/docker.sock`.

## Known constraints and risks

- Mapping depends on inferred OpenCode event schema and may need updates if upstream payloads drift.
- SSE disconnects are treated as terminal (`opencode_disconnected`) and intentionally fail the session.
- Ephemeral port reservation uses a standard bind-to-0 then spawn pattern with a small race window.
- CI coverage is unit-level for mapping/harness logic; there is no real-binary OpenCode integration test in this branch.

## Outstanding questions

See `scratch/questions.md` for unresolved API-shape confirmations still worth validating against a live OpenCode server.
