# OpenCode session harness review

## What was implemented

- Added a full `OpenCodeHarness` that runs `opencode serve` on an ephemeral local port and communicates over HTTP + SSE.
- Added OpenCode bus event mapping (`opencode_mapping.rs`) into canonical `SessionEvent` types (`TurnStarted`, `TextDelta`, `ItemStarted`, `ItemCompleted`, `TurnCompleted`, `DiffUpdated`, `Error`).
- Registered `opencode` in harness parsing/creation (`HarnessKind`) and terminal harness error detection (`opencode_disconnected`).
- Updated session resolution so `harness: "opencode"` is accepted (removed prior not-implemented rejection).
- Added/expanded unit tests for:
  - harness resolution and terminal-error classification
  - OpenCode SSE parsing and session-id parsing
  - status transitions, permission handling, tool lifecycle mapping
  - nested session-id payload shapes and error-to-failed-turn completion
- Updated daemon docs (`docs/lfd.md`) to list `opencode` as a supported sessions harness.

## Key choices

- **Long-lived server per session**: Spawn one `opencode serve` child and communicate only via HTTP/SSE. This avoids transport-specific assumptions in the session API.
- **Turn boundaries from status transitions**: OpenCode does not emit explicit turn lifecycle events, so mapping relies on `session.status` (`active`/`idle`/`error`).
- **Permission auto-approval**: `permission.asked` events are automatically approved via HTTP to preserve non-interactive session flow.
- **Coarse tool typing with safe fallback**: Tool parts map to `Command`/`File` when obvious fields exist, otherwise generic `Tool`.
- **Defensive parsing**: Mapping now accepts top-level and nested session-id/state shapes and explicitly completes active turns as failed on `session.error`.

## How it fits together

`SessionManager` creates an OpenCode harness when `harness: "opencode"` is requested. The harness starts `opencode serve`, creates a provider session, and streams `/event` SSE frames through `opencode_mapping` into normalized `SessionEvent`s that the existing session event bridge persists and broadcasts. No session API shape changes were required.

## Risks and bottlenecks

- **Schema drift risk**: OpenCode event payload shape is inferred heuristically; unknown future payload variants may map less precisely.
- **SSE disconnect sensitivity**: A dropped event stream emits terminal `opencode_disconnected`, which intentionally fails the session.
- **Port allocation race window**: Ephemeral port reservation via bind-to-0 then spawn has a small race window (standard tradeoff).
- **No real-binary integration test in this diff**: Coverage is strong at unit level but does not execute against a live OpenCode daemon in CI here.

## What's not included

- No OpenCode-specific model/session configuration extensions beyond existing generic `SessionConfig` fields.
- No change to session API contracts or transport-neutral event schema.
- No OpenCode end-to-end integration harness test with a real `opencode` binary.
- No changes outside the branch scope (e.g., unrelated executor/docker behavior).

## Validation run

- `cargo fmt --all`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow lfd::sessions:: -- --nocapture`
- `cargo test -p loopflow opencode -- --nocapture`
- `cargo test --all` *(fails in this environment due missing `/var/run/docker.sock` for two docker startup tests; unrelated to session harness changes)*
