# Review: iOS action button wiring

## What was implemented

- Wired `ActionButtonsView` into `MobileWaveDetailView` via a bottom `.safeAreaInset(edge: .bottom)` rail so suggested actions are available while viewing Output.
- Hoisted session lifecycle in iOS detail view:
  - resolved `sessionState` once per detail render path,
  - configured client context from size class,
  - called `sessionState.onAppear()` / `sessionState.onDisappear()` at detail scope.
- Added `WaveSessionView` configuration knobs:
  - `showsSuggestedActions` (default `true`),
  - `managesLifecycle` (default `true`).
  This keeps macOS behavior unchanged while iOS chat tab opts out of duplicate action buttons/lifecycle.
- Updated iOS `RepoState` convenience init to call the designated initializer explicitly.
- Marked Stage 03 pre-req complete by linking `wave/mobile/03-multi-client.md` to `scratch/mobile-ios-action-buttons.md`.
- Fixed iOS list view build blockers surfaced during validation:
  - wrapped `MobileWaveListView` body content in `Group` so view modifiers apply cleanly,
  - made `.refreshable` closure `@MainActor` to satisfy Swift 6 sendability checks.

## Key choices

- **Single action surface on iOS:** action rail is always at bottom of detail view; chat tab disables internal suggested actions.
  - Alternative rejected: keep actions chat-only (hid primary mobile interaction behind tab switch).
- **Reuse existing action send path:** action taps call `await sessionState.sendSuggestedAction(action)`.
  - Alternative rejected: iOS-specific action transport/parsing (would diverge from macOS semantics).
- **Lifecycle owned by detail container:** session lifecycle no longer depends on entering Chat tab.
  - Alternative rejected: leave lifecycle in `WaveSessionView` on iOS (stale/missing actions while in Output).

## How it fits together

`MobileWaveDetailView` now owns session lifecycle and renders suggested actions in a bottom safe-area inset. The Chat tab still uses shared `WaveSessionView`, but iOS passes `showsSuggestedActions: false` and `managesLifecycle: false` so only one action surface exists and lifecycle logic is not duplicated. `SessionState` remains the single source of truth for suggested actions and sending behavior, preserving macOS/iOS parity.

## Risks and bottlenecks

- Visual verification remains manual: headless runs/builds cannot confirm thumb-zone spacing, overlap with keyboard, or tap ergonomics.
- `.task` lifecycle in `MobileWaveDetailView` still depends on SwiftUI view identity behavior; future navigation refactors should ensure task lifetime remains wave-scoped.
- Existing warning in iOS build path (`bannerIcon != nil` always true) is pre-existing and unrelated to this change.

## What's not included

- No new suggested-action generation or reconciliation logic.
- No multi-client stale-action conflict handling.
- No macOS interaction changes.
- No discovery/reconnection UX redesign.
