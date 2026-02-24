# 04: API Surface Gating

## Current state

Phase 04 API surface hardening is implemented on branch `jack-heart.wavemodel.20260223_1611`.

The `lfd` HTTP and WebSocket boundary now enforces strict default limits, fail-closed proxy trust, sanitized client-facing errors, and safe outbound redirect handling for authenticated requests.

## What shipped

### 1) Configurable security envelope (`api_security.http`)

Default policy now lives in `lfd::config` with YAML + env overrides:

- `max_json_body_bytes = 1_048_576`
- `max_hook_body_bytes = 262_144`
- `max_ws_frame_bytes = 65_536`
- `max_ws_message_bytes = 262_144`
- `max_ws_queue = 256`
- `max_ws_malformed = 3`
- `auth_failures_per_minute = 12`
- `trusted_proxy_cidrs = []` (fail-closed by default)

### 2) Inbound HTTP hardening

- Global body caps applied to `/v0/*`, `/status`, and `/ws` handshake paths.
- Stricter hook cap applied to `/hooks/git` and `/v0/hooks/github`.
- Auth-failure throttle added with key: `(resolved source IP, auth context hash, endpoint group)`.
- Deterministic `413` and `429` responses returned with sanitized payloads.

### 3) WebSocket gating

- Frame/message limits sourced from config.
- Per-connection bounded outbound queue; overflow disconnects the client.
- Malformed inbound message counter; disconnect after configured threshold.

### 4) Centralized error sanitization

HTTP error payloads redact:

- repo/worktree/home filesystem paths
- bearer/static/session token-like substrings
- internal host/volume identifiers

Detailed raw errors stay in structured logs.

### 5) Safe outbound HTTP redirect handling

`SafeHttpClient` now fronts authenticated outbound calls (`github.rs`, `registration.rs`):

- allows only `http`/`https`
- disables automatic redirects and follows manually (max 5)
- strips sensitive headers on redirect authority change
- rejects non-replayable redirect follow-ups only when an actual follow is required

### 6) Trusted proxy guardrails

- Source identity defaults to socket peer IP.
- `X-Forwarded-*` is honored only when peer IP is in configured trusted CIDRs.
- Malformed/ambiguous forwarded headers fall back to peer IP.

### 7) Coverage and docs

- Added regression tests for body/frame caps, auth throttling, proxy trust, WS malformed behavior, redirect header stripping, and sanitized error payloads.
- Added client token-leak prevention checks in Python and Swift suites.
- Updated `docs/lfd.md` with new `LFD_HTTP_*` settings and `api_security.http` examples.

## Security invariants now enforced

- Fail closed on auth/trust ambiguity.
- No cross-host forwarding of sensitive auth headers.
- API size abuse gets deterministic rejection behavior.
- Forwarded client identity is trusted only behind explicit proxy allowlists.

## Validation summary

Targeted Phase 04 test suites pass:

- `cargo test -p loopflow -- lfd::http:: lfd::auth:: lfd::github:: lfd::registration::`
- `uv run pytest python/tests/test_client.py -k token`
- `swift test --package-path swift --filter FileTokenProviderTests`

Known unrelated instability during full-suite runs:

- Intermittent `cargo test --all` failure at `wave_rename_renames_branch`.
- Environment-sensitive Concerto UI failure in `ScreenshotPipelineTests.testCapture` (app foreground activation).

## Remaining follow-ups

1. Decide whether to fix or quarantine the unrelated flaky Rust test before merge.
2. Stabilize Concerto screenshot pipeline activation behavior in CI/local headless runs.
3. Keep expanding sanitizer regression cases as new token/path formats appear.
