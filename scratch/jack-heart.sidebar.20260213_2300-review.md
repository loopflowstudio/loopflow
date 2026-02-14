# Sidebar cleanup — review

## What was implemented

Simplified the wave sidebar from 5 groups (Blocked, Open PRs, Recent Activity, Active, Idle) to 2 (Active, Idle). Removed noise from WaveRow: live output, iteration count, PR limit indicator, and keyboard focus border overlay.

## Changes by file

| File | What changed |
|------|-------------|
| `WaveStore.swift` | `WaveGroups` collapsed from 5 fields to 2. `recompute()` reduced to a simple idle/non-idle partition. Removed `attentionCount`, `openPRCount`, and the time-window/priority sorting logic. |
| `WaveRow.swift` | Removed `isKeyboardFocused` property, `SidebarLiveOutput` struct, iteration count display, PR limit indicator, keyboard focus border overlay, and `statusHelpText` computed property. |
| `WaveSidebar.swift` | `waveList` renders two sections (Active, Idle) instead of five. Removed `isKeyboardFocused` parameter from `WaveRow` calls. Removed attention count badge from header. |
| `WaveRowTests.swift` | Removed `showsIteration` and `showsPRLimitWhenWaiting` tests. Simplified `makeWave` helper (status/iteration no longer parameterized). |
| `WaveStoreTests.swift` | Replaced PR grouping test with a status-based grouping test confirming idle waves with open PRs land in the idle group. |

## Key choices

- **Status-only grouping**: Waves are now grouped purely by status (idle vs non-idle). The old grouping mixed status, PR state, and recency into overlapping categories — a wave could qualify for multiple groups. The new model is a clean partition.
- **Kept `iterationText` on WaveViewModel**: The computed property still exists in LoopflowCore (with its tests) even though the sidebar no longer displays it. It's a public API on a shared model — removing it is a separate decision.
- **Kept keyboard navigation**: `keyboardFocusedId` state and arrow-key handling remain in WaveSidebar. Only the visual focus *border overlay* on non-selected rows was removed. Keyboard nav still works via selection.

## How it fits together

WaveStore owns the canonical wave state. `recompute()` derives `WaveGroups` on every mutation. WaveSidebar reads `groups` to render sections. WaveRow is a pure view — takes a wave, selection state, and callbacks.

The data flow is: WebSocket events → `WaveStore.set/setAll` → `recompute()` → `groups` update → SwiftUI re-renders sidebar sections → WaveRow renders each row.

## Risks

- **None identified.** The change is purely subtractive. All 94 Swift package tests and 93 Concerto UI tests pass. No new code paths.

## What's not included

- Sorting within groups (active waves are unsorted — dictionary iteration order). If ordering matters, that's a follow-up.
- Removing `iterationText` from WaveViewModel/LoopflowCore.
- Any changes to WaveDetailPanel or other views that may still reference the old group concepts.
