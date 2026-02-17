# 04: API Surface Gating

Constrain the lfd HTTP/WebSocket surface so malformed, oversized, or cross-boundary requests fail safely.

## What exists after this

- Mutation endpoints are rate limited and size bounded.
- Error payloads are sanitized (no internal paths, tokens, or stack traces).
- WebSocket handshake and message envelopes are bounded.
- Outbound HTTP behavior cannot leak credentials across host boundaries.
- Forwarded headers are trusted only from configured proxy CIDRs.

## Scope

### Inbound hardening

- Global body size caps for JSON endpoints.
- Stricter caps for high-risk surfaces (`/hooks/*`, future file APIs).
- Per-client auth-failure throttling on hook/auth endpoints.
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
