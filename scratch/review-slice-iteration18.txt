# Iteration 18 — native outline and required Session Runs

2026-09-24. The slice advances the accepted design. Fixed disappearing empty
planning subjects and restored inspection evidence lost with the old toolbar.
Configured read-only navigation passes. The complete canvas is not ready for
publication: Monitor, exact active Task Runs, repeatable measurements and retained
human/external acceptance remain open.

Read the current slice in `main-view-task.md`, the canvas launch decision,
required Session/Run design, wave memory and current source. The full tracked
diff against `e0849bad498ff8dfb51b344eba3f7520a44d906e` is captured at
`/tmp/loo291-review18-final-tracked.patch`; this review's untracked evidence is
[here](configured-ui-evidence/iteration18-review/receipt.json). Earlier unchanged
code retains its earlier review and proof scope, not a new full-suite verdict.
The latest marked contract slice prepares Runs before publishing Sessions; the
broader native outline/Monitor/measurement launch decision remains the target.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| One compressible outline | Preserve identities, upcoming Tasks, missing ancestry and non-Task Sessions across presentations | One typed projection and retained navigation owner; empty planning leaves now survive Compact | Focused navigation suite; failing then passing empty Wave/Project cases | pass locally |
| Configured navigation | Inspect real planning without launching a provider | LOO-291 selected by exact planning ID; directive and snapshot evidence available; Full retains selection | Final `outline.log`, current disposable app and existing development Home | pass, read-only path |
| Session leaves and retained terminals | Select the original Session and preserve companion/input/layout | Existing workspace registry, multiplexer and window-local surface pool remain | Native `sessionRowRestoresWorktree`, real PTYs with fixture Session records | pass locally; no configured provider continuation claim |
| Inspection evidence | Distinguish no Session from unavailable reading; disclose planning age | Details show snapshot generation and explicit absence only on successful Session reads | Two-case inspection test; configured snapshot text observed | pass, bounded |
| Required Run reference | All three Session kinds carry explicit required `run_id` | Rust/Swift and shared fixtures agree; boundary ID remains action target | Prior required-field fixtures; fresh CLI prelaunch lookup | pass, contract/local CLI |
| Prelaunch and launch identity | Unopened Session resolves to its Run; child consumes it; resume retains it | Prepared manifest is published before boundary; existing capture owns launch | Added real `lf runs <run_id> --json` assertion; concurrent first-launch/resume proof | pass with local stand-in provider |
| Open during resume | Metadata must not wait for the conversation to end | Concurrent correction releases preparation lock before waiting on resumed provider | Reviewed source and passing concurrent regression | pass locally; credited separately |
| One authority | No second inventory, mapping, policy or terminal owner | Podium reads, shared action descriptors, boundary references and Run capture remain the owners | Negative searches and launch/projection/pane source trace | pass, source |
| Complete Task workspace | Task Monitor and Sessions share the existing multiplexer; exact live Run evidence | Task selection still opens details; no Monitor pane or shared exact live-Run projection | PaneContent, SessionsView, Podium and Activity/Run contracts | gap |
| Performance and complete acceptance | Two repeatable usable/rendered measurements, budgets, external trials and human confirmation | No new runner, baseline, budget or acceptance series | Canvas and Task obligations compared with current code | gap |

## Corrections made in this review

1. Compact previously returned zero rows for a readable Wave with no Projects,
   or its sole Project with no Tasks. The view then said “No matching Work.”
   Compression now promotes children only when children exist. The empty leaf
   retains inspection, contextual conversation and ancestor access. Session-only
   mode still correctly has no fabricated leaves. `empty-before.log` records both
   failures; the navigation suite passes after correction.
2. Removing the toolbar also removed the explicit no-Session state and planning
   timestamp, while README still promised them. Restored these facts inside Work
   inspection. The timestamp explicitly means snapshot generation, not provider
   freshness; unavailable Session reads never display an empty claim. Reuses the
   projection's existing reverse identity join and computes it once per inspection.
3. Extended the isolated CLI proof to look up the unopened Session through the
   public `runs` command, checking its exact ID, parent and absent terminal outcome.
   This exercises the user lookup, beyond reading a manifest file directly.
4. Clarified the legacy-boundary documentation: listing fails as a whole until
   explicit JSON open prepares the missing Run. It does not return a partial
   unavailable row. No live boundary was migrated to manufacture proof.

The concurrent [Session recovery contribution](session-run-review.md) fixes a
separate real defect: native resume held the preparation lock throughout a
provider conversation, blocking another JSON open. Reviewed its unlock boundary,
first-launch/resume regression and passing receipts. Preserved that writer's
changes; they are not this review's implementation credit.

## Configured demonstration and limits

The final probe uses the existing development Home
`/Users/jack/.lf-dev/installed/local-be852452823d43d7b7fde663651a7590`, current
checkout CLI and freshly copied SwiftPM executable in a disposable app. No
installed app was replaced. The owned app was terminated after inspection;
no Session was opened, transferred, resolved or completed.

The first ambient CLI read returned 11 Session references, but its Home selection
was not aligned with the app's store. Retained as a read receipt only, with no
cross-surface identity credit. An initial probe mistakenly pinned production
`.lf` and was rejected by the development database guard before app launch.
The corrected probe explicitly selects the existing development Home. Initial
UI lookup used identifiers that SwiftUI did not expose on these controls; AX roles
and the existing “Outline presentation” label locate the actual controls. This
was a probe selector correction, not missing Accessibility permission.

Final `outline.log` records exact Task selection, directive availability, restored
snapshot evidence, all three presentation choices, switching to Full while
retaining selection, and unchanged configured Session IDs. This is real app/CLI
interaction, not a fixture screenshot, configured provider continuation, human
approval or a rendering-time measurement. The other writer's zero-Session
population receipt uses a different selected Home and is not contradictory proof.

## Verification

- Swift navigation/real-PTY proof: 20 tests passed in two suites. After the final
  inspection-only cleanup, its two argument cases passed again (one test).
- Isolated prelaunch CLI lookup: one test passed. Concurrent first-launch/resume
  regression passes both paths with an owned stand-in provider. No user process
  was involved.
- Inspected the concurrent passing all-target Clippy receipt; no Rust production
  edit followed it. Formatting and `git diff --check` pass.
- Negative searches retain one production Podium caller per shared inventory and
  one root workspace registry. SessionScope, FlowResolutionAction,
  requestedSessionId and the former temporary post-launch Run-binding writer
  remain absent. Required Run identity does not itself establish process liveness.
- The earlier fallback-build attempt stopped at resource preflight. No successful
  Xcode fallback compile or hosted UI-suite run is newly established here.

## Next implementation boundary

Keep this checkout and the same owners. Extend existing pane content with a
Task-bound Monitor, retain each Task's pane choice, and show Monitor, Session and
a companion shell through the current split/resize/zoom controls. Supply exact
Task-to-live-Run evidence from one Rust reader: deduplicate Run ownership, retain
old active Runs beyond ordinary history limits, distinguish live waiting clients
from dead/unknown evidence, and never use checkout coincidence or prepared
manifests as liveness proof. Mirror any wire fields and fixtures together.

Implement `hierarchy_interaction_ms` and `task_workspace_ready_ms` with the fixed
populations, retained PTYs, usable/rendered endpoints, hitches and per-attempt
outcomes defined in the canvas decision. Collect baselines before targeted
optimization and choose budgets from them. Cross-repository flat coverage,
fallback compilation, configured combined-pane input and the human composition
demo remain required. Preserve the ten human-selected external trials, authorized
directive edit and twenty long-lived-registry trials; benchmark fixtures do not
replace them. Do not create the deferred optimization Tasks yet.

No publication, landing or Task completion. The
[review-slice skill](/Users/jack/.agents/skills/review-slice/SKILL.md) publishes
“When all applicable `Done when` claims hold and the slice is coherent”; the
remaining core and acceptance claims above do not hold yet.
