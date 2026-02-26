# Metering infra review — `jack-heart.cost.20260226_0920`

## What was implemented
- Added two new persisted session event payloads in Rust: `TurnUsage` and `ContextSnapshot`.
- Extended `SessionEvent` with `TurnUsage { turn_id, usage }` and `ContextSnapshot { snapshot }`, including new `event_type()` mappings (`turn_usage`, `context_snapshot`).
- Wired token-usage extraction into all three harnesses:
  - Claude (`result` events)
  - Codex (`turn/completed` params)
  - OpenCode (`session.status` idle transition usage block)
- Updated session startup to emit a `ContextSnapshot` event before harness startup.
- Added/extended tests for parsing, serde round-trips, and prompt-breakdown conversion.

## Key choices
- **Optional provider-specific fields**: `TurnUsage` keeps optional fields (`reasoning_tokens`, cache tokens, model, cost) to avoid brittle assumptions across providers.
- **Stable serialized source keys**: `ContextSnapshot.sources` uses `HashMap<String, u64>` with explicit key mapping instead of serializing `DocumentSource` directly.
- **Emit usage after completion**: `TurnUsage` is emitted immediately after `TurnCompleted` to keep turn boundary semantics clear for consumers.
- **No schema migration**: New data rides in existing `session_events.data` JSON payloads.

## How it fits together
`prepare_session_prompt()` now returns `ContextBreakdown`, and `create_session()` persists that as one `context_snapshot` runtime event before harness startup. During runtime, each harness maps provider-native completion payloads into a normalized `TurnUsage` struct and emits `turn_usage` after `turn_completed`. The existing event bridge/store path persists both new variants without additional routing changes.

## Risks and bottlenecks
- Provider payload schemas can drift; unknown/missing fields currently degrade to `0`/`None` rather than failing fast.
- `ContextSnapshot.budget` currently reflects `DEFAULT_CONTEXT_BUDGET`; if per-session budgeting is introduced, this field may need to source dynamic config.
- OpenCode usage capture depends on status transition semantics (`active -> idle` with usage block).

## What's not included
- No new HTTP endpoints or response-shape docs.
- No database/schema migration.
- No analytics aggregation/UI work (covered by follow-on wave items).

## Validation run
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py -v`

### Notes on environment-specific failures during validation
- `.lf/config.yaml` test command failed on this machine when running:
  - `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'`
- Failure reason: missing local `Mac Development` signing cert.
- Re-running with CI-aligned flags avoided signing issues:
  - `CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
- A subsequent `xcodebuild` run still failed in `ConcertoUITests-Runner` due early runner exit/connection bootstrap issues (`127.0.0.1:2486` refused), while unit/smoke suites passed.
