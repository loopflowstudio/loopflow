# 01: Loopback Auth — Consolidated Status

Remove blanket loopback trust so local and remote mutation requests require authentication.

## Security model

Protected routes now use a three-tier policy:

- **Public**: health, metrics, webhook endpoints (outside auth middleware)
- **Read** (`GET`, `HEAD`, `OPTIONS`): loopback allowed without token
- **Mutate** (`POST`, `PATCH`, `DELETE`): token required, including loopback

For `auth.provider=local`, non-loopback requests also require the same session token.

## What shipped

### Rust (`lfd`)

- Added startup session-token generation in local mode.
- Persist token to `~/.lf/session-token` with `0600` permissions on Unix.
- Added `session_token` to `HttpState`.
- Updated auth middleware to:
  - bypass auth only for loopback reads
  - require token for all mutations
  - allow remote local-mode requests only with valid token
- Added unit tests for auth tiering and local token authorization paths.
- Added session-token generation/persistence tests (hex format + file mode).

### Python (`lfq`)

- `LFD_TOKEN` remains highest precedence.
- If unset and base URL is local (`127.0.0.1`, `localhost`, `::1`), reads `~/.lf/session-token`.
- Does not read/send local token file for non-local base URLs.
- Added tests for precedence, file fallback, missing file, and remote non-fallback.

### Swift (Concerto / LoopflowCore)

- Added `FileTokenProvider` for `~/.lf/session-token` discovery.
- Wired local services to resolve tokens from:
  1. explicit provider
  2. static connection token
  3. local session-token file fallback
- Added unit tests for token-file reads and async token resolution.

### Docs

- Updated `docs/lfd.md` auth behavior to match shipped policy.
- Marked wave roadmap item `wave/security/README.md` phase 01 as in-progress.

## Key decisions kept

- Method-based read vs mutate classification in middleware.
- Startup-generated local session token (no static manual token setup).
- Local-only token-file fallback in clients to avoid remote credential leakage.
- Static/studio auth-provider validation flow left intact in this phase.

## Remaining work

1. **Manual Concerto verification** (still pending):
   - connect Concerto to local `lfd`
   - create and run a wave successfully using file-based token discovery
2. Optional: run explicit curl checks to confirm loopback read/mutate behavior end-to-end.

## Known residuals (intentionally out of scope for phase 01)

- No websocket message-level auth (HTTP handshake only).
- No auth-failure rate limiting / body-size limits.
- No TLS/mTLS changes.
- No static/studio loopback hardening beyond current read/mutate split.
