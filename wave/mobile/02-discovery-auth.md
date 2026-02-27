# 02: Discovery Auth Modes

Harden discovery with dual auth, connection tokens, and a desktop toggle so mobile connects securely without studio acting as a relay.

## Carried context

- Discovery is shipped: lfd sends `url` + `repos` in register/heartbeat, mobile discovers via studio and connects directly.
- Address detection currently shells out to `tailscale status --json`. Follow-up should migrate to Tailscale LocalAPI for reliability.
- Studio `validate-connection` endpoint exists (studio repo). Connection tokens are HMAC-signed, 10-minute expiry, studio-issued.
- `xcodebuild test -scheme Concerto` fails locally during ConcertoUITests link (`open() failed, errno=1`). Swift package tests pass. May be environment-specific.

## What to build

1. **Dual auth mode (`StudioWithLocal`)**
   - Static token accepted from loopback, studio connection tokens accepted from anywhere.
   - Concerto macOS uses static token locally. Mobile uses connection tokens.
   - Studio going down doesn't break local access.

2. **Connection token protocol**
   - lfd mints opaque 256-bit tokens, sends pool to studio on registration.
   - Studio hands tokens to authenticated mobile users via discover.
   - lfd validates locally via in-memory ledger (no HTTP round-trip on connect).
   - Tokens are single-use, short-lived (1 hour), pruned on expiry.
   - Pool management: lfd controls replenishment threshold, studio reports `tokens_remaining` in heartbeat.

3. **Desktop toggle: "Connect with my phone"**
   - Setting in Concerto macOS, off by default.
   - On: prompt studio sign-in if needed, restart lfd with dual auth + `0.0.0.0` bind.
   - Off: restart lfd with static auth + `127.0.0.1` bind, deregister from studio.

4. **Token revocation**
   - `lfq token revoke <prefix>` and `lfq token revoke --all`.
   - Heartbeat `revoke` field for studio-initiated revocation.
   - Auto-revoke on suspicious patterns (same token from multiple IPs).

5. **TLS enforcement**
   - lfd refuses studio auth on non-loopback, non-Tailscale bind over HTTP.
   - `advertise_url` config for reverse proxy setups.
   - `--allow-insecure-bind` escape hatch.

6. **Container mode default**
   - `mode: container` defaults `auth.provider` to `loopflow.studio`.

## Studio-side dependencies

These items live in the studio repo and should be coordinated:
- Token pool endpoints (receive pool from lfd, hand out on discover)
- Discover rate limiting per user per daemon
- Heartbeat `tokens_remaining` + `revoke` fields

## Constraints

- Discovery remains additive. Manual connection always available.
- Local static-token connections unaffected by studio outage.
- No per-repo or per-capability token scoping in v1.

## Done when

- Dual auth mode works: static token locally, connection token remotely.
- Desktop toggle restarts lfd with correct auth + bind configuration.
- lfd mints, pools, and validates connection tokens via local ledger.
- Token revocation works via CLI and heartbeat.
- TLS enforcement prevents insecure remote binds.
- Existing tests pass, new tests cover token lifecycle and dual auth.
