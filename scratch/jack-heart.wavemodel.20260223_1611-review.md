# API surface gating review (`jack-heart.wavemodel.20260223_1611`)

## What was implemented

- Added a new `api_security` config surface in `lfd` with strict defaults and env overrides for:
  - HTTP body limits (global + hook-specific)
  - WebSocket frame/message/queue/malformed limits
  - auth-failure throttling
  - trusted proxy CIDR allowlist
- Applied body caps at the router boundary:
  - `/v0/*`, `/status`, `/ws` use global JSON limit
  - `/hooks/git` and `/v0/hooks/github` use stricter hook limit
  - Added a shared `413` normalization middleware to return deterministic JSON errors.
- Hardened auth middleware:
  - Added per-minute auth failure throttle keyed by `(resolved source IP, auth context hash, endpoint group)`.
  - Added trusted proxy source resolution (only honors `X-Forwarded-For` when peer IP is in configured CIDRs).
  - Ensured studio-provider missing-token failures now flow through throttle accounting instead of bypassing it.
- Hardened WebSocket handling:
  - handshake/message/frame caps from config
  - bounded outbound queue with disconnect-on-overflow behavior
  - malformed inbound counter with disconnect after configured threshold
- Added centralized API error sanitization:
  - redacts known paths, bearer/token-like substrings, and internal identifiers in response payloads
  - preserves raw details for logs.
- Introduced `SafeHttpClient` and reused it for authenticated outbound calls (`github.rs`, `registration.rs`):
  - only `http`/`https`
  - redirects disabled by default and followed manually (max 5)
  - strips sensitive headers on redirect authority change
  - fixed replayability behavior so non-replayable requests are only rejected if an actual redirect must be followed.
- Added/updated tests across Rust, Python, and Swift for token handling, redirect behavior, proxy trust, limits, and WS guards.
- Updated `docs/lfd.md` with new `LFD_HTTP_*` env vars and `api_security.http` YAML config examples.

## Key choices

- **Fail-closed defaults:** all new security knobs default to restrictive values (`trusted_proxy_cidrs` empty, explicit byte caps, throttle enabled).
- **Boundary-layer enforcement:** limits and sanitization are applied in shared router/middleware paths instead of per-handler custom checks.
- **Unified outbound safety client:** redirect and header-leak rules are centralized in `SafeHttpClient` and shared by GitHub + registration flows.
- **Throttle key shape:** hybrid key avoids penalizing all users behind one NAT while still slowing repeated auth abuse.

## How it fits together

`LfdConfig` now resolves `api_security.http`, `bin/lfd` injects this into `HttpState`, and all ingress/egress boundaries consume the same security policy. Incoming requests hit body-limit + auth/throttle + sanitized error paths; WebSocket connections inherit configured frame/message/queue/malformed caps; outbound authenticated HTTP goes through `SafeHttpClient` redirect rules to prevent cross-host credential forwarding.

## Risks and bottlenecks

- `cargo test --all` remains intermittently flaky on `wave_rename_renames_branch` (existing unrelated test instability); targeted security suites pass consistently.
- Concerto UI `xcodebuild test` failed locally in `ScreenshotPipelineTests.testCapture` due app activation state (`Running Background`), indicating environment sensitivity for that UI test path.
- Error sanitization is heuristic by design; false positives can reduce message specificity, and false negatives are still possible for novel token formats.

## What's not included

- No changes to business authorization policy or IAM model.
- No expansion of SSRF policy beyond outbound redirect/scheme/header safety for existing registration/GitHub clients.
- No attempt to fix unrelated flaky tests/UI harness instability in this branch.
