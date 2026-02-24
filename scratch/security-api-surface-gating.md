# 04: API Surface Gating

## Problem

lfd’s HTTP and WebSocket surface is still permissive in ways that can be abused: oversized payloads can burn resources, raw internal errors can leak paths/tokens, and outbound HTTP calls can accidentally carry auth headers across host boundaries.

Who benefits:
- Operators running lfd on non-loopback addresses (fewer crash/leak paths)
- Local users relying on session-token auth (token stays local)
- Remote users behind reverse proxies (spoofed forwarded headers no longer influence trust)

Why now: phases 01–03 locked down auth and container runtime; the biggest remaining risk is API-envelope hardening and credential leakage at boundaries.

## Approach

Ship one “API guardrail” layer applied to all lfd HTTP/WS traffic, with strict defaults and explicit config knobs.

1. **Add `ApiSecurityConfig` to `lfd::config` (YAML + env overrides).**
   - `http.max_json_body_bytes = 1_048_576` (1 MiB)
   - `http.max_hook_body_bytes = 262_144` (256 KiB)
   - `http.max_ws_frame_bytes = 65_536` (64 KiB)
   - `http.max_ws_message_bytes = 262_144` (256 KiB)
   - `http.max_ws_queue = 256`
   - `http.max_ws_malformed = 3`
   - `http.auth_failures_per_minute = 12`
   - `http.trusted_proxy_cidrs = []` (fail-closed default)

2. **Inbound hardening in router/middleware.**
   - Apply global body cap to `/v0/*`, `/status`, `/ws` handshake paths.
   - Apply stricter body cap to `/hooks/git` and `/v0/hooks/github`.
   - Add auth-failure throttle middleware keyed by **(resolved client source, auth context hash, endpoint group)**.
   - Return deterministic 4xx errors (`413` for size, `429` for throttle) with sanitized payloads.

3. **WebSocket gating in `routes/ws.rs`.**
   - Set handshake message/frame caps from config.
   - Insert per-connection bounded outbound queue (`max_ws_queue`); overflow disconnects client.
   - Count malformed inbound messages (non-text/non-pong or invalid envelope) and disconnect after `max_ws_malformed`.

4. **Centralized error sanitization path.**
   - Replace raw `err.to_string()` HTTP responses with a shared mapper that redacts:
     - filesystem paths under repo/worktree/home
     - bearer/static/session token-like substrings
     - internal host/volume identifiers
   - Keep detailed errors in structured logs only, and log sanitized request metadata.

5. **Safe outbound HTTP client for `github.rs` and `registration.rs`.**
   - Introduce `lfd::http_client::SafeHttpClient`.
   - Allow only `http`/`https` schemes.
   - Disable automatic redirects; manually follow up to 5 hops.
   - On host change, strip sensitive headers (`Authorization`, `Cookie`, and internal token headers) before follow-up request.
   - Reuse this client for all authenticated outbound calls.

6. **Proxy trust guardrails.**
   - Resolve source IP from socket peer by default.
   - Honor `X-Forwarded-*` only when peer IP matches configured trusted CIDR.
   - If headers are malformed or trust is ambiguous, fall back to peer IP (never trust forwarded values).

7. **Regression coverage across Rust + clients.**
   - Rust: oversized body/frame rejection, WS malformed disconnect, 429 throttle behavior, redirect host-change strip, sanitized error payloads, proxy CIDR trust behavior.
   - Python + Swift: confirm session-token fallback is local-only and no bearer token leaks on remote URLs/redirects.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Per-route hand-written limits and checks | Fine-grained control | Repeats logic, drifts quickly, misses new endpoints |
| Trust `X-Forwarded-*` by default | Easier reverse-proxy setup | Violates fail-closed security invariant; spoofable in direct-connect paths |
| Rely on reqwest default redirect behavior | Less code | Security properties are implicit and hard to test/guarantee |

## Key decisions

- We are enforcing defaults with explicit opt-out knobs, not optional middleware. This follows the wave invariant: **“Fail closed on auth/trust ambiguity.”**
- Throttling keys are hybrid (source + auth context) to avoid punishing NAT-heavy environments while still slowing brute force.
- Outbound HTTP goes through one safe client abstraction so we can prove the invariant: **“No cross-host auth header forwarding.”**
- Limits and throttles are applied by shared API boundary, not route tags, following the post-ship direction: **“Method-tier auth beat route tagging.”**

## Scope

- In scope:
  - HTTP body caps (global + hooks)
  - Auth-failure throttling for auth-sensitive entry points
  - WebSocket frame/message/queue/malformed caps
  - Central sanitized API error mapper
  - Safe outbound HTTP redirect/scheme/header handling
  - Trusted proxy CIDR gating for forwarded headers
  - Regression tests in Rust, plus token-leak prevention checks in Python/Swift clients
- Out of scope:
  - Business authorization policy changes (phase 06)
  - Full IAM/key-tier model (phase 05+remote)
  - Host compromise protections
  - SSRF policy for future URL-fetching endpoints

## Done when

- `cargo test -p loopflow -- lfd::http:: lfd::auth:: lfd::github:: lfd::registration::`
- `uv run pytest python/tests/test_client.py -k token`
- `swift test --package-path swift --filter FileTokenProviderTests`
- Plus observable checks:
  - Oversized JSON/WS payloads get predictable `413`/disconnect behavior.
  - Cross-host redirect test fixture proves sensitive headers are stripped.
  - Error responses never include repo/worktree paths or token strings.
  - `X-Forwarded-*` affects source identity only when peer IP is in trusted CIDRs.
