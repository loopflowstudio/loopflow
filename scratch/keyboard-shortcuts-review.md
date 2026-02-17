# Keyboard Shortcuts Review (branch: `keyboard-shortcuts`)

## What was implemented

- Added an app-wide keyboard routing system (`KeyboardRouter`) using a single `NSEvent` local monitor with per-window handler registration.
- Added shortcut and chord models/catalog (`ShortcutAction`, `ShortcutBinding`, `ChordBinding`, `ShortcutCatalog`) with normalized key/modifier matching, repeat control, and slash-key layout fallback.
- Wired shortcut handling into `ContentView` with action dispatch to sidebar/detail behaviors (navigation, wave actions, tools, tabs, command palette, help).
- Replaced sidebar-local key handlers with notification-driven focus/select behavior (`WaveSidebar`) so keyboard routing has a single source of truth.
- Added a shortcut help overlay (`ShortcutHelpOverlay`), chord indicator UI, and no-wave feedback toast for actions that require selection.
- Added per-window `WindowAccessor` utility and reused it in `RepoWindow`, `ScreenshotWindow`, and `ContentView`.
- Updated command palette shortcut affordance (`⌘K /`) and added tab-switch notification handling in `WaveDetailPanel`.
- Added unit tests for key normalization, repeat behavior, chord behavior, mode routing, and help-overlay dismiss logic (`KeyboardRouterTests`).

Additional polish in this gate pass:
- Removed unused keyboard notification constants.
- Stopped swallowing launcher errors in command palette actions (now surfaces error messages).
- Ensured menu-driven command palette open closes help overlay.
- Scoped help/chord overlay rendering to the active key window to avoid duplicate overlays in background windows.
- Added a keyboard shortcut reference section to `swift/README.md`.

## Key choices

- **Single app-wide monitor + window dispatch**: prevents duplicate handling and keeps action execution bound to the key window.
- **Mode-based routing** (`textEditing`, `terminal`, `commandPalette`, `helpOverlay`, `global`): keeps typing contexts safe and predictable.
- **Notification-based integration** for existing views: minimized invasive rewrites while replacing overlapping local key handlers.
- **Requires-wave guardrail** in `ContentView`: avoids destructive no-op confusion and gives throttled, subtle feedback.

Alternatives considered implicitly by design:
- Per-view `.onKeyPress` everywhere (rejected due duplication/conflicts).
- Focus-state plumbing per text input (rejected in favor of responder inspection).

## How it fits together

`ConcertoApp` injects one shared `KeyboardRouter` into repo windows. Each `ContentView` registers/unregisters its window handler via `WindowAccessor`. The router intercepts key events, resolves mode from first responder + overlay/palette state, matches shortcuts/chords, and emits `ShortcutAction`. `ContentView` translates actions into repo operations or notifications, and `WaveSidebar`/`WaveDetailPanel` consume those notifications to move focus, select waves, rename, and switch tabs.

## Risks and bottlenecks

- **UI automation environment sensitivity**: full UI-test run currently fails in this environment with `Timed out while enabling automation mode` (runner init), so keyboard UX coverage is primarily unit-level plus compile/test coverage.
- **Global help/chord state in shared router**: rendering is now constrained to the active key window, but router state is still shared across repo windows.
- **Ghostty responder detection by type-name substring**: practical but string-based and therefore somewhat brittle to implementation renames.

## What's not included

- No migration to a fully per-window help/chord state model inside `KeyboardRouter`.
- No additional UI automation tests for shortcut flows (blocked by current automation-mode timeout in this environment).
- No broader refactor of notification-driven action plumbing beyond keyboard-related paths.

## Validation run

- `swift test --package-path swift` ✅
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -only-testing:ConcertoTests` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'` ⚠️ failed in environment: `Timed out while enabling automation mode` during `ConcertoUITests-Runner` initialization.
