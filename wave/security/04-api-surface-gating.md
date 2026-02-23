# 04: API Surface Gating

Constrain the lfd HTTP/WebSocket surface so malformed, oversized, or cross-boundary requests fail safely.

## What exists after this

- Mutation endpoints are rate limited and size bounded.
- Error payloads are sanitized (no internal paths, tokens, or stack traces).
- WebSocket handshake and message envelopes are bounded.
- Outbound HTTP behavior cannot leak credentials across host boundaries.
- Forwarded headers are trusted only from configured proxy CIDRs.

## What we learned from phases 01–03

- Method-tier auth (`read` vs `mutate`) is simpler than route tagging. This phase should reuse those tiers for throttling and caps where possible.
- Session-token file fallback is now in Python and Swift local clients; leakage tests must cover both local session tokens and static tokens.
- Path traversal defenses are already centralized in `lfd::security`, so this phase should not add duplicate path-validation code.
- Error sanitization should cover internal identifiers (volume names, worktree paths) — these shouldn't appear in error responses.

## Security boundary for this phase

This phase prevents common API-surface failures:

- Oversized/malformed requests causing avoidable service instability.
- Internal error detail disclosure through HTTP responses.
- Internal auth material being forwarded to unintended hosts.
- Blind trust of spoofable forwarded headers.

This phase does not provide:

- Business-logic authorization guarantees by itself (covered by auth phases).
- Protection from host-level compromise.

## Scope

### Inbound hardening

- Global body size caps for JSON endpoints.
- Stricter caps for high-risk surfaces (`/hooks/*`, future file APIs).
- Per-client auth-failure throttling on hook/auth endpoints, keyed by source and auth context.
- WebSocket caps:
  - max frame/message size
  - max queued outbound events per client
  - disconnect on repeated malformed payloads

### Error and logging hardening

- Centralized error mapping for 4xx/5xx with redaction.
- No raw panic/backtrace output in HTTP responses.
- Header/log sanitization for untrusted request metadata.

### Outbound leakage controls (new priority)

- Strip `Authorization`, cookie-like, and internal auth headers on:
  - redirect to different host
  - explicit cross-host outbound requests
- Refuse unsafe URL schemes by default.
- Add tests proving no token leaks to:
  - redirected hosts
  - status endpoints
  - error payloads/log snapshots

### Proxy-trust guardrails

- Ignore `X-Forwarded-*` unless source IP is in configured trusted proxy CIDRs.
- Default behavior is fail-closed (no implicit proxy trust).

## Done when

- Body/frame limits are enforced with test coverage.
- Outbound redirect/cross-host header stripping is enforced with tests.
- Sanitized error responses are consistent across endpoints.
- Proxy header trust is CIDR-gated and covered by regression tests.

## Open questions

- Should default throttling keys be source IP only, token hash only, or a hybrid (for NAT-heavy deployments)?
- What body/frame limits are safe for current webhook payloads without breaking legitimate use?
- On WebSocket pressure, should we drop oldest queued events or disconnect immediately?

## Checkpoints

1. Body and WebSocket caps enforced with clear 4xx responses.
2. Error/log redaction centralized so endpoints share one sanitization path.
3. Cross-host redirect/header stripping and proxy CIDR trust rules covered by regression tests.

## Try it

- Fire oversized JSON and WebSocket payloads from a local script and verify predictable rejection behavior.
- Simulate outbound redirect to a different host and confirm `Authorization` headers are stripped.
- Send malformed requests repeatedly and verify throttling triggers before service instability.

## What might change

- If proxy deployments need richer trust modeling, CIDR-only rules may be split from this phase into a focused follow-up.
- If real workloads require larger WS queues, caps may become configurable with conservative defaults.
