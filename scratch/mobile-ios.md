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
destination and explicit Create & start through shared recoverable admission.

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

RemoteMobileClient watches shared all-repository WorkViewPlans for actual Task
primary Sessions. It rejects unavailable/truncated
readings. A missing Session owner offers explicit Open Session; navigation and
reconnect never launch or resume a provider. The adapter bridges the actual owner and consumes its existing
binary protocol; no mobile CLI or second serialized domain model is introduced.

OwnerStream handles fragmented frames and nonce-qualified events, buffers snapshot
chunks until canonical geometry arrives, detects event gaps and enforces size bounds.
Input and geometry are distinct claims. Opening/reconnecting observes; Use keyboard
explicitly takes control. The request waits for its replacement screen, and repeated
taps cannot overlap takeover requests. The ordered bounded writer never retries
raw input. Backgrounding disconnects; returning opens one fresh observer attachment to the
same Session and preserves provider-owned drafts. A failed attempt leaves explicit
Reconnect; no background retry loop or input replay runs. Missing/mismatched Sessions have
no misleading keyboard or reconnect action. Ended Sessions retain their screen
and offer explicit Open Session, without replaying any input.

The observer viewport preserves the owner’s grid. Geometry control fits the phone;
loss restores the owner’s geometry. Layout deduplicates resize traffic and theme
installation. Search filters the compact Task list by title/path. A fresh control
grant restores the software keyboard even if the earlier attachment hid it.
System text size scales the terminal and accessory controls. Accessibility sizes
stack the Session header; observer font changes preserve even alternate-screen
cells without sending remote geometry changes.

Open Session delegates the complete `lf resume --start-only --json` operation
through the shared Machine command. It pins the recorded Session ID, reuses the
existing startup lock and returns without viewer claims. An uncertain reply
exposes passive Reconnect; no automatic command replay runs. A later explicit
open can resume a provider that has ended, so this is not a durable idempotent
receipt. Runs remain unavailable through the same shared admission policy.

## Next work and acceptance

The mounted-terminal sustained-output proof now passes; retain its failure-stage
diagnostics. Two earlier batching experiments failed at output/recovery deadlines
with misleading socket-error attribution. Later batched and original-decoder runs
passed. Their cause remains unresolved; do not infer that timeout-message cleanup
fixed those failures. No batching change remains: it reduced feed calls without
materially reducing feed time. Physical-phone rendering/energy and typing under
load still need proof. Detailed results and failure logs are recorded below.

1. Continue live acceptance with actual Desktop and a separate physical phone,
   service/network interruptions and sustained rendered workloads. Native UIKit editing,
   passive reconnect, competing control/geometry and owner exit now pass over
   actual Tailscale WSS. The proof uses a real owner and a local stand-in program;
   it does not establish provider-specific IME behavior or manual Desktop UX.
   Raw sustained-output acceptance now delivers 32 MiB to an active phone stream,
   disconnects a stalled viewer and restores it passively; see the evidence below.
2. Finish installed Desktop-to-phone onboarding acceptance. Desktop now delegates
   automatic host setup and device management to the CLI and displays the existing
   ticket as a QR code; iOS validates scans before explicit Connect. Verify actual
   camera scanning, first-certificate provisioning latency and real permission/
   sign-in failures. A running CLI alone does not establish readiness.
3. Continue Task-list performance and recovery acceptance under sustained Work
   changes and real network loss. The phone now uses one all-repository selection
   watch, closes it offscreen/in background and preserves stale readings on failure.
   Local query cost is measured below; physical-phone latency/energy remains open.
4. Verify real phone ergonomics: horizontal observer panning, software keyboard
   layout, CJK conversion, dictation, physical keys, VoiceOver and sustained-output
   battery use. Programmatic scroll offsets and UIKit text calls do not prove these.
5. Continue new-Task ergonomics and failure acceptance. The describe-first sheet,
   saved draft/destination, shared recoverable admission and explicit initial
   terminal opening are implemented. Creation uses one request UUID/payload saved
   before sending. App restart and lost replies recover it; changing paired Machines
   cannot retarget it. Started opens the recorded Task/Session; failed/unknown keeps
   Open Task without rerunning the Graph. Existing-Task navigation stays passive.
   Desktop delegation and explicit create/start share Session owner startup exclusion.
   Verify actual dictation and software-keyboard layout on a physical phone. The
   phone still pairs one Machine. Save for later now moves a request into a
   protected recovery list and frees the composer; inspecting/recovering it keeps
   a newer draft intact. Broader multi-Machine selection remains UX work.
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

### One Task reading and live replacement — November 4, 01:49

The phone previously performed bootstrap + roadmap + two requests per repository.
The roadmap could also observe the external tracker before returning. It now uses
a shared all-repository selection read (`desktop plans --all`) with the existing
ViewPlan projection and one pinned store snapshot. Its watch uses
the existing Subscription owner and Session observation probe; no mobile store,
Task resolver, polling scheduler or delta log.

While the Task list is visible and the app is active, one bounded replacement
stream runs. Leaving/backgrounding cancels it; return starts fresh. Pull-to-refresh
confirms a new reading on the same connection. Failure preserves the last list
with an explicit stale/error explanation. Incomplete projection bodies fail, and
an older canceled opening cannot replace a newer watch. Whole readings coalesce
to one queued replacement; checkpoint acknowledgements settle before coalescing.

The new read exposed a shared projection cost: every repository loaded all Tasks
before filtering them. The existing task reader now applies repository scope in
SQL. On the current Machine, 46 repositories/36 visible Tasks/124,608 JSON bytes
took 4.653 seconds before that fix, then 0.876/0.707/1.013 seconds (median 0.876).
These are local debug-CLI wall times, not phone/network latency or a controlled
benchmark suite. The cold original measurement and warm repeats are not a claim
of a portable speedup. Network requests now stay at bootstrap + one subscription
instead of growing with repository count; refresh uses its existing connection.

Native suite: 29 passed, one opt-in skipped, including four Task-update tests.
CLI/read/watch parity and cross-repository service replacement tests passed;
the latter observes a rename and a new repository without client polling.
All-target Clippy passed. The separate native host-Tailscale proof also passed
initial/confirmed readings and retained-owner editing/reconnect at
`Test-LoopflowiOS-2026.11.04_01-48-15--0800.xcresult`; temporary Serve configuration
was restored. All 21 selection-engine tests and six Desktop protocol tests passed.

### Recoverable creation admission — November 4

`task create --run --json --request-id UUID` now reserves one Task identity in the
shared store, validates the complete request on reuse, serializes competing callers
and records whether Graph launch was attempted. Existing planning creation and
ordinary `task run`/primary-conversation admission still own their effects. A saved
result survives the CLI and Machine service exiting. Unknown launch outcomes never
automatically advance the Graph. Failed starts retain their filed Task. This is
backend support for Jack Heart's selected flow; the iOS creation sheet and explicit
initial terminal launch remain unfinished.

The real service test exposed macOS ENOTCONN during peer-credential lookup when a
caller immediately disconnected. That now closes only its connection instead of
terminating the listener. The same test admits creation, drops the reply, waits for
the command, restarts the service and recovers through both socket and CLI with one
Task, Graph process and primary Session. It uses an isolated local socket and
fixture repository, not a phone or external planning provider.

Checks: six creation/DTO integration tests, shared Rust wire fixtures, app/CLI write parity, seven Swift MachineProtocolTests, all-target Clippy and signed iOS Simulator build passed; logs `/tmp/mobile-create-tests-final.log`, `/tmp/mobile-create-validation.log`, `/tmp/mobile-create-native-build.log`.

### Native Create & start — November 4, 02:46

Jack Heart's describe-first flow now creates through the shared Machine command,
opens the admitted primary Session through the retained owner, and navigates to
that recorded terminal. The phone saves its exact request before sending, keeps
uncertain payloads immutable, preserves failed starts, and restores drafts across
app restarts. Original Machine identity and destination remain attached to pending
requests. Closing a sheet cancels observation without replaying the command or
letting a late response overwrite a newer recovered draft. Native keyboard
Dictation supplies editable text; there is no separate speech service.

Actual host-Tailscale acceptance passed in Simulator at
`Test-LoopflowiOS-2026.11.04_02-46-23--0800.xcresult`: existing Session handoff and
passive reconnect, then native creation, same-result recovery, recorded Session
opening and native text submission. The isolated fixture recorded exactly two
provider launches, one per intended Session, and both exact submissions. It uses
stand-in providers and direct native model/input calls, not manual button presses
or real provider accounts. The temporary private Serve route was removed; the
original empty Serve configuration was verified afterward.

Six native creation tests cover saved drafts, lost replies/app restart, Machine
changes, failed/unknown starts, local read/write failures and late canceled replies.
Six shared admission/DTO tests and focused Desktop terminal-opening regressions
pass. UI captures in both themes and accessibility text size were inspected after
fixing the test's unmounted-window capture. These do not establish physical-phone
keyboard or VoiceOver quality. Final six creation tests passed at
`Test-LoopflowiOS-2026.11.04_02-48-59--0800.xcresult`; the ordinary native suite
passed before the last cancellation/rendering refinements (34 passed, one opt-in
skipped). Final all-target Clippy and seven shared Swift protocol tests passed.
Logs: `/tmp/mobile-creation-native-final.log`, `/tmp/mobile-creation-validation-final.log`,
`/tmp/mobile-terminal-regressions.log`, `/tmp/mobile-creation-tailscale.log`.
Rendered captures are `demo-assets/mobile-create-ready-{light,dark,large-text}.png`.

### Foreground restoration and operated keyboard — November 4, 03:00

Returning from the background now automatically restores the selected Session as
an observer. It never claims input/geometry or resends keys. A failed foreground
attempt leaves the existing Reconnect action; ended/unavailable Sessions retain
their explanation. Leaving the Session screen clears the pending restoration.
Generation fencing closes an older attachment that completes after backgrounding.

Twenty-one native terminal tests pass, including fresh-screen/draft restoration,
no control/geometry takeover, one attempt per return, explicit retry after failure,
leaving before restoration, and an attachment completing late. Actual Tailscale
owner acceptance also passes through this restoration entry point at
`Test-LoopflowiOS-2026.11.04_02-54-45--0800.xcresult`, preserving newer Desktop-sized
control and exact input counts. Its temporary route was removed.

The new LoopflowiOSUITests target operates the actual app in explicit fixture mode:
Task tap, Use keyboard, system typing, Home, app activation, observer restoration,
explicit takeover, continued typing. Captures show the unfinished draft, hidden
keyboard on return and one 31-character submission after continuation. The peer
requests a steady cursor using the terminal protocol; this avoids XCTest waiting
60 seconds for repeating cursor animations. Live terminal styling is unchanged.
This is operated Simulator/software-keyboard evidence, not a physical-phone or
VoiceOver demonstration. The separate Tailscale proof uses the actual carrier.

Checks: native terminal suite `/tmp/mobile-foreground-final-tests.log` (21 pass); operated app suite `Test-LoopflowiOS-2026.11.04_03-00-21--0800.xcresult` (1 pass); real carrier `/tmp/mobile-foreground-tailscale.log` (pass). Captures: `demo-assets/mobile-foreground-{keyboard-before,observer-return,keyboard-after}.png`.

### Enlarged text and touch targets — November 4, 03:22

The Session header now stacks at accessibility sizes instead of squeezing its
status into broken words. Keyboard/reconnect labels contain the full 44-point
touch target. Terminal text and the accessory row scale with the system setting;
the row grows for wrapped titles. Changing an observer font preserves its exact
120×40 alternate screen, including far-edge cells, and sends no geometry change.
Explicit takeover still fits the terminal to the phone.

The Task/Session touch-target and accessible-description audits pass at standard
and accessibility-extra-extra-large sizes. Keyboard → Home → return → continued
typing also passes at both sizes. Captures were inspected; enlarged text and
controls remain visible above the keyboard. Native tests cover font-change cell
preservation and 4.5:1 minimum text contrast across both default theme surfaces.
The iOS contrast/clipping audit also flagged fully visible custom-font labels;
one flagged status measured 14.14:1 from its captured foreground/background.
Those audit categories are not asserted as passing; palette checks and screenshot
review supplement the selected audits. This is not VoiceOver navigation evidence.

Checks: `/tmp/mobile-readable-final.log` (23 native terminal tests, 2 operated UI
tests); enlarged UI `/tmp/mobile-readable-accessory.log` (2 UI tests pass). That
earlier run exposed two fixture assumptions about the simulator's default font;
the line-selection fixture now pins its starting size and the final suite passes.
Simulator text size was restored. Captures:
`demo-assets/mobile-large-{tasks,session,keyboard}.png`. Physical-phone gestures,
dictation, VoiceOver and sustained-output energy remain acceptance work above.

### Saved requests without blocking new work — November 4, 03:32

Save for later atomically moves an active request and its optional result into
the same protected local file, clearing only the composer. It does not cancel
remote execution. Saved requests show original instructions/Machine/destination,
check the same admission identity, and open the recorded Task. A newer draft is
preserved throughout recovery. Failed local saves leave the request in place;
canceled late replies cannot restore a record another sheet already opened.
Existing on-phone drafts retain their data when the recovery-list field is absent.
No credentials or second server admission path were added.

Twelve creation tests pass, including lost replies/restart, re-pairing, local
write failures, cancellation and the Save for later → Check status button path.
Mounted UIKit/SwiftUI captures were inspected in both themes and accessibility
text size (`demo-assets/mobile-saved-request*.png`). These are native view/model
tests, not physical-phone gestures. `/tmp/mobile-saved-final.log` records the pass.

Actual host-Tailscale acceptance also passes at
`Test-LoopflowiOS-2026.11.04_03-31-33--0800.xcresult`: create a Task, save its
confirmed request, reload it, recover the identical receipt and open its Session
without submitting the newer draft. The real owner receives native input once;
the isolated fixture records two provider starts for the two intended Sessions.
This proves saved confirmed-result opening over the carrier; lost-result recovery
is exercised by the separate native admission tests. The temporary Serve route
was removed and the original empty configuration verified. Broader multi-Machine
selection and the physical-phone acceptance above remain.

### Consumer-driven terminal output — November 4, 03:44

RemoteAttachment now reads as its renderer consumes events. Its eager reader
Task and 16-event queue are removed: a legal coalesced packet containing many small
frames cannot overflow that artificial event-count limit. Only the current decoded
batch is retained, then released; framing/snapshot limits remain in OwnerStream.
Cancellation closes the connection, writer failures retain their uncertainty
message, and input is still bounded and never replayed. No server queue or owner
policy changed.

The actual host-Tailscale fixture now includes an independent retained terminal
which sends 512 × 64 KiB, paced by the active phone's acknowledgements. The active
native adapter received all 33,554,432 bytes while a second connection deliberately
stopped reading. That stalled connection disconnected after 1,061,888 output bytes;
its replacement received the current screen as an observer. The original controller
could still finish the terminal. The preceding draft/handoff/create tests also
pass, with exactly the two intended provider launches.

Final carrier result: `Test-LoopflowiOS-2026.11.04_03-44-31--0800.xcresult`;
outer log `/tmp/mobile-output-final.log`. The temporary Serve route was removed
and its original empty configuration verified. Twenty-six native terminal/decoder
tests passed in `/tmp/mobile-pull-tests.log`; cargo fmt and all-target Clippy passed.
This sustained load exercises the real adapter and carrier, not SwiftTerm drawing
32 MiB or physical-phone energy/latency. Those acceptance items remain open.

### Mounted rendering and connection errors — November 4, 04:11

The sustained-output acceptance now mounts SessionTerminalController in a real
Simulator window, applies actual geometry/snapshot/output callbacks and verifies
the final rendered text. The current original decoder delivered all 33,554,432
bytes through 32,769 feeds, taking 6.313 seconds inside feed calls; the longest
feed was 0.491 ms. A stalled viewer disconnected after 1,060,864 bytes and its
replacement received the current screen without control. Existing draft handoff,
saved creation recovery and exact submission/provider counts also passed.
This measures synchronous feeding, not GPU frame cadence, physical-phone energy
or keyboard responsiveness under load.

An experiment combining adjacent output up to 16 KiB reduced calls to 17,826 but
took 6.245 seconds in feeds (1.904 ms longest). The original path's neighboring
run took 6.314 seconds. The additional buffering code was removed: this evidence
does not establish a useful improvement. Two earlier experimental runs failed
after rendered completion or during output, respectively. XCTest attributed raw
socket errors to nonthrowing reader loops; their actual cause remains unresolved.
Preserved native logs: `/tmp/mobile-render-batched-native.log` and
`/tmp/mobile-render-diagnostic-native.log`. The test now records counts and the
actual error at its output/drain deadlines. Subsequent passing runs do not erase
these failures or establish that batching caused them.

Separately, MachineConnection now translates a deadline-triggered socket failure
into “The Machine did not respond in time,” including when the deadline fires
during an awaited read/write. Explicit task cancellation stays CancellationError;
other network failures retain their original error. No timeout, retry or input
delivery policy changed. Live acceptance opens an idle authenticated socket and
asserts its timeout, rejects a later write, and cancels another pending read.

Checks: host-Tailscale native acceptance passed at
`Test-LoopflowiOS-2026.11.04_04-10-58--0800.xcresult` (22.400 seconds), log
`/tmp/mobile-connection-lifecycle-native.log`; diff whitespace passed. Temporary
Serve configuration was removed and verified empty. The preceding unchanged-
decoder rendered proof also passed at 04:08:57. No Apple distribution work or
physical-device acceptance is claimed.

### Searchable creation destinations — November 4, 04:21

The describe-first sheet now opens a native searchable repository list instead of
a menu. Names and full paths distinguish equal basenames. Unavailable repositories
show their shared error and cannot be selected. Pull-to-refresh and a toolbar action
reload destinations, including after the list was empty. Refresh preserves the
instructions and selected path; a missing selected repository requires an explicit
new selection. Selection persists before returning to the composer. An unavailable
Machine no longer remains labeled “Connecting…” after its request has failed.
The same creation model/admission owns all effects; no new backend or Machine
picker was introduced.

Checks: 14 native creation tests passed at 04:17:36, including exact-path selection,
empty-to-populated refresh, unavailable entries, missing saved destinations and
offline refresh without losing the draft. The final native search capture passed
at `Test-LoopflowiOS-2026.11.04_04-20-44--0800.xcresult` after correcting the test's
UIKit assumption (this iOS build exposes UITextField, not UISearchBar). Native
text insertion filtered a path fragment to the intended row; its capture was
inspected, alongside light/dark, accessibility text and empty-state layouts.
Logs: `/tmp/mobile-repository-picker-tests.log`, `/tmp/mobile-repository-search-field.log`.
Captures: `demo-assets/mobile-repository-picker-{light,dark,large-text,empty,search}.png`.
These are mounted Simulator views; physical-phone keyboard and VoiceOver review
remain outstanding. Backend behavior is unchanged, so no carrier rerun was needed.

### Explicitly continue a stopped Session — November 4, 04:47

A saved conversation with no running owner now offers Open Session. An ended
terminal retains its screen and offers the same action. The Machine delegates
`resume --start-only --json` to the CLI, pins the resolved Session and uses the
same startup exclusion as Desktop and Task creation. It reuses a live owner or
resumes that conversation, then the phone attaches as an observer. Input and
geometry still require Use keyboard. No CLI is installed on iOS, no provider
runtime moved into the service, and no second lifecycle owner was introduced.
The reply decodes the existing shared SessionRecord. CLI explanation now describes
owner reuse without claiming a takeover; existing owners retain their harness.

Lost replies leave passive Reconnect, never automatic command replay. Background,
cancellation and leaving the screen fence late opening replies. A canceled opening
cannot leave the screen stuck loading. This operation is intentionally not a
creation-style durable receipt: another explicit open after the provider ended
can resume it again. Missing identities, Runs and incompatible flags fail before
launching. Ordinary `resume --json` retains its existing relay lifetime.

Native terminal suite: 28 tests passed at 04:41, including explicit button action,
no implicit opening/control, lost replies, duplicate taps and lifecycle cancellation.
The final prominent-button captures and action test passed at 04:45; light/dark
and accessibility3 screenshots were inspected and saved as
`demo-assets/mobile-open-session-{light,dark,large-text}.png`. Shared Swift DTO and
protocol suites passed 59 tests. CLI proofs cover concurrent starts sharing one
provider, preserved viewer control/geometry, response loss followed by passive
inspection, service exit without owner loss, Run refusal and normal relay behavior.
Read-only explanation, Rust wire/DTO fixtures and app/CLI write parity pass.

Actual host-Tailscale acceptance passed twice, including the full SessionRecord
decoder at `Test-LoopflowiOS-2026.11.04_04-46-31--0800.xcresult`. The same recorded
Session continued after its first provider ended and received native text once.
Opening a live owner retained the newer viewer's 120×40 screen and input claim.
The fixture counted exactly three intended provider starts: the initial Session,
its explicit continuation, and a new Task. Each submitted line arrived once.
The 32 MiB mounted rendering/slow-viewer proof also passed. The temporary Serve
route was removed and the original empty configuration verified afterward.
These use stand-in providers and Simulator UIKit input, not actual account history,
physical keyboard/dictation or a manual Desktop interaction. Those acceptance
items above remain open; no distribution, push or merge occurred.

Logs: `/tmp/mobile-session-open-rust-final.log`, `/tmp/mobile-session-open-layouts.log`,
`/tmp/mobile-session-open-polish.log`, `/tmp/mobile-session-open-shared-swift.log`,
`/tmp/mobile-session-open-validation.log`, `/tmp/mobile-session-open-validation-rest.log`,
`/tmp/mobile-session-open-tailscale-final.log`. Final CLI argument/unknown-identity
checks passed in `/tmp/mobile-session-open-last.log`; eight shared Swift protocol
tests passed in `/tmp/mobile-session-open-protocol-final.log`. Cargo fmt, all-target
Clippy with warnings denied and diff whitespace checks pass.
