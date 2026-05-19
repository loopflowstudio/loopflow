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
  `DiscoveryService` (studio OAuth → loopflow.studio daemon list). Pairing
  now enters through `PairingPayload` in `LoopflowCore` and
  `ConnectionStore.setPairingPayload(_:)` so token + optional pin storage stay
  centralized.
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
- **QR scanning exists only in the iOS pairing setup path**
  (`Concerto/Platform/iOS/ConnectionSetupView.swift`) using
  `AVCaptureMetadataOutput`; iOS Simulator still has no camera, so paste and
  `loopflow://pair` deep-link remain the required smoke-test paths.
  `NSCameraUsageDescription` is now present in `Concerto/Info.plist`.
- **`EventService` reconnects on path/foreground and maps WS 4401 to auth
  failure.** `MobileRootView` calls `checkConnectionHealth()` on active
  `scenePhase`, and `EventService` emits `authFailed("Session expired…")` when
  URLSession exposes close code `4401`. Full path-aware studio rediscovery on
  4401 is still a follow-up; QR/paste now shows re-pair instead of a spinner.
- Remote contract reference: `scripts/test_remote_smoke.py` (Bearer header,
  http→ws / https→wss, `connected` snapshot, `--insecure`/custom CA).
- **TokenLedger has per-row `expires_at`** (`token_ledger.rs` schema
  ~270-277): a long-lived pairing token needs **no schema change**, just a
  `mint_with_ttl(count, ttl)`. Tokens are reusable; `Claimed` is pool
  accounting, doesn't gate `validate()`. *Sliding* TTL is the trap — WS
  re-validates every 60s (`ws.rs:82-104`) and `validate()` never bumps expiry,
  so sliding = 1 DB write/phone/minute. Fixed 90-day TTL clears the
  daily-experience bar without it. `token_kind` audit column deferred.
- **iOS registers and handles `loopflow://pair`.** The scheme was already in
  `Concerto/Info.plist`; `MobileRootView.onOpenURL` now routes pair URLs into
  `RepoState.connect(pairingURL:outputBuffer:)`. Do not add `lfd://`.
- **`lf op pair` mints 90-day ledger tokens and prints QR + URL.** Host
  resolution is `--host` else `tailscale ip -4`; plaintext is refused unless
  the encoded host is in `100.64.0.0/10`. Auto Tailscale pairing emits
  plaintext because lfd has no native TLS; explicit non-Tailscale hosts default
  to TLS. Operator supplies cert pin via `--fingerprint` / `--tls-url`; as of
  2026-05-19 `--tls-url` shells out only to `openssl` and hashes DER bytes
  in-process (no `xxd`, no shell pipeline).
- **Terminal QR rendering does not need `qrcode` image features.** As of
  2026-05-19, `rust/loopflow/Cargo.toml` uses
  `qrcode = { version = "0.14", default-features = false }`; enabling defaults
  reintroduces the optional `image` dependency tree even though `lf op pair`
  only uses `render::unicode`.
- **Accountless pairing uses `auth.mode=studio` without studio registration.**
  As of 2026-05-18, missing `~/.lf/credentials.json` no longer kills lfd in
  studio mode; it logs that discovery is disabled but keeps the local
  connection-token ledger active. This preserves the two-mode auth model while
  enabling QR/paste without a loopflow.studio account.
- **Run an iOS build for mobile UI changes.** `swift test --package-path swift`
  only built macOS here and missed iOS-only errors. The useful check was:
  `cd swift && xcodegen generate && xcodebuild build -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'generic/platform=iOS Simulator' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`.
- **Unset `LF_RUN_ID` for full Rust tests inside wave sessions.** As of
  2026-05-18, `cargo test --all` inherits the active wave's `LF_RUN_ID`, which
  breaks tests that intentionally exercise run-id behavior
  (`journal::tests::terminal_run_events_clear_context_for_the_next_run`,
  `ops::ingest::tests::ingest_prefers_bucketed_items`). Use
  `env -u LF_RUN_ID cargo test --all` for local gate runs; CI is unaffected.

## Preferences

- Mobile wave is **view-only by charter** (`wave/Mobile/README.md`): no
  write/build/land/chat. Hold this line against scope creep.
- Headless runs here: make executive calls, note ambiguity in
  `scratch/questions.md`, don't stop.
