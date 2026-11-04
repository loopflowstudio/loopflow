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

1. Prove actual retained-owner attachment over WSS: native terminal rendering,
   editing an unfinished draft, Desktop handoff, background/reconnect, owner exit,
   slow output and uncertain input without replay or duplicate submission. Current
   native live acceptance reaches pairing and Task reads, not an editable owner.
2. Make setup automatic when host Tailscale is already signed in: discover the
   active installation/state, explain the exact missing step, preserve unrelated
   Serve routes, and persist remote enablement through the existing service
   installation authority. QR setup can replace manual file transfer. The foreground
   service flag and setup file are an integration path, not finished onboarding.
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
