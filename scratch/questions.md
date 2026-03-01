# Open questions / assumptions

- `lfq token revoke <prefix>` is implemented as **token hash prefix** revocation (SHA-256 hex), not raw-token prefix, because raw tokens are never stored.
- In `auth.provider=dual` with postgres storage, the token ledger persists to `~/.lf/connection_tokens.db` (SQLite sidecar) to keep ledger semantics and avoid coupling to postgres schema.
- WebSocket re-validation checks the token every 60s and closes with code `4401` on failure.
