# Mobile wave memory

## Patterns

- **iOS is already a first-class Swift target.** `swift/project.yml` declares
  `supportedDestinations: [macOS, iOS]` (iOS 18.0). `Concerto/Platform/iOS/`
  has `MobileRootView`, `DiscoveryView`, `ConnectionSetupView`,
  `RepoState+iOS.swift` (iOS forces `connectionStore.setMode(.remote)`).
- **Connection infra exists — reuse, don't rebuild.** `ServerConnection`
  (host/port/useTLS/authMode/staticToken), `ConnectionStore` (persists to
  UserDefaults `concerto.connectionSettings.v2`), `ConnectionSecretStore`
  (Keychain `studio.loopflow.connection.token`, keyed by `host:port`),
  `CertificatePinningDelegate` (SHA256 pin in UserDefaults `lfd.pinned-cert.*`),
  `EventService` (WS with exponential backoff + `NWPathMonitor`),
  `DiscoveryService` (studio OAuth → loopflow.studio daemon list).
- **Multiplatform boundary is enforced** by
  `scripts/check_swift_multiplatform_boundaries.py`: `LoopflowCore` shared and
  must not import AppKit/macOS frameworks; `#if os(iOS)` only as whole-file
  gates under `Platform/iOS/`. Put pure logic (payload parsing) in
  `LoopflowCore`, camera/UI in `Platform/iOS/`.
- **lfd HTTP API**: axum, default `127.0.0.1:2486`. Auth = `Authorization:
  Bearer <token>` on all `/v0/*` + `/ws` + `/status`. `/health`, `/metrics`,
  webhooks unauthenticated. `/ws` sends `connected` snapshot, 30s ping,
  `close(4401)` on token revoke. Two auth modes: `local` (single token),
  `studio` (loopback local_token + remote token ledger).

## Learnings

- **Connection token TTL is 1 hour, hard-coded** —
  `DEFAULT_TOKEN_TTL = 60*60` in `rust/loopflow/src/lfd/token_ledger.rs:18`.
  Studio survives this via a daemon-replenished pool (size 5, threshold 2,
  `registration.rs:22-23`) re-fetched on each discovery; the phone's durable
  credential there is the studio OAuth refresh token, NOT the lfd token. Any
  accountless (QR/paste) path needs a longer-lived pairing-class token or it
  breaks "still connected tomorrow."
- **lfd has NO native TLS.** Raw `TcpListener`, no rustls
  (`bin/lfd.rs:309-317`). It refuses non-loopback studio binds unless the IP
  is Tailscale `100.64.0.0/10` or `--allow-insecure-bind`
  (`bin/lfd.rs:108-114`, `is_tailscale_ip`). "TLS by default" is a *client*
  policy: useTLS=true, plaintext only over Tailscale (WireGuard L3), real TLS
  only from a reverse proxy/relay in front. Don't promise lfd-terminated TLS.
- **No QR scanning anywhere** — only `AVCaptureDevice` for audio in
  `VoiceInputService`. iOS Simulator has no camera; the paste / deep-link
  path is the headless-testable fallback and must exist.
  `NSCameraUsageDescription` is **absent** from `Concerto/Info.plist` — adding
  it is required for the QR path.
- **`EventService` has reconnect but no `scenePhase` and no `4401` handling** —
  device-sleep recovery and token-expiry UX are genuine gaps, not bugs in
  existing code.
- Remote contract reference: `scripts/test_remote_smoke.py` (Bearer header,
  http→ws / https→wss, `connected` snapshot, `--insecure`/custom CA).
- **TokenLedger has per-row `expires_at`** (`token_ledger.rs` schema
  ~270-277): a long-lived pairing token needs **no schema change**, just a
  `mint_with_ttl(count, ttl)`. Tokens are reusable; `Claimed` is pool
  accounting, doesn't gate `validate()`. *Sliding* TTL is the trap — WS
  re-validates every 60s (`ws.rs:82-104`) and `validate()` never bumps expiry,
  so sliding = 1 DB write/phone/minute. Fixed 90-day TTL clears the
  daily-experience bar without it. `token_kind` audit column deferred.
- **iOS already registers the `loopflow` URL scheme** for studio OAuth
  (`Concerto/Info.plist:21-30`) but has **no `onOpenURL` handler anywhere** —
  deep-linking is greenfield. Reuse `loopflow://pair`; don't add `lfd://`.
- **`lf op pair` can't auto-capture a TLS fingerprint** — lfd serves no TLS;
  any cert lives in a separate proxy on another host. Operator supplies the
  pin (`--fingerprint` / `--tls-url`); absent → phone TOFU.

## Preferences

- Mobile wave is **view-only by charter** (`wave/Mobile/README.md`): no
  write/build/land/chat. Hold this line against scope creep.
- Headless runs here: make executive calls, note ambiguity in
  `scratch/questions.md`, don't stop.
