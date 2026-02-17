# Keyboard Shortcuts: Current State and Follow-Ups

## Scope

Concerto now has app-wide keyboard routing for repo windows:

- Single-key navigation and actions (J/K, C/E/D/R/S/L/N, T/I/F/P, 1/2, `/`, `?`)
- Chords (`G H`, `G L`) with timeout and indicator
- Mode-aware pass-through for text editing, terminal focus, and command palette focus
- Help overlay (`?`) and no-wave feedback toast for wave-required actions

## Architecture Decisions to Keep

- One shared `KeyboardRouter` monitor for the app process; dispatch by active `windowNumber`
- Shortcut matching via normalized key + normalized modifiers, with repeat guarded per binding
- Notification-driven integration in existing views (`WaveSidebar`, `WaveDetailPanel`) to avoid duplicate key handlers
- Help/chord overlay visibility rendered only in the active key window

These decisions are the baseline for any future keyboard work.

## Remaining Work

1. **Per-window router state**  
   Help/chord state still lives in a shared router object. Rendering is scoped to key window, but state ownership is not.

2. **UI automation coverage**  
   Add UI tests for keyboard flows once `ConcertoUITests-Runner` automation mode is stable in CI/dev environments.

3. **Ghostty responder detection hardening**  
   Current terminal detection relies on responder type-name matching and should move to a less brittle signal if available.

## Validation Snapshot

- `swift test --package-path swift` ✅
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -only-testing:ConcertoTests` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'` ⚠️ fails in this environment: `Timed out while enabling automation mode` (`ConcertoUITests-Runner` init)
