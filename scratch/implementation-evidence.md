# Implementation evidence — 2026-09-23

## Scope and observable finish

The latest user direction is a scope-aware conversation entry, with a separate
ordinary terminal action. Opening a conversation no longer unconditionally
creates a Task/worktree. Two-level worktree/terminal layout and shell Session
attachment remain required. A button rename or fixture screenshot alone is
insufficient proof.

Implemented here: scoped inline interactive prompts honoring lf configuration;
per-checkout terminal layouts and outer splits; retained shells; terminal/client
association tied to the originating PTY; markup-free fallback Session titles.
The shell returns after agent exit or external handoff. Closing/undoing a launch
pane does not replay its initial command. Hidden groups report RUNNING rather
than VIEWING.

## Screenshot counterexamples

- The original two design clients were displayed in shell panes while both
  rows said ELSEWHERE. New provider-client receipts carry their attached terminal
  IDs only when the shell marker matches the actual PTY. The view resolves that
  association without moving or restarting the provider. Old already-running
  clients without the marker cannot be retroactively associated by cwd guessing.
- The newer screenshot showed planning unavailable from `/`. Source inspection
  found `roadmap --all` still applying inherited LF_WAVE_ID. Explicit `--all`
  now skips ambient Wave selection; explicit `--wave` remains respected.
- `lf ls --json` and `lf status <name> --json` show `list` and `engbot` as
  abandoned, disabled registrations with no Projects, Tasks, Runs or live
  listener. Neither has an authored GOAL.md. Their IDs are
  `9861d5b6-e53d-493d-a59b-cc8960ff88ed` and
  `4dbc0638-a441-489a-b095-f9e407ea01bf` respectively.

## Wave cleanup boundary

The user explicitly requested deleting these entries and updating the list
source. Added shared `lf ls --current` and `lf work forget wave <id>` with a
dry-run path. No UI-local lifecycle filter remains. Ordinary roadmap reads omit
historical Waves; an explicit Wave query still supports inspection.

Real cleanup is **not performed**. The installed CLI has no forget command.
`scripts/dev-lf` correctly uses an isolated development Home, so dry runs report
both IDs absent. Installed Home: `home_39860354aaca640c2ccb50bf6ca609d8`;
development Home: `home_da732804e6a182b9d9cdfcfb6ed67c8d`. Do not redirect a
development binary into the live store to get around its authority boundary.
After installing a version containing this command, preview both exact IDs,
forget them, then verify the live registry contains the three intended Waves.

## Verification

- Focused Swift run: 78 tests passed, including real Ghostty surfaces proving
  two shell-associated clients resolve here and another window resolves elsewhere.
- Rust PTY proof: repeated bindings on the original PTY succeed; mismatched PTY
  and detached stdin are rejected.
- CLI proof: roadmap --all works from `/` with valid and stale inherited Wave IDs.
- CLI cleanup proof: current read excludes history, dry run retains the record,
  forget removes only the empty abandoned registration, and current/populated
  Waves survive rejected deletion.
- Xcode build-for-testing succeeded with two workers. The aggregate runner was
  blocked by main's active 19.1 GiB cache; supported recovery left that active
  cache untouched. Direct focused Xcode compilation ran in this checkout.
- No real provider launch, external app handoff, or live client movement was
  performed. These are not claimed as proven by mocked or fixture argv.

## Integration with main-view

Read both the original `loopflow.main-view` design checkout and the current
`loopflow.main-view-task` implementation that matches the newer screenshot.
That branch's committed unified navigation is now integrated through a local
Loopflow rebase. The unified navigator remains mounted across presentation
changes; its toolbar now launches scoped conversations and ordinary terminals.
WorktreeNodeView and the retained registry own terminal grouping. No files in
either sibling checkout were edited, and its uncommitted launcher/docs work was
not imported.

Review found and corrected two integration issues: newly imported Session JSON
fixtures needed the required terminal_ids field; All work needed repository
launch context despite retaining the previous Task selection. Native focus is
also disabled while the terminal surface is hidden by list/details navigation.
The forget transaction now rechecks placement enabled state before deleting.

## Integrated proof and review

- Swift integration: initial run passed 82 of 93 tests; all 11 failures were
  imported navigation fixtures missing the newly required terminal_ids field.
  Updated the wire fixtures, then all 16 workspace/navigation/launch tests passed,
  including the new visible-scope regression.
- Real Ghostty checks passed for local shell attachment and for a launch command
  exiting into a usable shell whose terminal marker survives.
- Empty-registration deletion proof passed again after adding the transactional
  enabled-state check. `cargo clippy --all-targets -- -D warnings` passed.
- Captured and inspected `ux-reference-images/integrated-workspace.png` from the
  compiled native app in fixture mode. It shows the unified toolbar with both
  actions and visible launch scope. Its fixture ELSEWHERE row has no local
  terminal and is expected; it is not evidence about the user's live clients.
- Review: shared CLI remains the only current-Wave filter; window surfaces have
  one owner; outer close retains the inner store; replaying a closed launch pane
  cannot rerun the provider; hidden terminals cannot claim focus; returning to
  terminals remains available even when a checkout group is hidden.
- Post-integration Xcode `build-for-testing` succeeded for the app and test
  targets. The installed app was not replaced. All commits remain local.
