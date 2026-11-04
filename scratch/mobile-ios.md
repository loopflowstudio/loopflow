# LOO-718 native iOS implementation

Jack Heart requested sustained improvement toward an excellent iOS app, prioritizing
polish, performance and UX over feature count. Continue in this checkout. The
initial fixture/demo boundary is not the completion boundary. No merge or Auto-Ship
authorization. Apple registration, signing for distribution and TestFlight remain
LOO-721; no Apple account work has been performed here.

## Current design

Native Swift, host Tailscale only, no embedded node, public relay or required hub.
Start with existing open Tasks and their recorded Session terminal. Preserve
Desktop typography/colors and the compact Paste / New line / keyboard controls.
New-Task flow remains Jack’s selected describe-first sheet, with a visible editable
destination and explicit Create & start, after shared recoverable admission exists.

LOO-593’s backend was merged locally at 23ed1ea7dd; neither PR was landed. The
Machine service shares Rust reads and delegates complete commands to lf. Existing
Graph and Session owners retain execution and lifetime. Desktop automation and
rendering stay local to Desktop. Do not introduce another owner, scheduler or
store. Follow-up architecture remains in mobile-task-plan.md and mobile-backend.md.

The iOS target in swift/ios.yml includes the shared project and pins SwiftTerm
1.20.0. UIKit supplies native text input, composition, selection and accessible
terminal reading. Ghostty’s Desktop artifact is Mac-only; the earlier wrapper
comparison did not establish better device performance or accessible reading.
The renderer can be revisited on evidence, without changing the Machine protocol.

## Native connection

The service’s opt-in remote listener binds loopback; host Tailscale Serve supplies
private HTTPS/WSS. Remote authentication dispatches directly to shared operations,
never through the trusted same-user Unix socket. A local command writes a private,
one-use, ten-minute setup ticket. SQLite stores ticket/device token hashes. Device
revocation denies new connections and closes active ones on the bounded check.

The app accepts an imported or pasted setup file and validates the private HTTPS
origin before connecting. It stores the issued grant immediately in Keychain with
WhenUnlockedThisDeviceOnly protection, before any fallible bootstrap. First bootstrap
pins Machine identity; later mismatches fail explicitly. Credentials never enter
URLs, logs or rendered UI. Forget Machine removes the local credential; server
revocation remains a separate Machine operation.

RemoteMobileClient reads the shared roadmap, resolves repository paths, and reads
WorkViewPlan for actual Task primary Sessions. It rejects unavailable/truncated
readings. A missing Session owner remains unavailable, with no implicit launch or
provider resume. The adapter bridges the actual owner and consumes its existing
binary protocol; no mobile CLI or second serialized domain model is introduced.

OwnerStream handles fragmented frames and nonce-qualified events, buffers snapshot
chunks until canonical geometry arrives, detects event gaps and enforces size bounds.
Input and geometry are distinct claims. Opening/reconnecting observes; Use keyboard
explicitly takes control. The request waits for its replacement screen, and repeated
taps cannot overlap takeover requests. The ordered bounded writer never retries
raw input. Backgrounding disconnects; reconnect asks the same Session for a fresh
screen and preserves provider-owned drafts. Missing/ended/mismatched Sessions have
no misleading keyboard or reconnect action.

The observer viewport preserves the owner’s grid. Geometry control fits the phone;
loss restores the owner’s geometry. Layout deduplicates resize traffic and theme
installation. Search filters the compact Task list by title/path. A fresh control
grant restores the software keyboard even if the earlier attachment hid it.

## Next work and acceptance

1. Continue live acceptance with actual Desktop and a separate physical phone,
   service/network interruptions and sustained/slow output. Native UIKit editing,
   passive reconnect, competing control/geometry and owner exit now pass over
   actual Tailscale WSS. The proof uses a real owner and a local stand-in program;
   it does not establish provider-specific IME behavior or manual Desktop UX.
2. Finish installed Desktop-to-phone onboarding acceptance. Desktop now delegates
   automatic host setup and device management to the CLI and displays the existing
   ticket as a QR code; iOS validates scans before explicit Connect. Verify actual
   camera scanning, first-certificate provisioning latency and real permission/
   sign-in failures. A running CLI alone does not establish readiness.
3. Replace per-repository sequential Task reads with a measured efficient path and
   connect shared replacement subscriptions. Keep stale/error state honest and
   cancel offscreen work. Do not create a second Work projection or event log.
4. Verify real phone ergonomics: horizontal observer panning, software keyboard
   layout, CJK conversion, dictation, physical keys, VoiceOver and sustained-output
   battery use. Programmatic scroll offsets and UIKit text calls do not prove these.
5. Complete shared recoverable create/start before enabling the selected new-Task
   sheet. A lost reply must recover the original Task or explicit uncertainty;
   preserve a created Task when launch fails. No implicit Auto-Ship or input replay.
6. Preserve useful telemetry: shared command/Process attribution, identity and
   connection diagnostics without duplicate provider usage or persisted terminal
   contents. Full Desktop Work cutover and installed update survival retain their
   separate acceptance in mobile-task-plan.md/mobile-backend.md.

## Dated evidence — November 4, 2026

All 25 signed native tests passed at 00:28, including opt-in actual Tailscale WSS
pairing against an isolated empty registry, Keychain save/reload/update/removal,
Machine identity pinning and repeated Task reads. Result:
`swift/.build/ios/Logs/Test/Test-LoopflowiOS-2026.11.04_00-28-01--0800.xcresult`.
The integration test was supplied a fresh ticket and passed; it was not skipped.
No production registry or credential was used. The temporary route was restricted
to the existing tailnet, then removed; the test grant was revoked and the isolated
service stopped. Serve configuration is back to its original empty state. The signed Simulator setup screen was inspected directly.
An unsigned build showed a Keychain error; enabling normal ad-hoc simulator signing
supplied the application identifier and resolved it. No insecure credential fallback.
Apple’s entitlement contract:
https://developer.apple.com/documentation/security/sharing-access-to-keychain-items-among-a-collection-of-apps

The Rust-generated owner fixture comparison passed; cargo fmt and all-target
Clippy with warnings denied passed. Native tests cover split snapshots, fragmented
control events, foreign nonces as output, sequence gaps, fresh grants, control loss,
request-in-progress input suppression and missing-owner states. Existing mounted
terminal tests cover IME/selection/paste, geometry and accepted-then-failed input.
These are focused proofs, not completion of the wider goal or a physical-device demo.

The earlier design and proof history remain in
`073568812fb5f3deaa5207305b6bb63aa71675b3:scratch/mobile-ios.md`.
Earlier research/prototype sources are indexed by the other scratch notes.
Preserve demo-assets and the historical HTML studies; their fixture screenshots do
not establish that the live path has been demonstrated.


### Retained-owner proof — November 4, 00:39

The opt-in `transport_tests::mobile_machine_native_acceptance` test starts the
actual owner/CLI and remote Machine service in a private registry, seeds one Task,
and invokes native XCTest through host Tailscale Serve. UIKit opens the recorded
Session, edits the existing desktop-side draft, disconnects, then reconnects after
a newer viewer has taken input and 120×40 geometry. The phone remains an observer
until explicit takeover. The existing child receives exactly
`desktop draft + phone + desktop` once and exits; its provider launch log has one
entry. The native screen reports ended rather than reconnecting a finished owner.
No real provider account, Task data or credentials were used. The temporary route
was removed; owners, service and registry are fixture-owned and cleaned up.

This found and fixed an initial-readiness race: MobileWorkspace changed to viewing
before its queued snapshot/geometry were rendered. It now remains loading until
the ordered initial control event, after the replacement screen. The same native
proof then passed with canonical geometry, without adding sleeps or tolerating
wrong dimensions. Result:
`swift/.build/ios/Logs/Test/Test-LoopflowiOS-2026.11.04_00-39-21--0800.xcresult`.
The test uses two real protocol viewers, not an operated Desktop app, and mounted
UITextInput calls, not hardware key presses. Remaining device/demo obligations
above are unchanged.

Regression check: all 24 ordinary native tests passed at 00:40 (the opt-in test
skipped without its consumed ticket; its separate live pass is above). Five
Machine-service transport tests passed; the isolated launchd test remains an
explicit opt-in. The final diff contains the readiness fix, repeatable acceptance
fixture and reconciled documentation, not a new Session execution path.

### Automatic host setup implementation

Keep one service installation authority. Add a same-user-only control operation
for enabling/disabling its loopback carrier without restarting local Desktop
connections. The remote carrier must reject this operation even for paired devices.
Save the owned loopback port and Tailscale endpoint in Machine-local configuration
for the next supervised launch. Setup detects a working signed-in host CLI (including
renamed Mac app bundles), derives its DNS name and selects an unused HTTPS port.
Reuse only a route matching Loopflow's recorded endpoint and backend; never reset
Serve or overwrite an unrelated route, and reject Funnel on the selected endpoint.
Persist only after the route is verified. Keep ticket generation in the existing
admission code; no new listener, installation owner or pairing protocol.


### Automatic setup proof — November 4, 01:00

`lf machine service pair --name ... --output ...` now detects the working host
Tailscale CLI and private DNS endpoint, obtains the selected existing service,
enables its loopback carrier through a local-only operation, selects an unused
HTTPS port and saves its configuration. The service restores enabled settings on
startup; disabled state is saved before route cleanup. The paired remote protocol
explicitly rejects configuration changes. Listener teardown closes only remote
connections; local Work watches and retained owners keep running.

The real-host proof used a private registry and a deliberate unrelated text route
on 8443. Setup found the renamed Tailscale 2.app despite the broken PATH wrapper,
selected 443 and preserved the unrelated route. Native pairing, Keychain identity
pinning and repeated Work reads passed through the automatically configured
endpoint: `swift/.build/ios/Logs/Test/Test-LoopflowiOS-2026.11.04_01-00-20--0800.xcresult`.
Repeating setup kept the same local service instance; restarting the foreground
service without a remote-port flag restored its saved listener. Disable removed
only Loopflow’s route. The test grant was revoked, service stopped and the test
route removed; Serve is back to its original empty configuration. The installed
launchd registration path itself was not changed by this work.

Focused Rust checks prove six Machine-service behaviors, route ownership including
Funnel and foreground collisions, and local subscription continuity during
listener changes. Native suite: 25 passed, one opt-in test skipped before its
separate real-host run. Protocol fixtures round-trip in Rust and Swift. The
complete native retained-owner proof passed again with the listener manager at
01:06 (`Test-LoopflowiOS-2026.11.04_01-05-59--0800.xcresult`), including draft
editing, passive reconnect, control handoff and one exact submission. Desktop's
five shared protocol tests passed at 01:08. The final route-selection test and
all-target Clippy check also passed. Temporary Serve routes were removed.

Sources used for setup semantics:
https://tailscale.com/docs/reference/tailscale-cli/serve
https://github.com/tailscale/tailscale/blob/main/ipn/serve.go
These establish persistent background routes and separate foreground configuration;
Loopflow’s ownership/persistence behavior is verified by its own tests above.

### Desktop pairing and phone scanning — November 4, 01:27

Jack Heart's automatic host-Tailscale direction now extends to Desktop Settings →
Mobile. The view delegates setup, device listing and revocation to the existing
CLI. A private temporary ticket file is read into a QR image and immediately
removed; the QR disappears on observed pairing, expiry or closing settings.
An explicit Share setup file action offers the same ticket through the system
share sheet for phones without camera access. No ticket enters a URL, diagnostic
log or automatic pasteboard write. Hiding a code does not
revoke it: the server still owns its one-use, ten-minute lifetime.

iOS scans only QR codes, validates the existing bounded setup format and shows
the Machine before explicit Connect. File import/paste remains available without
camera permission. Scanning stops when the sheet closes or the app backgrounds.
Desktop and iOS share ticket validation and endpoint construction. No second
pairing protocol or Machine installation path is introduced.

Validation: Desktop compiled and both pairing tests passed, including decoding the
actual generated QR image with Vision and decoding Rust’s configuration/device
fixtures. The signed native suite passed 25 tests with the opt-in network test
skipped (`Test-LoopflowiOS-2026.11.04_01-22-39--0800.xcresult`). Subsequent button/
fallback-copy edits built successfully. The route fixture test, all-target Clippy
and diff whitespace check passed. CLI status was exercised with unset and disabled
private configurations; neither enabled access, and conflicting flags were rejected.

Simulator screenshots in `demo-assets/mobile-qr-onboarding*.png` establish light
and dark initial layout and the corrected button contrast. Computer-use returned
`cgWindowNotFound`; no camera, share-sheet or Desktop settings interaction was
operated. Physical camera scanning, file sharing and installed app interaction
remain acceptance work. Apple’s API requirements:
https://developer.apple.com/documentation/visionkit/scanning-data-with-the-camera
