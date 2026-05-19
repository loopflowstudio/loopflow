---
status: in-progress
claimed_by: 571591d4-5ce2-4739-9b77-86372a58bc8f
claimed_at: 2026-05-19T01:05:20.697842Z
asana_id: '1214269992290477'
---
# Remote lfd connection

## Problem

The phone needs to talk to an lfd running somewhere that isn't the phone — a
Mac Mini at home, a cloud VM, a studio-registered daemon. Until now lfd has
been local-only in daily use. Without a connection that is *simple to
establish* and *boring to maintain*, the entire mobile read surface collapses
before the first wave card renders.

Who benefits: the conductor who wants to glance at work from the train without
opening a laptop. Why now: it gates `see-your-waves` and `see-wave-tasks` —
nothing on the phone works until this does.

The surprising finding from research: **the hard parts are already built.**
Keychain storage, certificate pinning, a reconnecting WebSocket with
`NWPathMonitor`, a `ConnectionStore` that persists config, and a studio-OAuth
daemon discovery flow all exist today. What's missing is the *first thirty
seconds* (there is no QR path and no manual URL+token entry on iOS), a token
that lives longer than **one hour**, and resilience across app suspension and
token expiry. This design targets exactly those gaps and resists rebuilding
what works.

## Approach

One first-run screen with three ways in, ordered by how little the user has to
do:

1. **Scan QR** (hero). Laptop/host runs `lf op pair`, which prints a QR to the
   terminal. Phone scans it and is connected. Zero typing.
2. **Paste link** (fallback). The QR encodes a `lfd://pair?…` URL. The same
   URL pasted into a text field — or opened as a deep link — drives the
   identical code path. This is also the headless-testable path.
3. **Sign in with Loopflow** (zero-config for studio users). The existing
   `DiscoveryService` → `loopflow.studio` OAuth → daemon list. Already built;
   surface it as the third option.

`lf op pair` is the one new piece of host-side machinery. It:

- ensures `auth.mode = studio` with a token ledger present,
- mints a **pairing-class token** with a long TTL (see De-risking),
- resolves the best reachable host: a Tailscale IP (`100.64.0.0/10`) if the
  tailnet is up, else a configured public URL, else LAN address with a loud
  warning,
- captures the serving cert fingerprint when TLS terminates in front of lfd,
- renders `lfd://pair?host=…&port=…&tls=…&token=…&fp=…` as a terminal QR.

The phone decodes that payload, writes the token to Keychain keyed by
`host:port` (existing `ConnectionSecretStore`), pins `fp` if present, builds a
`ServerConnection`, and connects through the existing HTTP + WebSocket
services. Pinning the fingerprint *from the QR* is strictly better than
trust-on-first-use over the network: the QR arrives over a trusted side channel
(your own screen, in the room), so the phone never has to blindly trust a first
TLS handshake.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does a connection token survive until tomorrow morning? | **No.** `DEFAULT_TOKEN_TTL = 60 * 60` (1 hour), hard-coded in `rust/loopflow/src/lfd/token_ledger.rs:18`. Studio keeps a *pool* of 5 tokens replenished by the daemon's heartbeat (`registration.rs:22-23`); the phone re-acquires a fresh one each discovery. | A 1-hour token makes "open app, it's already connected" false for the QR/paste path. **Decision: add a pairing-class token with a long, sliding TTL.** Studio path is fine as-is — the durable phone credential there is the studio OAuth refresh token, not the lfd token; expiry → silent re-discovery. |
| Does lfd serve TLS, so "TLS by default" is literally true? | **No.** lfd is HTTP-only — raw `TcpListener`, no rustls anywhere in the serve path (`bin/lfd.rs:309-317`). It *refuses* non-loopback binds in studio mode unless the address is in the Tailscale CIDR or `--allow-insecure-bind` is passed (`bin/lfd.rs:108-114`, `is_tailscale_ip`). | "TLS by default" is a *client* policy, not an lfd feature. **Decision: client defaults `useTLS=true`; the only permitted plaintext is a Tailscale-range host (encrypted at WireGuard L3, matching lfd's own bind guard). Never plaintext over the open internet.** Real TLS comes from a reverse proxy / studio relay in front of lfd; the existing `CertificatePinningDelegate` handles it. The design states this honestly rather than implying lfd terminates TLS. |
| Is the WebSocket token-revocation close handled on the client? | **No.** `EventService` has exponential backoff + `NWPathMonitor` (`LocalEventService.swift:151-397`) but no handling of the server's `close(4401)` on token revoke/expiry (`http/routes/ws.rs`), and no `scenePhase` awareness. | Two additions: (a) on `scenePhase == .active`, if the socket is down, reconnect *immediately* (skip backoff) — covers device sleep / app suspension; (b) on close code `4401`, branch by setup path: studio → silent re-discovery; QR/paste → surface a "session expired, re-pair" state. |
| Does any QR scanning exist? | **No.** Only `AVCaptureDevice` for *audio* in `VoiceInputService`. No `AVCaptureMetadataOutput`, no VisionKit `DataScannerViewController`. | New iOS-only view under `Concerto/Platform/iOS/`. Needs `NSCameraUsageDescription` in the iOS Info.plist. Simulator has no camera → the paste/deep-link path is the required fallback and the test path. |
| Is there manual URL+token entry on iOS? | **No.** `ConnectionSetupView` only renders `ConnectionsPanel` (provider OAuth — GitHub/Claude/etc.), not lfd host config. macOS has `ConnectionSettingsView` with URL+token fields; iOS has none. | Build the unified setup screen iOS-side; reuse `ServerConnection`, `ConnectionStore`, `ConnectionSecretStore` unchanged. Mirror macOS field semantics, don't fork the model. |
| Will multiplatform boundary checks block this? | `LoopflowCore` must not import macOS frameworks; `#if os(iOS)` only as whole-file gates under `Platform/iOS/` (`scripts/check_swift_multiplatform_boundaries.py`). | All camera/scanner UI lives in `Platform/iOS/`. Pairing-payload parsing is plain `Codable`/URL logic → goes in `LoopflowCore` (shared, testable, no platform import). |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Bump `DEFAULT_TOKEN_TTL` globally to 90 days | One-line change | Weakens *every* token including the studio pool, where short TTL is the security model. A revoked laptop shouldn't imply a 90-day window everywhere. Pairing tokens are a distinct class with a distinct risk profile. |
| Studio OAuth as the *only* path | Reuses fully-built `DiscoveryService`; no new host command | Forces a loopflow.studio account and internet round-trip for someone whose Mac Mini sits on their own tailnet. The wave README's headline daily experience is "scan a QR from laptop" — a local, accountless path. Cutting it guts the feature. |
| Add native rustls TLS to lfd serve | "TLS by default" becomes literally true | Large surface change (cert provisioning, renewal, ACME) far outside a mobile-connection item; ~1000+ LOC of its own. lfd's existing posture (Tailscale L3 or front a proxy) is deliberate and sufficient. Pin the cert from the QR instead. |
| mDNS / Bonjour host discovery on LAN | No QR, no typing on same network | Doesn't survive leaving the LAN (the entire point is the train), adds a discovery stack, and still needs a token. QR carries host *and* credential *and* fingerprint in one trusted artifact. |
| TOFU cert pinning on first network connect | No `fp` in QR payload | The QR already crosses a trusted channel; carrying the fingerprint there removes the blind-first-handshake window for free. Keep TOFU only as the fallback when `fp` is absent (Tailscale-plaintext case). |

## Key decisions

1. **Pairing-class token, long sliding TTL.** Add a pairing token path to
   `TokenLedger` (e.g. `mint_pairing` or a TTL parameter) with a 90-day TTL
   that slides forward on each successful auth. Rationale: the phone is a
   trusted personal device; re-pairing weekly is the failure the daily
   experience explicitly rules out. Revocation still works (`POST
   /v0/tokens/revoke` by hash prefix, `lfq token revoke`). Pairing tokens are
   visibly distinct in the ledger so `lfq token revoke --all` and audit stay
   meaningful.
2. **`lf op pair` is the host-side entry point.** A new ops step/command, not a
   new daemon endpoint — it runs where the user already has a terminal,
   reuses ledger minting, and prints a QR. Side-channel by nature → belongs in
   `ops/`.
3. **`lfd://pair?host&port&tls&token&fp` is the wire format.** One URL serves
   QR, paste, and deep link. Parsing lives in `LoopflowCore` as pure Codable
   logic with round-trip tests. `tls` defaults true; `fp` optional.
4. **TLS-by-default is a client policy with one explicit exception.** Default
   `useTLS=true`. Plaintext permitted *only* for `100.64.0.0/10` hosts
   (Tailscale, encrypted at L3 — mirrors `bin/lfd.rs` bind guard). Any other
   plaintext host is refused with a clear error, not a silent downgrade.
5. **Resilience is two concrete hooks, not a rewrite.** (a) `scenePhase`
   → immediate reconnect on foreground; (b) WS `close(4401)` → path-aware
   recovery (studio: silent re-discover; QR/paste: "re-pair" error state).
   Network drops are already covered by `NWPathMonitor` — leave it.
6. **Three explicit error states**, each actionable on the connection screen:
   `unreachable` (host/port wrong or down → re-check or re-pair),
   `authRejected` (token bad/expired → re-pair or sign in),
   `trustMismatch` (cert changed → block, show fingerprint diff, require
   explicit re-pair). No generic "something went wrong."

## Scope

**In scope:**
- `lf op pair` host command: mint pairing token, resolve host, render QR + print the `lfd://pair?…` URL
- Pairing-class token in `TokenLedger` (long sliding TTL) + revocation parity
- iOS unified setup screen (`Platform/iOS/`): Scan QR / Paste link / Sign in with Loopflow
- `lfd://pair` payload parsing + cert-fingerprint pin-from-QR in `LoopflowCore` (shared, tested)
- Camera permission wiring (`NSCameraUsageDescription`) + deep-link handler
- `scenePhase`-aware immediate reconnect; WS `4401` path-aware recovery
- Three concrete connection error states with actionable UI
- Reuse (unchanged): `ServerConnection`, `ConnectionStore`, `ConnectionSecretStore`, `CertificatePinningDelegate`, `EventService` reconnect, `DiscoveryService`

**Out of scope:**
- Wave list / roadmap / item rendering — `see-your-waves`, `see-wave-tasks`
- Any write/build/land/chat operation (wave is view-only by charter)
- Native rustls TLS termination inside lfd serve
- Push notifications, background refresh, Android
- mDNS/Bonjour LAN discovery

## Done when

A runnable script proves the end-to-end path without a physical phone:

`scripts/test_pairing_smoke.py` — start lfd in studio mode, run the
`lf op pair` codepath to mint a pairing token and emit the `lfd://pair?…`
URL, then drive the connection the way the phone would: parse the payload,
`GET /v0/waves` with `Authorization: Bearer <token>`, open `/ws`, assert the
`connected` snapshot arrives. Extend the existing
`scripts/test_remote_smoke.py` harness rather than duplicating its TLS/WS
plumbing.

Observable acceptance, mapped to the item's "Done when":
- Single-screen setup, all three paths reach a connected state — iOS
  Simulator via deep link (`xcrun simctl openurl … "lfd://pair?…"`), since the
  Simulator has no camera
- Token in Keychain — `ConnectionSecretStore` round-trip, survives app restart
- TLS by default — non-Tailscale plaintext host is refused with `unreachable`/
  policy error, not silently downgraded
- Recovers from sleep/drop — `scenePhase` foreground reconnects with no manual
  action; `NWPathMonitor` drop/restore cycles in the smoke script
- Clear error states — token revoked mid-session (`lfq token revoke`) drives a
  visible `authRejected`/re-pair state, not a spinner

Full suite still green: `cargo test --all`, `swift test --package-path swift`,
`uv run python scripts/check_swift_multiplatform_boundaries.py`,
`uv run pytest tests/e2e/test_api_smoke.py`.

## Wave alignment

- **Vision** — serves "connect to a remote lfd, see your waves … without
  opening a laptop." This item is the gate; it stays strictly view-only.
- **Goals** — every "Done when" in `wave/Mobile/1-remote-lfd-connection.md` is
  mapped above: single-screen setup, Keychain, TLS-by-default, recover from
  drops/sleep, clear error states.
- **Risk (README: "Remote auth and host discovery need to feel simple or the
  whole surface collapses")** — the QR hero path makes first-connect a single
  scan; the pairing-token TTL fix is what keeps it *feeling* simple on day two
  and beyond. Wild-failure check: the way this gets ripped out is a token that
  silently dies overnight so the app greets you with an error every morning —
  the long sliding TTL is the direct countermeasure to that exact failure.
- **Scope exclusions** honor README "Not here": no build work, no editing, no
  native chat.

## Measure

Baseline today: time-to-first-wave-on-phone is effectively ∞ (no QR, no manual
entry — only studio OAuth). Target: a cold install reaches a connected,
streaming state in **under 30 seconds** via QR, and a warm launch on a
subsequent day reaches connected with **zero user action**. The pairing smoke
script asserts the warm-launch path (stored token still valid, `/ws` snapshot
received) — that assertion passing is the quantitative bar.
