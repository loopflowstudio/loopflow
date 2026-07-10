# Linear PM token refresh — validation

Design and rationale are folded into `wave/infrastructure/MEMORY.md`
(Shipped + Planning-model). Verify the change:

## Done when

- A Linear token inside the 20-minute refresh window is refreshed and persisted
  before a PM request uses it.
- The background token refresher can refresh Linear rows.
- PKCE refresh requests do not require a client secret.
- Rotated and temporarily omitted refresh tokens both preserve continuity.
- Legacy rows fail safely and explain the one-time reconnect when no OAuth client
  configuration is available.
- `cargo fmt`, focused Rust tests (`cargo test -p loopflow provider_auth token_refresh`),
  and `cargo clippy -- -D warnings` pass.
