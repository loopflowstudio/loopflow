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

Without `--fixture`, this build displays an unavailable Machine connection.
It contains no live network endpoint, pairing, provider launch or on-phone CLI.
The fixture label stays visible in both screens. Backgrounding closes the
attachment; **Reconnect** restores the same fixture Session as an observer.
Input and geometry require a fresh grant. Failed input is never queued or retried.

The app uses the shared Loopflow fonts, Light/Dark themes and WorkViewPlan types.
`MobileClient` is an injectable UI boundary, not a serialized protocol.
`MachineProtocol.swift` and `tests/fixtures/dto/machine/` come from LOO-593 at their
canonical paths. Fixture Task reads pass through that codec. The current local
service admits roadmap reads and resolves a Session to its existing owner. Live
WorkViewPlan admission still needs joining before a remote adapter can use this screen.
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
  -parallel-testing-enabled NO -only-testing:LoopflowiOSTests/MobileTerminalTests \
  CODE_SIGNING_ALLOWED=NO < /dev/null
```

Xcode 26.2/iOS 26.2 simulator is the tested toolchain; deployment target is iOS 18.
The pinned package's build plugin only generates source-control version metadata;
`-skipPackagePluginValidation` permits that reviewed plugin in headless builds.
The tests mount the actual UIKit terminal and assert editing, marked-text commit,
multiline paste, native replacement, selection, rendered accessibility text,
viewport resizing, control loss, identity and no replay. They also consume the
backend's shared Machine fixtures, without inventing a second codec.

Device demo still owns actual software-keyboard layout, CJK keyboard/dictation
quality, physical key combinations, VoiceOver navigation and sustained-output
energy use. The tests exercise UIKit input methods; they do not manufacture real
hardware presses or prove every IME. Live owner snapshots, canonical geometry
while observing, authenticated control handoff, bounded output recovery and
Tailscale are integration work. Contextual approvals/Stop stay absent until the
backend supplies trustworthy state and action semantics; terminal prompts remain
answerable through ordinary typing.
