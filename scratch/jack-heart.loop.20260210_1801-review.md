# StepRunner: Split Run/Auto Buttons

## What was implemented

Replaced the four-pill stimulus picker (Once / Loop / Watch / Schedule) with two distinct action buttons:

- **Run** — single execution, always visible, always clear
- **Auto** — split button with dropdown to select loop/watch/cron mode

The "once" stimulus is now the Run button's dedicated action. The auto button defaults to Loop and remembers the last-selected auto mode. Cron field appears conditionally when Schedule is selected.

## Key choices

**Split button over pills.** The old stimulus picker mixed "what kind of run" with "go." Users had to pick a pill, then hit a separate run button. The split button collapses selection and execution into one click for the common case (Run or Loop), while the dropdown provides access to watch/cron without cluttering the primary action.

**Removed "Once" from auto options.** Once is just Run. Having it in both places was confusing. The auto button only offers loop/watch/cron — modes that imply ongoing execution.

**Default auto mode is Loop.** When a wave has `manual` or `once` stimulus, the auto button defaults to `.loop` rather than showing a meaningless state. `normalizedAutoMode(from:)` handles this mapping.

**Removed OutputBuffer environment dependency.** StepRunner no longer reads from OutputBuffer, so the `@Environment` was removed. The preview was updated to match.

## How it fits together

StepRunner is the primary execution panel in Concerto's wave detail view. It sits below the wave configuration (area, direction) and above the output/terminal. The two buttons map directly to the daemon API: `runWith(stimulus:)` sends the selected stimulus kind to `repoState.runWave()`.

Auto mode metadata (label, icon) is derived inline via `autoLabel(for:)` and `autoIcon(for:)`. The static `autoModeKinds` array (`[.loop, .watch, .cron]`) drives both the dropdown menu and constrains valid auto modes.

## Risks and bottlenecks

- The split button is a custom composite — `HStack` with two `Button`s and a `Menu`. No native SwiftUI split button exists. The divider is a 1px `Rectangle`. This works but could look off if system font scaling changes significantly.
- `buttonsDisabled` gates both buttons identically. If a use case arises where Run should be enabled but Auto shouldn't (or vice versa), the logic will need to split.
- `isSendingRun` tracks the API call, not the wave's running state. There's a brief window between the API call completing and the wave status updating where buttons re-enable. Acceptable for now — the wave status check in `buttonsDisabled` catches up quickly.

## What's not included

- No stop/cancel button for running waves (existing gap, not introduced here).
- No visual indicator on the auto button showing the wave is actively looping/watching. The wave status badge elsewhere in the UI covers this.
