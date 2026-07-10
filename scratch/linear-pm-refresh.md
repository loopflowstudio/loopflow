# Linear PM token refresh

## Problem

Linear OAuth access tokens expire after 24 hours. Loopflow stores the refresh
token, but discards the PKCE client ID and later requires both client ID and
client secret from the process environment or Doppler. PM commands wait until
expiry, swallow the refresh error, and ask the user to authenticate again.

## Approach

- Persist the non-secret OAuth client ID beside the provider token.
- Refresh OAuth tokens 20 minutes before expiry on PM access and in the existing
  background refresh trigger.
- For PKCE grants, refresh with the stored client ID and refresh token; no client
  secret is required.
- Preserve the previous refresh token when Linear omits a replacement, and
  persist rotated refresh tokens when it returns one.
- Keep a legacy fallback that resolves client credentials once, then stores the
  client ID on the refreshed row.
- If proactive refresh fails while the access token is still valid, use the
  current token and retry later. If it is expired, return a sanitized reason and
  the one-time reconnect command.

## Done when

- A Linear token inside the 20-minute refresh window is refreshed and persisted
  before a PM request uses it.
- The background token refresher can refresh Linear rows.
- PKCE refresh requests do not require a client secret.
- Rotated and temporarily omitted refresh tokens both preserve continuity.
- Legacy rows fail safely and explain the one-time reconnect when no OAuth client
  configuration is available.
- `cargo fmt`, focused Rust tests, and `cargo clippy -- -D warnings` pass.
