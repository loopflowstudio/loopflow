# Review: make Ghostty typing terminal-native

## What was implemented

- `GhosttyMetalView` now keeps ordinary printable typing on the `ghostty_surface_key` path instead of routing it through AppKit text insertion by default.
- `insertText` remains the narrow path for IME commits and explicit paste, so composition and paste semantics stay separate from direct typing.
- Terminal-focused keyboard routing still lets Concerto keep intentional multiplexer shortcuts, and the help overlay now wins over terminal focus so dismiss keys keep working while a pane is active.
- Added regression coverage for the Ghostty typing helpers and terminal keyboard-mode behavior.

## Key choices

- **Direct keys first for normal typing.** Printable single-key input bypasses `interpretKeyEvents` unless composition is active, which keeps terminal apps from seeing ordinary typing as paste-like text injection.
- **Text input stays for composition.** Marked text, `inputmethod` keyboard sources, and Option-modified printable keys still use the text-input path to preserve IME and dead-key composition.
- **Terminal shortcuts stay minimal.** In terminal mode, only `.multiplexer` shortcuts are intercepted by Concerto; everything else falls through to the terminal.
- **Overlay dismissal beats terminal focus.** The help overlay now resolves before terminal mode so `Esc`/`?` still dismiss it even when Ghostty owns first responder.

## How it fits together

`GhosttyMetalView.keyDown` now chooses between two paths: direct `ghostty_surface_key` dispatch for ordinary typing, or AppKit text input for composition-capable input. `KeyboardRouter` sits above the terminal surface and only steals the intentional pane-management shortcuts, with tests covering both the Ghostty helper decisions and the responder-mode priority rules.

## Risks and bottlenecks

- The IME heuristic currently keys off `selectedKeyboardInputSource` identifiers containing `inputmethod`; unusual input sources may still need follow-up validation.
- Option-modified printable keys stay on the AppKit text path to preserve dead-key accents; that is intentional, but it narrows the direct-key path on some layouts.
- Full macOS UI test execution is still noisy in this environment: the `ConcertoUITests-Runner` crashes before XCTest connects, even though the app/unit targets build and the unit test targets pass.

## Validation

- `swift test --package-path swift` ✅
- `cd swift && xcodegen generate` ✅
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests` ✅
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoUITests/ScreenshotPipelineTests/testCapture` ⚠️ runner crashed during bootstrap (`signal kill` before XCTest connected)

## Wave alignment

This directly advances the agent-embedding wave goal that coding sessions should happen in embedded Ghostty terminals that feel like real terminals, not chat wrappers. The work reduces one of the wave README's explicit product risks: the terminal feeling like a hosted text view instead of the primary input surface.

## What's not included

- No new remote-terminal transport work.
- No broader shortcut redesign beyond preserving existing multiplexer overrides and fixing help-overlay priority.
- No automated IME end-to-end UI coverage yet; manual Japanese/Korean/Chinese validation is still recommended before merging.
