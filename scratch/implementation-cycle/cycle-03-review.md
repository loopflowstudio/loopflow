# Cycle 3 — review-slice: frame, Wave plan, basic Task overview

2026-09-25. Disposition: **the slice advances the full design after two bounded
fixes and one added assertion.** It is a strict subset: Flow catalogue, pinned
two-loop diagram with queue → land, pause/restart controls and comments are the
next slice. No commit, publication, PM/live-store write, installation, Session
operation or Task completion.

## Claim matrix

| Claim | Implemented | Proof | Result |
|---|---|---|---|
| One connected repo header + outline, repo switch in place, bottom search | `WorkspaceNavigator.header` menu (repo list + actions), search pinned below ScrollView; repo rows deleted | Mounted native captures `/tmp/loo291-c3-parent-captures/*`; `WorkspaceNavigationTests` repository-root assertion | pass (fixture reads) |
| Sidebar = started working set, stable through process exit | `WorkspaceTask.inWorkingSet` = open Sessions ∨ (runtime.started ∧ incomplete); live-process input (`activeWorktrees`) deleted | `startedWorkingSet`, `denseWorkingSet` (3×50, 20 started) | pass |
| Started is shared durable evidence, not inspection/checkout/prepared Run | `Store::task_started`: Started/progress/body_handed_off/flow_finished event or `worker_generation>0`; roadmap adds published/merged PR | Real CLI `observing_and_preparing_a_task_are_not_execution` (isolated Home, stand-in provider): false after prepare, filtered read, unopened review; true after independent Run and after its exit | pass, rerun here |
| No periodic history scan / second writer | One `EXISTS` over `idx_task_events_task` + flow position per Task per roadmap read; Started written only at Run capture (`started-read-boundary.md`) | Source; `rg task_started` → one reader, no writer | pass |
| Inline named Session count | Count button inspects overview; help/a11y + context menu name each Session | Native drill-down proof | pass |
| One Session → exact native conversation; zero/several → overview | `SessionsView.openTask` | `TaskMonitorProofTests` (one), `namedSessionDrillDownRetainsTerminal` (two) | pass |
| Task ancestor always goes upward, even with one Session | Crumb calls `model.select` directly, never `openTask` | **Added** assertion to single-Session test: content = details, no selected Session, same pane | pass (new) |
| Wave: objective → Current KRs → full ranked plan | `waveDetail`; explicit "No current chapter plan."; partial/unavailable warnings retained | Fixture captures `wave-*.png`; WorkspaceNavigation tests | pass |
| Task: title once, Wave / issue (Linear link), New session beside title | Breadcrumb shows Task crumb only inside a Session | `breadcrumb-issue` link assertion; capture | pass |
| Description: real block Markdown, source unchanged | Foundation `AttributedString(markdown:)` presentation intents → headings, nested/ordered lists, quote, code, table rows, rule, links; verbatim fallback; editor saves source | `descriptionMarkdownBlocks` + editor tests | pass |
| New session: captured Task, shared prepare, configured destination, no managed Flow | Existing checkout or `lf task prepare <ID> --json` → `lf --interactive --task <ID> : …` in worktree shell | Receipt parser/argv test; prepare semantics from `flow_tests` (prepare is not start) | pass except stale target, **fixed** below |
| Noise removed | No ELSEWHERE/VIEWING badges, snapshot line, chips, worktree path, Open/Inspect menu in Podium path | `rg` negative search (remaining "Open" only in legacy RoadmapView/KR a11y value) | pass |
| Native continuity | Same surfaces, first responder, draft echo, companions through overview/Session/rename/ancestor | 4 Ghostty/PTY tests in selection | pass (fixture reads, owned `/bin/cat`) |
| Rust/Swift DTO parity | `started` required in both, no defaults; three fixtures updated | 8 Rust `dto_fixtures`, Swift `DTOFixtureTests` | pass |

## Findings and fixes

1. **Fixed — New session could redirect another repository.** After the
   `lf task prepare` await, `launch` set `navigation.content = .terminals` on
   `model.navigation`, which is the *current* repository's navigation. Switching
   repositories during preparation would flip the new repository to terminals
   while the shell landed in the original repository's retained layout.
   `launch(_:in:)` now takes the originating repository's navigation, captured
   before the await. Source-level fix; no native test injects a slow prepare
   (the launcher is a static subprocess call and adding a seam only for tests is
   forbidden). Recorded as a proof gap.
2. **Fixed — location vanished while a Session was open.** In compact/full the
   Session is inline on its Task row, so `isSelected` highlighted nothing once a
   Session was selected. The Task row carrying the selected Session now stays
   highlighted.
3. **Added proof** that the Task ancestor goes upward from a one-Session Task
   (the two-Session path was already covered).
4. Not changed: Task overview omits a Sessions line when the Session read fails;
   the view-level "Sessions unavailable — …" banner already states it, and
   "No open Sessions." requires a successful read. `started` is a store-read
   failure propagated as a roadmap error (explicit, never silently false).

## Receipts

- `swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel --filter 'WorkspaceNavigationTests|WorkspaceNavigationProofTests/namedSessionDrillDownRetainsTerminal|WorkspaceNavigationProofTests/workspaceRetainsNativeSplit|TaskDirectiveEditorTests|LocalWaveAgentLauncherTests|DTOFixtureTests|TaskMonitorProofTests'`
  → **52 tests / 6 suites pass** (`/tmp/loo291-c3-review-swift.log`), after both fixes.
- Same tool, `TaskMonitorProofTests|…namedSessionDrillDownRetainsTerminal` → **4 pass**
  with the new ancestor assertion (`/tmp/loo291-c3-review-ancestor.log`).
- `cargo test -p loopflow --test flow_tests observing_and_preparing_a_task_are_not_execution`
  (ambient LF_* cleared) → 1 pass; `--test dto_fixtures` → 8 pass
  (`/tmp/loo291-c3-review-rust.log`). No Rust edits in this review, so no fmt/clippy rerun.
- `git diff --check` passes. Accepted prototype: all 16 hashes in
  `scratch/visual-study/accepted-reference.json` match (untouched).
- Native captures (fixture reads, real PTYs, hidden window):
  `/tmp/loo291-c3-parent-captures/{Full hierarchy,Compact,Sessions,wave}-{false,true}.png`,
  compared with `/tmp/loo291-accepted-{frame,task}-reference.png`. Hierarchy,
  cream sidebar, bottom search, title-once and New session match; the
  reference's Flow region and Comments are absent by design of this slice.
  Captures predate fix 2 (no Session was selected in them).

Final SHA-256: SessionsView `12157104910ccabac44da817e5361f87215b1113e89d99f3e8e4f9294c342b88`,
WorkspaceNavigator `eec92ee5218061f2fe00477cb8f969266e3b6b1857fb58ad1049953e38749e39`,
TaskMonitorProofTests `5e8f80de7f5a49ece99ad565b7b1831e780cdf7f11adca441ea752deaa5509bb`,
WorkSurfaceView `d9c7fc62818fd0ddb7d1a70094bac7bbd2e6436a35d196db5f1751cc0f1f9cc9`;
MarkdownBlocks, WorkspaceProjection, sqlite/chapters.rs and waves.rs unchanged
from the compress/implement receipts.

## Review limits

Read the complete working-tree diff for Rust, fixtures and all changed Swift
production files; test files only where claims depend on them. The historical
Task diff (hundreds of files) was not re-reviewed; prior cycle receipts cover
it. No configured app, vendor provider, live Linear read or dev-Home access (its
database guard was not bypassed). Real `lf task prepare` from the app and the
repo-switch race in finding 1 are unexercised.

## Remaining full-design gaps

- Next slice: real Flow catalogue search/preview; pinned occurrence diagram with
  both return edges and final Advance → queue → land; running/paused/blocked
  situations; Stop & restart / pause controls; actual comment count/thread.
- Minor presentation deltas vs reference: Wave colour dots/started counts in the
  sidebar, underlined issue link; revisit in the post-build UI discussion.
- Historical starts predating recorded evidence appear only with open Sessions.
- Configured demo, both performance measures, external trials and authorized edit.
