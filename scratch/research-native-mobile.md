# Native terminal integration for Loopflow mobile

Research for Jack Heart · 2026-11-03 · Source inspection; no builds or device trials

Historical comparison before implementation. [mobile-ios.md](mobile-ios.md) records
the subsequent SwiftTerm selection, build/input proof and remaining device work.

Jack selected a terminal as the core mobile experience. **Recommendation:** spike Lakr233/libghostty-spm behind a small Loopflow transport adapter, with native Machine/Session navigation and Task context around it. Compare Termini on the same byte stream. Use SwiftTerm as an accessibility and input baseline, not an automatic replacement. Study Rootshell and gterm for concrete interaction techniques without importing their SSH-centric application architectures.

## The useful boundary

```text
Native navigation and Task context
        │ selects Session
Loopflow attachment controller
        │ ordered terminal bytes / input / resize intent
UIKit terminal surface + Ghostty renderer
```

The attachment controller owns authorization, input-control state, reconnect cursors and snapshot replacement. The terminal library owns parsing, rendering, text selection, keyboard encoding and local viewport presentation. This keeps a terminal implementation change from changing which remote Session runs.

Current [Swift packaging](../swift/Package.swift) includes a shared iOS-compatible library, but GhosttyKit is a macOS dependency of LoopflowMac and no mobile application target exists. The [Desktop patches](../swift/GhosttyKitPatches/README.md) implement command-block selection, bounded text extraction, colors and feedback against a pinned Ghostty build. They are useful prior work, not an iOS integration already waiting to be enabled.

Loopflow's server-side [libghostty-vt pin](../rust/loopflow/ghostty-vt.json) holds terminal state independently of a renderer. The [attachment protocol](../rust/loopflow/src/transport/README.md) repaints a new viewer through VT bytes, then streams output; its on-disk Ghostty snapshot is a different representation. Mobile should consume the attachment stream, not decode the server's private snapshot file format. Reattachment currently excludes images and limits retained scrollback; choosing a renderer with graphics support does not remove those server limits.

Upstream Ghostty 1.4 builds libghostty-vt for iOS and has an XCFramework, but its C API remains work in progress with compatibility breaks. That is evidence of a usable emulator foundation, not a complete UIKit renderer/input toolkit. Pin the chosen revision and artifact. [Ghostty 1.4 notes](https://ghostty.org/docs/install/release-notes/1-4-0)

## Candidate comparison

| Candidate | Evidence of fit | Decision risk |
|---|---|---|
| Lakr233/libghostty-spm | UIKit/AppKit wrapper, host-managed I/O, touch selection and native input implementation | Another patched Ghostty distribution; VoiceOver quality and Loopflow patch parity need proof |
| Termini | Explicit controller callbacks for output, outgoing bytes and size; SwiftUI surface | Different Ghostty fork/artifact and narrower wrapper; test input details rather than infer them |
| SwiftTerm | Swift/UIKit terminal with actual accessibility reading and scrolling implementation | Separate emulator behavior; compare fidelity to Loopflow's server-generated VT |
| Rootshell / gterm | Complete mobile terminal apps with real keyboard, selection and lifecycle patterns | Broader app architecture and maintenance assumptions than Loopflow needs |

Lakr's [`InMemoryTerminalSession`](https://github.com/Lakr233/libghostty-spm/blob/main/Sources/GhosttyTerminal/InMemory/InMemoryTerminalSession.swift) accepts write and resize callbacks, supports incoming output and exposes viewport reading. This is the transport seam Loopflow needs; it does not require launching a shell on the phone. Its [package](https://github.com/Lakr233/libghostty-spm/blob/main/Package.swift) requires Swift 6.2 and declares iOS 15. It exports GhosttyKit, a name already used by Loopflow's binary target: resolve package/module separation before integrating, and do not link two incompatible Ghostty C implementations into one app.

Termini's [controller](https://github.com/hudsonkit/Termini/blob/main/Sources/Termini/TerminiTerminalController.swift) accepts arbitrary output bytes and exposes outgoing-byte/resize callbacks. Its README's “SSH only” iOS wording describes built-in backend choices, not a prohibition on custom WSS. The [manifest](https://github.com/hudsonkit/Termini/blob/main/Package.swift) declares iOS 17, Swift tools 5.9 and a separate patched GhosttyKit artifact; its SSH product is separable from the renderer target. Compare the integration cost rather than selecting based on advertised transport.

Both packages have MIT top-level licenses, but adoption still requires reviewing bundled binary notices, upstream patches and reproducible build sources. Existing Loopflow third-party packaging should govern that integration.

## Input, selection and accessibility

Lakr's [UIKit input code](https://github.com/Lakr233/libghostty-spm/blob/main/Sources/GhosttyTerminal/Platform/UIKit/UITerminalView%2BUITextInput.swift) implements marked-text composition. It routes a lone software Return through key handling while keeping multiline dictation/paste on the text path, and suppresses duplicate hardware-key input. This is substantive native input work. It does not prove every IME or dictation correction behaves correctly on the target iPhone.

Preserve the distinction among a key event, committed text and paste. Optional native draft composition can help edit dictated instructions, but Send must be explicit and paste must honor terminal modes. Raw terminal input remains immediate; a reconnect must never replay uncertain keys or silently send a saved draft. Test emoji, combining characters, Japanese conversion, Korean composition, dictation revisions, Ctrl-C, Escape, Option combinations and multiline paste against an actual agent TUI.

Rootshell documents a compose overlay, dictation/CJK preedit, sticky modifiers and touch selection. These are relevant product references; its broader VPN, SSH and workspace features are separate decisions. [Rootshell](https://github.com/kitknox/rootshell)

gterm provides a smaller study: custom passthrough Ghostty I/O, an accessory keyboard, and keyboard tests using a remote hex-input fixture. Its documentation distinguishes simulator assertions from physical trackpad/keyboard feel. Borrow that test distinction and compact keyboard controls. [gterm](https://github.com/madeye/gterm)

SwiftTerm's [iOS view](https://github.com/migueldeicaza/SwiftTerm/blob/main/Sources/SwiftTerm/iOS/iOSTerminalView.swift) implements `UIAccessibilityReadingContent`, line/page content and accessibility scrolling. Its [input implementation](https://github.com/migueldeicaza/SwiftTerm/blob/main/Sources/SwiftTerm/iOS/iOSTextInput.swift) explicitly addresses marked text and dictation. These make it a useful comparison baseline. The inspected Lakr view's `accessibilityValue` writes were debug UI-test instrumentation, not proof of production VoiceOver support. Neither source inspection establishes comfortable spoken navigation through a busy TUI; test it.

One trap in Lakr's [paste behavior](https://github.com/Lakr233/libghostty-spm#pasting-and-dropping-files): files/images can become local paths pasted into the terminal. An iPhone sandbox path means nothing to a remote agent. Loopflow must intercept file attachment, transfer it explicitly and send a remote reference, or leave that interaction unsupported. Text paste is not file transfer.

## Lifetime, energy and native presentation

Observe first. Opening a phone Session should not resize a desktop TUI merely because the software keyboard appears. Route library resize callbacks through Loopflow's independent size-ownership contract; acquire input only through the existing attachment control path. Preserve the local scroll position independently of terminal dimensions and remote output position.

Lakr's [lifecycle implementation](https://github.com/Lakr233/libghostty-spm/blob/main/Sources/GhosttyTerminal/Platform/UIKit/UITerminalView%2BLifecycle.swift) listens to application and individual scene transitions. That matters for iPad multiwindow, but still needs testing in Loopflow's container.

Apple expects a backgrounding app to quiet its work and stop submitting new Metal work before suspension. Therefore save drafts and connection cursors, stop rendering hidden terminals and reconstruct the foreground view from fresh attachment evidence. A WebSocket does not grant perpetual background execution. [Apple lifecycle guidance](https://developer.apple.com/documentation/uikit/preparing-your-ui-to-run-in-the-background)

Completion notifications require an APNs provider path, separate from terminal traffic. Silent background notifications are low priority, may be throttled and are not guaranteed; they cannot be the reconnect or correctness mechanism. [Apple background updates](https://developer.apple.com/documentation/usernotifications/pushing-background-updates-to-your-app) Foreground-only mobile can ship without a hub; notification infrastructure can be a later independent responsibility.

Measure idle foreground GPU/CPU, sustained output, memory with several cached Sessions, background activity and foreground recovery on a physical phone. Desktop Ghostty benchmarks do not predict a fork's iPhone energy use. Keep one active terminal surface initially and make cached previews visibly stale.

## Smallest decisive spike

Build one native screen using Lakr, then substitute Termini behind the same transport adapter. Connect to one real Loopflow Session, decode nonce-qualified control events separately from terminal bytes, and preserve original cursor accounting. Exercise observation beside Desktop, explicit takeover, keyboard-induced layout changes, IME/dictation, copy/paste, VoiceOver and suspend/reconnect beyond retention. Check image and history limitations honestly.

Select the dependency on those results. Neither library supplies Machine authorization, command deduplication, remote file attachments, process survival, multi-client ownership or durable notification delivery. Those remain Loopflow architecture, even when rendering is borrowed successfully.
