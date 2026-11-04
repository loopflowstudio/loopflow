# Loopflow for iOS

```sh
xcodegen generate --spec swift/ios.yml
open swift/LoopflowMobile.xcodeproj
```

Run **LoopflowiOS** on an iPhone simulator. The development scheme explicitly
passes `--fixture`: open a Task, choose **Use keyboard**, then type in its existing
Session. Paste and **New line** preserve multiline input; Return submits. Try
`permission`, then `y` or `n` and Return. Nothing executes outside the app.
The keyboard button hides the software keyboard while retaining hardware input.
Text selection and VoiceOver reading belong to the native terminal.
Search the compact Task list by title or checkout path. A Task without a recorded
Session opens an explanation instead of an empty terminal.

Observing preserves the owner's grid in a scrollable viewport. Taking geometry
control fits the terminal to the phone and software keyboard; losing it restores
the owner's grid. Layout changes send a resize only when the granted size changes.
A fresh keyboard grant restores the software keyboard even if it was hidden in
the previous attachment. Theme colors are installed only when the theme changes.

Disable `--fixture` to connect to a Machine. Sign into host Tailscale on both
devices. In Desktop, open Settings → Mobile → **Pair a phone**. On iPhone, choose
**Scan pairing code**, scan the Desktop code, then confirm **Connect Machine**.
Desktop detects and configures signed-in host Tailscale automatically. Camera
access is optional: **Other ways to connect** offers setup-file import/paste for
the same [Machine setup commands](../../rust/loopflow/src/machine_service/README.md#pair-a-phone-through-host-tailscale).
The app saves
the device grant in Keychain, pins the Machine identity
on first bootstrap, reads actual open Tasks and attaches their recorded Session
owners. It installs no CLI on the phone and never starts a provider.

The Task list uses one all-repository view subscription after checking Machine
identity. Whole readings replace the list; pull-to-refresh confirms the current
reading on that same connection. Leaving the list or backgrounding closes it.
Returning opens a fresh subscription. A connection or reading failure keeps the
last Tasks visible with an explanation; **Try again** reconnects. This path reads
the Machine’s local Work and does not synchronously refresh an external tracker.

The connected Machine’s hostname replaces the fixture label in both screens.
**Forget Machine** removes the phone’s saved credential; use Desktop’s **Remove
access** or the Machine’s device revocation command to withdraw its server grant. Backgrounding closes the
attachment; **Reconnect** restores the same Session as an observer.
Input and geometry require a fresh grant. Failed input is never queued or retried.
If opening or reconnecting fails, **Reconnect** stays on the Session screen.
Inspect the restored prompt before resubmitting input whose delivery was uncertain.
Missing, ended or mismatched Sessions retain their explanation without a retry.

The app uses the shared Loopflow fonts, Light/Dark themes and WorkViewPlan types.
`MobileClient` is an injectable UI boundary, not a serialized protocol.
`MachineProtocol.swift` and `tests/fixtures/dto/machine/` come from LOO-593 at their
canonical paths. Client-authored Task data passes through that codec; it is not
a recorded Machine reading. The live adapter uses authenticated WSS to the
Machine service’s separate remote admission boundary. Its private Unix socket
is never forwarded to remote callers. OwnerStream consumes Rust’s binary frames
and nonce-qualified events; snapshots are assembled before rendering at the
owner’s geometry. Every reconnect requests a fresh screen. Output/input queues,
message sizes and connection deadlines are bounded.
The deterministic peer owns its draft and consumes actual terminal bytes.
Its small line editor is test/demo code, not another server or Session runtime.

## Renderer

SwiftTerm 1.20.0 is pinned by commit in `swift/ios.yml`, isolated from Desktop's
SwiftPM/Xcode build. Its UIKit view supplies UITextInput, hardware key encoding,
selection and UIAccessibilityReadingContent. The app forwards committed input
immediately and preserves bracketed paste. Multiline paste is unavailable when
the peer has not enabled bracketed paste, so it cannot accidentally submit lines.
Only text is pasted; phone file paths are not remote attachments.

Ghostty was evaluated through the existing Desktop artifact and
[Lakr's wrapper](https://github.com/Lakr233/libghostty-spm/tree/db22505bd74d968c7fc12393219420ecbcead393).
Desktop's artifact is macOS-only. Lakr's UIKit wrapper has host I/O, selection
and marked text, but needs Swift 6.2 and exports the same GhosttyKit module name
as Desktop. Its viewport-reading API is not a VoiceOver reading implementation.
SwiftTerm's native accessibility/input path and Swift 6.0 package make it the
smaller first client integration. This is a source/packaging comparison followed
by a SwiftTerm build and input proof, not a Ghostty device-performance trial.
The bundled notices include SwiftTerm and the shared fonts; theme licenses stay
with the shared framework resources.

## Testing

```sh
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') xcodebuild test \
  -project swift/LoopflowMobile.xcodeproj -scheme LoopflowiOS \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro' \
  -derivedDataPath swift/.build/ios -skipPackagePluginValidation \
  -parallel-testing-enabled NO \
  CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- < /dev/null
```

Xcode 26.2/iOS 26.2 simulator is the tested toolchain; deployment target is iOS 18.
The pinned package's build plugin only generates source-control version metadata;
`-skipPackagePluginValidation` permits that reviewed plugin in headless builds.
Keep ad-hoc simulator signing enabled: an unsigned host cannot access Keychain.
Xcode supplies the simulator’s application identifier; device signing and
TestFlight belong to LOO-721. The tests mount the actual UIKit terminal and assert editing, marked-text commit, combining characters,
multiline paste, native replacement, selection, rendered accessibility text,
viewport resizing, preserved observer geometry, control loss, repeated in-screen retry, identity rejection and
accepted-then-failed input without replay or duplicate submission. They also consume the
backend's shared Machine fixtures, without inventing a second codec.

Device demo still owns actual software-keyboard layout, CJK keyboard/dictation
quality, physical key combinations, VoiceOver navigation and sustained-output
energy use. The tests exercise UIKit input methods; they do not manufacture real
hardware presses or prove every IME. The owner decoder consumes a Rust-generated binary fixture across fragmented
reads. Actual Tailscale pairing, Keychain persistence, Machine identity and empty
Task reads have passed in Simulator against an isolated service. Native editing
and reconnect have also passed through a real retained owner, including a newer
viewer holding input and 120×40 geometry. The child receives the combined draft
exactly once and no reconnect starts a provider. These peers use the actual
protocol; this is not yet a manual Desktop-to-physical-phone demonstration.
Sustained output, real-network outages and physical-phone use remain acceptance work. Contextual approvals/Stop stay absent until the
backend supplies trustworthy state and action semantics; terminal prompts remain
answerable through ordinary typing.


### Opt-in host Tailscale acceptance

```sh
# Inspect existing routes and choose unused ports before running this proof.
tailscale serve status --json
tailscale serve --bg --https=8443 http://127.0.0.1:49250
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  LOOPFLOW_MOBILE_SIMULATOR='<booted simulator UUID>' \
  LOOPFLOW_MOBILE_ENDPOINT=https://machine.tailnet.ts.net:8443 \
  LOOPFLOW_MOBILE_PORT=49250 \
  cargo test -p loopflow --test transport_tests mobile_machine_native_acceptance \
    -- --ignored --exact --nocapture < /dev/null
tailscale serve --https=8443 off
```

Generate the project and install the signed app in that simulator first. Substitute
the host’s real Tailscale DNS name. This opt-in Rust test creates a private registry,
Task and retained provider fixture, starts the real remote carrier, transfers a
one-use setup ticket directly into the app container, then runs the native XCTest.
It stops its owner/service and deletes its registry afterward. The native test
opens the Task’s Session in UIKit, edits the existing draft, disconnects, lets a
second viewer take control, reconnects passively at the owner’s geometry, then
explicitly takes control and submits. Rust asserts one provider launch and one
exact submission. The native log is `swift/.build/mobile-acceptance.log`. Remove
only the temporary Serve route afterward; never reset unrelated host routes.

`MachineConnectionIntegrationTests` skips unless the installed simulator app’s
Documents directory contains `LoopflowIntegration/setup.json`. Supply a fresh
setup file generated by an **isolated, empty** development Machine registry and
its temporary host Tailscale Serve route. Transfer the file directly; never print
its credential. Then run the signed test command above. The test removes the
setup file, redeems it, saves/reloads the grant, checks bootstrap identity and
reads the empty Task list twice. When the Rust fixture also supplies session.json,
it runs the retained-owner scenario instead. It restores any prior simulator credential.
Afterward revoke the test device, stop that development service and remove only
its temporary Serve route. This test deliberately does not assert that an empty
Task list proves Session attachment.
