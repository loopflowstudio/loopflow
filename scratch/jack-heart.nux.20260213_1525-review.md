# Review: StartWaveView — Replace Quick Experiment Landing

## What was implemented

Replaced the Quick Experiment system (detail panel + sidebar empty state) with a minimal "Start a wave" text field. Deleted `QuickExperimentView.swift` (211 lines), removed `launchQuickExperiment()` from both `ContentView` and `WaveSidebar`, and simplified the sidebar empty state.

New `StartWaveView` is 80 lines: heading, text field, error display, wave creation. Typing a name and pressing Enter creates a wave. Empty name falls through to auto-generated name via `NameGenerator`.

## Key choices

**Single text field over step buttons.** The Quick Experiment step launcher (design/review/debug/implement buttons) was removed entirely. The new landing funnels users toward creating a wave — the core unit of work — rather than running one-off terminal commands. This simplifies the onboarding mental model.

**Duplicated wave creation logic.** `StartWaveView.createWave()` and `WaveSidebar.createWaveDirectly()` share the same pattern (connect lfd if needed, create wave, post editWaveName notification). The only difference: `StartWaveView` passes `waveName` while `WaveSidebar` passes empty string. Extracting a shared helper would add indirection for two call sites — not worth it yet.

**Stripped `LoggingService` calls from `createWaveDirectly()`.** The verbose UI logging was development scaffolding. The error path still surfaces failures via alert.

**Removed `OutputBuffer` dependency from `ContentView` and `ScreenshotLayout`.** These views no longer need it since `launchQuickExperiment()` was deleted. `StartWaveView` injects its own `OutputBuffer` from the environment.

## How it fits together

`ContentView.detailContent` and `ScreenshotLayout.detailContent` show `StartWaveView()` when no wave is selected. The sidebar empty state shows "No waves yet" + a Create Wave button. Both paths create waves through `RepoState.createWave(name:)`, ensuring consistent behavior.

## Risks and bottlenecks

**Text field auto-focus.** `isTextFieldFocused = true` on appear may conflict with sidebar keyboard navigation if both compete for focus. In practice, the detail panel only shows `StartWaveView` when no wave is selected, so sidebar keyboard nav isn't active.

**500ms sleep after lfd connect.** Both `StartWaveView` and `WaveSidebar` use `Task.sleep(for: .milliseconds(500))` after connecting lfd. This is a race condition workaround — if lfd startup takes longer, wave creation will fail. Not new to this branch, but worth noting.

## What's not included

- No changes to `RepoState` or `LoopflowCore` — the existing `createWave(name:)` API handles named creation.
- No removal of `TerminalLauncher` or `TerminalApp` types — they're still used by the command palette.
- The sidebar empty state keeps a "Create Wave" button as fallback. The design doc suggested this could be omitted (just header "+" button), but keeping it provides discoverability for new users.
