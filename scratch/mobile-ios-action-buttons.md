# iOS Action Button Wiring

## Problem

`ActionButtonsView` is shared and already live in macOS `WaveSessionView`, but iOS still treats suggested actions as a chat-only affordance. In `MobileWaveDetailView`, the default surface is Output, so users have to switch context before they can use the most important mobile interaction.

This blocks Stage 03 multi-client validation because feature parity is incomplete: Mac can act directly on suggestions, iOS cannot do so in a thumb-reachable, always-available location.

Who benefits now:
- iPhone users who need fast “tap, don’t type” control while away from keyboard
- iPad users running side-by-side monitoring flows
- Stage 03 validation (both clients exercising the same session-action path)

## Approach

Move suggested actions to a persistent bottom action rail in `MobileWaveDetailView`, backed by the same `SessionState` and send path as macOS.

1. **Hoist session ownership in `MobileWaveDetailView`.**
   - Resolve `let sessionState = repoState.sessionState(for: wave.id)` once per render path.
   - Manage session lifecycle at the detail level (`configureClientContext`, `onAppear`, `onDisappear`) so suggested actions can appear even when Output tab is active.

2. **Render a bottom safe-area action rail in `MobileWaveDetailView`.**
   - Use `.safeAreaInset(edge: .bottom)` to pin `ActionButtonsView` near the thumb zone.
   - Show only when `!sessionState.suggestedActions.isEmpty`.
   - Keep styling lightweight (existing design tokens, no new visual system).

3. **Keep one action surface per screen.**
   - Add a `showsSuggestedActions` flag to `WaveSessionView` (default `true` for macOS).
   - For iOS chat tab usage in `MobileWaveDetailView`, pass `showsSuggestedActions: false` to avoid duplicate action stacks.

4. **Use the existing send path exactly.**
   - On tap: `await sessionState.sendSuggestedAction(action)`.
   - No new action parsing, no alternate transport, no platform-specific branching in the action logic.

5. **Validate form factors.**
   - iPhone: vertical stack, full-width buttons from `ActionButtonsView` compact behavior.
   - iPad: adaptive grid behavior from `ActionButtonsView` regular behavior.

Research and precedent informing this choice:
- Current architecture already standardizes action semantics in `SessionState.suggestedActions` + `sendSuggestedAction`.
- `ActionButtonsView` already implements size-class-specific layout and accessibility timing gates.
- Wave vision explicitly prioritizes action-button-first mobile UX with chat as secondary.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep actions only inside `WaveSessionView` (chat tab only) | Minimal code change | Fails mobile “action-first” intent; actions remain hidden in primary Output flow |
| Build iOS-specific action buttons in `Concerto/Platform/iOS` | Fast local tuning | Duplicates shared component and increases divergence from macOS behavior |
| Replace tabs with a fully new mobile interaction shell | Strong product reset | Too large for this pre-req; delays Stage 03 validation unnecessarily |

## Key decisions

- **Decision: anchor actions in `MobileWaveDetailView` with `safeAreaInset`.**
  - Why: guarantees bottom placement on iPhone and iPad without fragile spacer math.

- **Decision: lifecycle belongs to the mobile detail container, not the chat tab view.**
  - Why: actions must update while users are watching Output, not only after entering Chat.

- **Decision: preserve one shared action send implementation (`SessionState.sendSuggestedAction`).**
  - Why: parity and correctness are more important than mobile-only behavior tweaks.

- **Wild success scenario:** users monitor output, tap suggested actions in-place, and rarely open Chat for routine flow control.

- **Wild failure scenario:** stale or duplicate buttons erode trust (e.g., chat-only lifecycle, two button stacks, or buttons hidden under safe area).
  - Mitigation in this design: lifecycle hoist, single-surface rendering toggle, bottom safe-area inset.

## Scope

- In scope:
  - `MobileWaveDetailView` embeds `ActionButtonsView` at bottom
  - iOS action taps use `SessionState.sendSuggestedAction`
  - `WaveSessionView` gains an opt-out for internal suggested action rendering
  - iPhone + iPad layout verification

- Out of scope:
  - New suggested action generation logic
  - Multi-client stale-action reconciliation logic (separate Stage 03 item)
  - macOS interaction changes
  - Discovery, reconnection, or connection-profile UX work

## Done when

- Observable outcomes:
  - Suggested actions appear at the bottom of `MobileWaveDetailView` when available
  - Tapping an action sends the corresponding message through existing session input flow
  - iPhone (compact) and iPad (regular) both render correctly
  - macOS behavior remains unchanged

- Verification:
  - `swift test --package-path swift`
  - `uv run python scripts/check_swift_multiplatform_boundaries.py`
  - `uv run python scripts/concerto-dev.py run-ios` (manual iPhone + iPad simulator check)
