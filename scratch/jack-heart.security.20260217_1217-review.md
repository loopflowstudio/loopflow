# Gate Review — jack-heart.security.20260217_1217

## What was implemented

- Added loopback-auth hardening in `lfd`:
  - Local-mode startup now generates a random session token and writes it to `~/.lf/session-token`.
  - Auth middleware now allows loopback reads without a token but requires a token for mutations.
  - `HttpState` now carries `session_token` for middleware checks.
- Added token persistence module (`rust/loopflow/src/lfd/session_token.rs`) with token-format and permission tests.
- Updated Python client token resolution:
  - `LFD_TOKEN` still has highest priority.
  - Falls back to `~/.lf/session-token` for local daemon URLs.
  - Avoids sending local session tokens to non-local base URLs.
- Added Python tests for token resolution precedence, file fallback, and remote non-fallback behavior.
- Added Swift `FileTokenProvider` and wired local services to auto-resolve local tokens.
- Added Swift unit tests for token file reads.
- Updated docs to match shipped auth behavior (`docs/lfd.md`), and restored `wave/security/01-loopback-auth.md` so roadmap links remain valid.

## Key choices

- **Method-based route tiering in middleware**: classify reads vs mutations by HTTP method (`GET/HEAD/OPTIONS` vs others) instead of maintaining a per-route auth table.
- **Startup-generated session token for local mode**: no manual token config required, token rotates on daemon restart.
- **Local-only file-token fallback in Python**: keeps local UX automatic while avoiding credential leakage to remote hosts.
- **Preserved static/studio provider paths**: token validation logic for those providers remains intact; only loopback bypass semantics changed by read/mutate tiering.

## How it fits together

`lfd` startup chooses an `AuthProvider`; when provider is `Local`, it generates and stores a session token and places it in `HttpState`. `auth_middleware` checks whether a request is a loopback read (allow) or requires auth, then validates bearer tokens according to provider rules. Python (`lfq`) and Swift (Concerto services) now auto-discover the same local session token file for transparent local mutation auth.

## Risks and bottlenecks

- **Manual Concerto verification still pending**: code-level tests pass, but end-to-end local create/run from the app still needs a manual check.
- **Auth semantics changed for loopback mutations**: any local scripts that relied on mutation without auth now must supply the token.
- **Single-session token model**: token rotates only on daemon restart (intentional for this phase), so live rotation is still out of scope.

## What's not included

- No changes to TLS/mTLS transport.
- No websocket message-level auth (HTTP handshake only).
- No auth-failure rate limiting or request-size limiting.
- No static/studio provider isolation hardening beyond this phase’s read/mutate split.
