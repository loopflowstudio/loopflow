# Loopflow for iOS

```sh
xcodegen generate --spec swift/ios.yml
open swift/LoopflowMobile.xcodeproj
```

Run **LoopflowiOS** on your iPhone. The app opens the Machine connection screen;
connect over Tailscale, open a Task, then choose **Use keyboard** to type into
its running Session. Paste and **New line** preserve multiline input; Return submits.

The app icon uses Desktop's artwork. After changing `LoopflowMac/AppIcon.icns`,
regenerate the opaque iOS asset from the repository root:
`xcrun swift scripts/ios-icon.swift`.

For offline UI testing only, add `--fixture` to the launch arguments. This uses
the native terminal renderer with a simulated editor: no shell, provider or
Loopflow Session runs, and no commands execute. UI tests opt into it individually.
Submit `history` for numbered scrollback or `permission` for a simulated question.
This mode cannot demonstrate working with a real Session.
**Keys** shows arrows, Tab and Escape for provider menus and cursor movement.
The keyboard icon returns to the typing controls. Switching keys preserves the
keyboard, terminal size and draft. Keys require input control and send nothing
while Find is open or after control moves to another viewer.
The keyboard button hides the software keyboard while retaining hardware input.
Typing controls disappear while observing, leaving more room to read.
Text selection and VoiceOver reading belong to the native terminal.
Reading and selecting text do not require keyboard control. Tap an underlined
program-supplied web link to open the native browser, then Close to return to the
Session. Ordinary text URLs are not automatically made into touch links.
Choose **Find in Session** (magnifying glass) to search text loaded on this phone,
including local scrollback. Type a phrase, then use the arrows or Search key to
move between matches. **Done** returns to reading. Find works while observing;
its keyboard never sends the query to the program or takes control. Reconnect
keeps the query but clears its old highlight; search again against the new screen.
Search the compact Task list by title or checkout path. A Task without a recorded
Session opens an explanation instead of an empty terminal.

The **Sessions** button lists this Task's open conversations with their names,
providers, availability and primary/current markers. Select one to view its terminal;
**Use keyboard** takes control separately. Each keeps its own unfinished draft.
Reconnect, returning to the app and **Open Session** stay on the conversation you
selected. Returning to Tasks and reopening the same Task also keeps that choice
for this connected workspace, as an observer. Each Task remembers its own choice;
restarting the app starts at the Task list. Choosing one never changes the Task's
primary Session. Pull to refresh;
**Load more Sessions** reads the next page. A failed read preserves the list, and
closing the sheet cancels its requests. The shared CLI applies Task membership
before the page limit; the phone has no separate Session inventory or watcher.

Observing preserves the owner's grid in a scrollable viewport. Taking geometry
control fits the terminal to the phone and software keyboard; losing it restores
the owner's grid. Layout changes send a resize only when the granted size changes.
A fresh keyboard grant restores the software keyboard even if it was hidden in
the previous attachment. Theme colors are installed only when the theme changes.
System text size also enlarges the terminal and its accessory controls. At
accessibility sizes the Session header stacks vertically. Observing still keeps
every remote cell; pan to read the wider screen. Only explicit geometry control
fits the larger text to the phone. A newly opened Session starts at the bottom of
its remote screen, including when that screen is taller than the phone.
When reading earlier output or the top of a taller remote screen, **Latest**
returns to the bottom locally. New output leaves the reading position alone;
Latest preserves selection and never takes keyboard or geometry control.
Find hides Latest so it cannot cover a highlighted match.

In landscape, Session state and actions move into the navigation bar to keep the
terminal readable above the keyboard. Tap the status for the Task, Session,
Machine and current program report. Rotating preserves the draft; observers keep
the owner's grid. Long Find queries scroll inside their field and keep **Done**
visible, including at accessibility text sizes.

To connect to a Machine, sign into the same Tailscale account
on both devices. In Desktop, open Settings → Mobile → **Enable mobile access**.
On iPhone, choose **Scan QR code**, scan the Desktop code, then **Connect Machine**.
Desktop detects and configures signed-in host Tailscale automatically. Camera
access is optional: **Enter address** accepts the same HTTPS address, typed or
pasted, including a hostname without the `https://` prefix.

Connection errors distinguish an offline phone, an unknown address, an unreachable
Machine and a certificate problem. Server errors retain their recovery instructions,
including when the installed server needs updating. Use a separate physical device
for a live connection: a Simulator on the server's Mac shares its Tailscale identity
and is refused.

The QR contains only the address. Tailscale supplies caller identity; the Machine
admits its owner. The phone saves the verified endpoint and Machine identity in
Keychain, with no device token. Future launches reuse that connection. Opening
an existing Task observes its running Session; it never starts or resumes a
provider implicitly. No CLI is installed on the phone.

If a Task has no Session here and its checkout is on another Machine, the phone
explains where to look. It does not infer that the remote provider stopped or
start a replacement locally. A recorded conversation remains accessible even
when its Task's checkout is elsewhere; the Sessions sheet can also select a
conversation on the connected Machine. Automatic routing between Machines is
not yet available.

If its terminal has stopped, choose **Open Session** to continue the recorded
conversation on the Machine. It reuses a running owner or resumes saved history,
then opens as an observer. **Use keyboard** remains a separate action. If opening
is not confirmed, **Reconnect** inspects that Session; it never repeats the start.
An ended screen retains its output and offers the same explicit opening action.

The Session header shows the program's reported activity separately from keyboard
and connection state. Tap the activity to read its message. Working, blocked and
done are program reports; they never complete the Task or approve an action.
Disconnecting or switching Sessions clears the old report; reconnect restores
the owner’s current report along with the screen. A program that reports nothing
gets no inferred status. The retained owner validates the reports once and sends
the shared ProgramStatusRecords in its attachment stream; the phone adds no parser,
subscription or status writer. Only the controlling renderer answers feature queries,
and snapshot replay never answers them again.

Choose **New Task** (+), describe the work, and select a repository on the connected
Machine. The sheet saves your draft and remembers that destination. Use the
keyboard’s microphone to dictate, then edit before choosing **Create & start**.
Tap the destination to search repositories by name or path. The list shows full
paths so repositories with the same name stay distinct. Refresh there after
adding a repository on the Machine; unavailable entries explain why they cannot
be selected. Refresh preserves your instructions and chosen destination. If that
destination disappears, choose another explicitly before starting.
The Machine runs the shared creation command and opens its primary Session through
the existing retained owner. A confirmed start opens that Session as an observer;
**Use keyboard** explicitly takes input control.

If the reply is lost, **Check status** recovers the same saved request, including
after restarting the app. It never files another Task or starts a provider again.
A failed or uncertain start keeps the created Task available through **Open Task**.
The saved request remains recoverable until its Task screen appears. Closing the
sheet during a read keeps it saved, and a reply from an older sheet cannot dismiss
a newer one or change its destination.
The request retains its original Machine and repository even after changing connections;
reconnect that Machine to recover it. A failed local draft save prevents sending.
Drafts are protected local files excluded from backup; the saved Machine identity stays in Keychain.
Swiping New Task closed waits until its current draft is saved. If a local save
fails, the sheet keeps your text visible; Close retries saving before dismissing.

Choose **Save for later** to keep an unresolved request and start a different
draft. This does not cancel work on the Machine. **Saved requests** retains the
original instructions, destination and any confirmed result across app restarts.
Check status or open its Task there; your newer draft remains untouched. Recovery
uses the original request identity and Machine. A failed save keeps the active
request in place, and opening a saved Task removes only its recovery entry.

Reopening the app shows the last received Task list while connecting. When the
Machine is unavailable, the list retains its **Last updated** time and offers
**Try again**. These are saved Task rows, not live status; opening a Session still
requires the Machine. A fresh reading replaces the entire list, including an empty
one. The phone keeps one protected, backup-excluded cache up to 2 MiB, scoped to
the verified Machine. It stores no terminal output or input. Cache loss or failure
never blocks a live connection.

The Task list uses one all-repository view subscription after checking Machine
identity. Whole readings replace the list; pull-to-refresh confirms the current
reading on that same connection. Opening a Session keeps that subscription alive,
so returning to Tasks uses the current reading. Leaving the connected workspace
or the foreground closes it; returning opens a fresh subscription.
A connection or reading failure keeps the
last Tasks visible with an explanation; **Try again** reconnects. This path reads
the Machine’s local Work and does not synchronously refresh an external tracker.

The connected Machine’s hostname replaces the fixture label in both screens.
Choose **Machine connection → Change address** if Desktop shows a new QR code.
Scan it or enter the new address, then choose **Update address**. Loopflow verifies
that it is the same Machine before replacing the saved connection. Cancel or a
failed connection leaves the old address intact. Tasks and Session drafts stay on
the same Machine.
**Forget Machine** removes the saved connection. Manage device access in Tailscale;
Desktop’s **Turn off mobile access** disconnects all remote clients.
Backgrounding closes the attachment. Returning to the app automatically restores
the same Session as an observer, including its current draft and terminal size.
If that attempt fails, **Reconnect** tries again; no retry loop runs while away.
Leaving the Session screen cancels restoration. Ended or unavailable Sessions
keep their explanation.
Input and geometry require a fresh grant. Failed input is never queued or retried.
If opening or reconnecting fails, **Reconnect** stays on the Session screen.
Inspect the restored prompt before resubmitting input whose delivery was uncertain.
Missing or mismatched Sessions retain their explanation without a retry.

The app uses the shared Loopflow fonts, Light/Dark themes and WorkViewPlan types.
`MobileClient` is an injectable UI boundary, not a serialized protocol.
`ServerProtocol.swift` and `tests/fixtures/dto/server/` are the shared revision-5
contract; client-authored Task data passes through that codec and is not a
recorded Machine reading. The live adapter keeps one control WebSocket to the
Machine’s server while the app is in use (requests, the Task subscription,
cancel). `session.attach` resolves the current owner and issues a ticket; a
second connection redeems it with `attach`, reads `bridged`, and carries the
owner’s binary protocol. Reconnect starts from the Session ID, so no owner ID
is kept across attachments. A missing owner offers explicit Open Session;
navigation and reconnect never start a provider. Messages over 1 MiB travel by ticket too. The server’s private
Unix socket is never forwarded to remote callers. OwnerStream consumes Rust’s binary frames
and nonce-qualified events; snapshots are assembled before rendering at the
owner’s geometry. Every reconnect requests a fresh screen. The renderer requests
the next output event as it consumes the stream; the adapter retains only the
current decoded batch instead of filling a second event queue. Input queues,
message sizes and connection deadlines are bounded.
Long-lived Task updates and terminal streams check that the Machine still answers
while the app is in use. A lost connection becomes a reconnectable error even if
no new output arrives. The checks stop when that stream closes or backgrounds;
they never replay a command or keystroke.
Terminal attachments also send the owner's `controlled` heartbeat every ten
seconds. Its reply reports whether anyone holds control; it never grants input
or geometry to the phone. An unanswered heartbeat closes the attachment.
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
uv run python scripts/test_ios.py
uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests/OwnerStreamTests
```

The runner generates the project, creates an iPhone on the latest installed iOS
runtime, runs the native tests, then removes only that test device. Existing
Simulators are untouched. It clears inherited `LF_*` and closes test stdin.
Extra arguments go directly to `xcodebuild`. The changed-aware local gate selects
the `ios` suite for phone, shared-model and protocol changes; CI runs it in
`loopflow-ui-test` and retains failed `.xcresult` bundles.

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

`LoopflowiOSUITests` operates the app with the offline peer: open a Task, take
keyboard control, type through the system keyboard, go Home, return as an observer,
then explicitly take control and continue typing. Captures show the preserved
draft and software-keyboard layout. The deterministic peer requests a steady
cursor so XCTest does not wait for a repeating animation to finish.
Touch-target and accessible-description audits cover the Task list and Session.
Separate tests check text contrast in both themes and preserve alternate-screen
cells while changing font size. Enlarged layouts also need screenshot review;
the simulator's contrast/clipping audit reports false positives on some fully
visible custom-font labels.

Device demo still owns CJK keyboard/dictation quality, physical key combinations,
VoiceOver navigation and sustained-output energy use. Simulator software-keyboard
layout and app lifecycle are exercised; that does not prove every IME or hardware
keyboard. The owner decoder consumes a Rust-generated binary fixture across fragmented
reads. Address setup, Keychain persistence, Machine identity and empty
Task reads have passed in Simulator against an isolated service. Native editing
and reconnect have also passed through a real retained owner, including a newer
viewer holding input and 120×40 geometry. The child receives the combined draft
exactly once and no reconnect starts a provider. These peers use the actual
protocol; this is not yet a manual Desktop-to-physical-phone demonstration.
Native input under sustained output and foreground service interruption are
exercised in Simulator; physical network changes and phone use remain acceptance
work. Contextual approvals/Stop stay absent until the
backend supplies their action semantics; terminal prompts remain
answerable through ordinary typing.


### Opt-in host Tailscale acceptance

```sh
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  LOOPFLOW_MOBILE_SIMULATOR='<booted simulator UUID>' \
  LOOPFLOW_MOBILE_ENDPOINT=https://machine.tailnet.ts.net:8443 \
  LOOPFLOW_MOBILE_STATUS_FILE='<private host status JSON>' \
  LOOPFLOW_MOBILE_CERTIFICATE_FILE='<private host certificate/key PEM>' \
  cargo test -p loopflow --test transport_tests mobile_machine_native_acceptance \
    -- --ignored --exact --nocapture < /dev/null
```

Generate the project and install the signed app in that simulator first. Use an
unused host port, its real Tailscale DNS name and a private temporary directory.
Capture `tailscale status --json` into the status file. Capture
`tailscale cert --cert-file=- --key-file=- HOST` directly into the private PEM file,
never a terminal/log; remove both files afterward. No Serve route is needed.

The test creates a private registry, Task and retained provider fixture, starts
real HTTPS/WebSocket service transport, transfers the address into the app and
runs native XCTest. The host CLI responses are fixtures: the self-hosted Simulator
is represented as a second same-owner node. Real production rejects same-host
network identity because it cannot distinguish local OS accounts. This proof
therefore covers native TLS, protocol, editing and recovery; it does **not** prove
physical-device Tailscale identity or cellular/Wi-Fi behavior. It stops its owners
and service and deletes its registry afterward. The native test
opens the Task’s Session in UIKit, edits the existing draft, disconnects, lets a
second viewer take control, reconnects passively at the owner’s geometry, then
explicitly takes control and submits. While that draft is unfinished, the host
runner kills its isolated Machine service. The terminal and a separate idle Task
watch must report disconnection, preserve their last contents and reject offline
input. Retry while offline must fail honestly. After the service restarts, refresh
and reconnect restore current Tasks and the same draft without taking control.
Only the test runner handles those service transitions; the app has no remote
service-control API. The test also explicitly reopens the saved Session after
its first provider exits, then creates a Task through the native
creation model, recovers the same receipt, opens its recorded terminal, and types
into it. It also saves that request for later, reloads it and opens it while
preserving a separate unsent draft. Rust asserts exactly three provider launches
(initial, explicit continuation, new Task) and every exact submission. The native
log is `swift/.build/mobile-acceptance.log`; the isolated server's connection
diagnostics remain in `swift/.build/mobile-service.log`, including after a
failed handshake or service restart.

For an actual provider, use the same host/Simulator variables, additionally set
`LOOPFLOW_MOBILE_CLAUDE` to the absolute path of an already signed-in Claude CLI,
and select `mobile_machine_claude_acceptance`. Set
`LOOPFLOW_MOBILE_ACCOUNT_HOME="$HOME"` and `LOOPFLOW_MOBILE_ACCOUNT_PATH="$PATH"`
before starting Cargo: ordinary tests replace HOME and install a Keychain stand-in.
Only the explicitly selected provider uses this account and original PATH; the
Machine registry and other fixture programs remain isolated. This uses the configured
account for a new conversation in a disposable repository. Tools, MCP servers and
unmanaged customizations are disabled; no existing conversation is resumed and
no credential is copied. The provider uses its normal home while all Loopflow
records stay in the private registry. Its test transcript remains in Claude's
ordinary history. Native editing, service interruption, passive foreground return
and a context-dependent reply must keep one provider process. This optional check
needs network/account availability and is never part of ordinary CI.

For Codex, set `LOOPFLOW_MOBILE_CODEX` to its signed-in executable and select
`mobile_machine_codex_acceptance` with the same host/Simulator variables. This
runs a fresh conversation through Loopflow's private app server and native TUI,
preserving both processes across the same editing/recovery checks. The wrapper
disables daemon auto-start, hooks, plugins, apps, multi-agent, shell, browser and
computer-use features; the prompt requests replies without tools. It uses the
normal account home and retains the test transcript there. Existing conversations
and the shared Codex daemon are not selected. This is also opt-in, never ordinary CI.

The deterministic stand-in acceptance also includes a 32 MiB output stream paced
by an active viewer.
A second viewer stops reading, disconnects under the owner's bounded policy, then
reattaches to the current screen without taking control. The active viewer feeds
the mounted native terminal through MobileWorkspace and verifies its final screen.
Every 64 KiB, native text input inserts, deletes and submits one acknowledgement;
the remote program checks all 512 lines exactly. It records feed duration,
main-actor scheduling intervals and input-to-next-output latency. These are
Simulator measurements, not physical typing, GPU frame cadence or phone energy.
The carrier also checks that an expired read
reports a timeout and an explicitly canceled read stays cancellation.

`MachineConnectionIntegrationTests` skips unless the installed simulator app’s
Documents directory contains `LoopflowIntegration/address.txt`. Supply only the
HTTPS address of an **isolated, empty** development Machine registry with the
identity fixture above. The test removes the setup file, connects, remembers the
verified Machine identity and reads the empty Task list twice. When the Rust
fixture also supplies session.json, it runs the retained-owner scenario instead.
It restores any prior Simulator connection. An empty Task list alone does not
prove Session attachment.
