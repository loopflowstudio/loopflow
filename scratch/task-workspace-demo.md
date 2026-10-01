# Task workspace demo — 2026-09-30

LOO-353 Unit 1, [PR #1369](https://github.com/loopflowstudio/loopflow/pull/1369),
reviewed with Jack Heart from `growth-thoughts` at
`4b15f9591ce9062c16b115b3332c83101ab7e91e`.
Design: [growth-thoughts.md](growth-thoughts.md).
Prior local proof: [kickoff-evidence.md](kickoff-evidence.md).
Ask contract: [ask-evidence.md](ask-evidence.md).

Status, 2026-10-01: main is integrated through `5eaf887fe` in local merge
`d6db71c9f`. A compact Sessions pane, retained pane visibility toggle, toolbar
creation/Files controls and compact current-stage action are implemented for review.
The terminal presentation is unchanged. The rebuilt app initially failed against
the September demo copy: main now requires an exact development migration history,
and that copy has no receipts for the seven current drafts. Jack Heart supplied
screenshots of the same error in both the workspace and Open Task dialog. Retry
cannot resolve it. The first reopening was premature; compilation did not prove
runtime compatibility. No Unit 1 walkthrough or latency target has passed.

## October 1 Project repair and concept review

Jack Heart requested repairing Growth and Product's current Projects, then making
Projects independent of optional chapter coordination. Jack also accepted a Task
admission/completion audit and requested the broader research in this conversation.
The accepted direction is in [the design](growth-thoughts.md#optional-coordination-above-independent-operations--october-1-review);
[source findings, primary research and twelve ranked proposals](independent-operations-research.md)
separate accepted constraints from possible follow-ups.

Live provider inspection found one unarchived Project per Wave, both Backlog.
Authorized status changes returned success for Growth
`f95ba11f-ab26-4780-9b6b-8fb99a691e48` and Product
`5420affb-2d31-411a-b8d4-5516a7f49b8e`, both now In Progress. Existing plans and
Product's two KRs were retained; the existing `feature` fallback was recorded
explicitly because the source reader requires a nonempty Flow. No chapter was
rotated, Task moved or history retired. These are current-data repairs; optional
chapters remain implementation work.

Configured Product refresh then failed on a retained historical Project owned by
another Team. Legacy adoption preceded the normal Team filter. The local repair
reuses that filter before adoption and candidate selection, preserving foreign
Projects and their migration receipts. Two focused `legacy_adoption_` tests passed,
including listed and archived foreign Projects and ambiguity handling. `cargo fmt`,
`cargo clippy --all-targets -- -D warnings`, and whitespace checks passed.
Review finding: filtering only the adoption loop would still let a foreign Started
Project suppress legitimate current selection; filtering the candidate inventory
as well fixes that dependency.

The materialized demo CLI was rebuilt with this repair and installed into the
signed Dev app. Explicit `repo refresh product` succeeded against the copied Home;
subsequent `wave status product --json` returns Projects and Tasks with state `ok`,
the same current Project with status `started`, Flow `feature`, and both KRs.
Growth's configured reading likewise returns its same Started Project and Tasks.
Evidence: `.lf/tmp/sync-oct1/{growth-status-repair,product-status-repair,
growth-demo-verified,product-adoption-verified}.json`,
`product-adoption-refresh.log`, `adoption-tests.log`, `adoption-clippy.log`.
The live main database was not upgraded. The Dev app was reopened on the accepted
copied Home; native capture `/tmp/loopflow-20261001-151634.png` showed loading.
The next capture returned `No window to snapshot (8)`. Window closure versus app
exit was not established. Native rendering remains unverified for this repair;
the successful CLI readings are not substituted for that proof.

Filed [LOO-366](https://linear.app/loopflow/issue/LOO-366) for Project availability
and optional chapters, and [LOO-367](https://linear.app/loopflow/issue/LOO-367) for
Task admission/completion. Neither was launched. The deeper research was completed
locally instead of filing a duplicate research Task. Remaining long-tail features
are proposals. Next implementation should prove repeated/concurrent Project
activation and no-chapter use, then the planning-only and managed delivery Task
paths. Full Unit 1 provider continuation and measured layout performance remain
unproved; this repair does not close those acceptance gaps.

## October 1 retry

Jack requested integration with main and another demonstration. Loopflow selected
`merge_target` for the protected published branch; the operation used `lf sync`
and retained branch history. No push or PR mutation occurred.

The candidate removes the left-side button stack and description, places creation
in a + menu and Files in one toolbar control, and puts Task details behind the
Task title. The Sessions pane can hide and restore through a retained workspace
preference. The full Flow remains in Task details; its current stage is a compact
toolbar action. These are implemented proposals for Jack's next visual review,
not inferred acceptance of the exact arrangement.

Local proof after reconciliation: 17 Rust DTO fixtures, 33 Session tests, 67 Swift
workspace/navigation/file tests and 11 focused destination tests passed. The latter
exercises hiding/restoring the pane with retained shell layout and editor draft.
Cargo formatting/Clippy passed before the integration checkpoint. Both SwiftPM
and Xcode fallback build-for-testing compiled the candidate. Logs:
`.lf/tmp/sync-oct1/`. These checks do not prove a configured demonstration.

The old automatic source-Home seed mechanism was removed upstream. The ordinary
CLI intentionally validates an existing private Home without upgrading its draft
schema. The retry uses a new SQLite backup of the main Home and the documented
migration-test materialization path in a disposable source copy, without Git
worktree creation or changing the assigned checkout's migration catalog. The
copied database passed the production migration runner and persisted-JSON checks;
Home identity, scoped LOO-330 roadmap, Session inventory and its 40-entry directory
listing all exited 0. Copied Session driver ownership and provider endpoints were
cleared before launching, preserving history without attaching to live processes. This is a test-materialized copied-data demo, not a published
release or main-Home operation. Its exact paths are recorded in
`.lf/tmp/sync-oct1/demo-copy.json`. The earlier September copy remains preserved.

The first successful workspace capture exposed another counterexample: `roadmap
--all` rejected two `ear` snapshots belonging to different repositories, hiding
all planning. The reader now maintains ownership checks per canonical repository;
a focused Rust test proves separate repositories remain readable and duplicate
ownership inside one repository still fails. The full copied roadmap then exited
0. No snapshot was deleted to suppress the error.

The fresh main-Home copy contains no open Session for LOO-330. Its retained Task
and checkout remain available for visual and Files review; the empty Sessions
list is not proof of a grouping defect or a successful provider continuation.
The previous September screenshot had a conversation that is absent from this
fresh inventory. Use an available current conversation for the provider portion
of the subsequent walkthrough rather than inventing one from historical evidence.

### Reopened candidate — October 1, 13:42

Native capture `.lf/tmp/sync-oct1/reopened-workspace.png` shows LOO-330's retained
workspace, compact toolbar, empty Sessions pane, unchanged terminal surface and
restored repository planning navigation. The incompatible-database dialog and
global duplicate-ownership error are absent. Product still reports no In Progress
Project; this retained planning gap was not repaired or hidden by the demo.
The exact Task remains accessible. Jack has not yet reviewed the revised controls.

Final Cargo Clippy passed; `git diff --check` passed. Local implementation checkpoint:
`9d5af02fc`. The focused ownership test passed with exit 0; its log also contains an
earlier compilation failure because two sequential attempts overlapped their log
file. The final pass is the `1 passed` result, not the earlier type mismatch.

Next action: Jack reviews the revised workspace and uses the toolbar's Sessions
visibility toggle, + menu and Files control. Continue the provider and retention
walkthrough on a current placed Task with a live conversation. Full Ask retry,
symlink/draft, cross-Task focus and 20-action latency/CPU proof remain unexecuted;
this reopening is not Unit 1 acceptance. The app uses the test-materialized CLI
from the disposable source copy and a fresh migrated copy of Jack's Home. Ordinary
`loopflow-dev.py run` rebuilds the authoring CLI, so reusing this demo database
requires the matching materialized helper recorded in `demo-copy.json`.

### Wave names — October 1 follow-up

Jack Heart requested readable `wave.displayName` labels, with `a-b` displayed as
“A B” and the same treatment for snake case. `WaveViewModel.displayName` now
preserves any name containing an uppercase letter exactly, per Jack's follow-up.
For other names it replaces underscores and hyphens with spaces and capitalizes
the words. Registry names and identity are unchanged. Existing Wave, sidebar-row and portfolio
expectations use the display labels. The running demo has not been relaunched
for this follow-up.

## Visual review — September 30, 18:32

Jack supplied a screenshot of LOO-330 and said “extremely ugly.” When the agent
proposed broader layout changes, Jack clarified: “the terminal part is fine its
mostly the buttons on the left of it.” This supersedes the agent's initial
proposal to change the terminal background or center the visual revision on it.
Preserve the terminal presentation. Jack then requested the ability to collapse
the left pane and questioned whether the Flow module needs to stay at the top.

Observed in the screenshot: Task details, New conversation, New shell and Files
form a large vertical stack beside the terminal. A long Task description sits
below that stack, while the conversation title above it wraps and truncates.
Files is also exposed in the top toolbar.

Proposed revision, not yet accepted in detail: retain a compact conversation list;
replace the creation-button stack with one + menu containing New conversation and
New shell; keep one Files toggle in the toolbar; put Task details and the directive
behind the Task title. Keep Expand/Collapse and Focus associated with their pane
rows, using compact controls. Preserve retained surfaces, documents and direct
Session completion. Add a retained collapse preference for this pane and a small
restore control in the Task toolbar; collapse must preserve pane processes,
drafts, selection and layout. Jack explicitly requested this collapse capability.

For the Flow, the agent recommends moving the full module into Task details and
keeping a compact current-stage/status action near the Task title, linked to the
exact interactive Session when available. Jack questioned the persistent module;
Jack has not yet chosen this specific replacement. Preserve existing completion
and recovery controls and detailed Flow access. See the
[design](growth-thoughts.md#left-panel-visual-revision).

Recommended next action: implement the left-panel revision and review the real
LOO-330 workspace with Jack, with one conversation, multiple panes and Files open.
Verify readable names, unclipped actions and retained pane behavior at the current
window size and a narrower size. Jack's clarification defines the area to change;
the proposed exact controls still need visual review. Functional and latency proof
remain open. The earlier working-window evidence establishes rendering, not visual
acceptance.

## Jack's direction

Jack requested the Desktop app on the main Home and real Tasks, covering checkout
association, files without PR/Project metadata, retained drafts when symlinks make
files read-only, pane collapse/focus/restore, participation, Ask retries and latency.
Jack initially said any Task was fine, then suggested LOO-298 or growth-thoughts.
The demo selected LOO-298 for its placed Task workspace; this conversation remains
in the supplied growth-thoughts checkout.

Jack later confirmed LOO-298 had landed. The copied reading agrees: its runtime
is done. The walkthrough now targets LOO-330, which has an existing checkout and
pending design review. Jack authorized stopping the concurrent merge-recovery
run and finishing in this demo Session. Its `lf` process and the separate waiting
Ask were interrupted through their normal signal handlers; their provider and
parent processes exited. The Ask was not completed and recovery was not resumed.

## Executed observations

- The working tree was clean. PR #1369 is open and its head matches this checkout.
- `lf home id --json` identifies the main Home as
  `home_39860354aaca640c2ccb50bf6ca609d8`; the selected published installation uses
  `/Users/jack/.lf/loopflow.db`.
- `lf task status LOO-353 --json` reports no Task. The subsequent Product reading
  does include LOO-353 in its chapter, with no runtime or checkout. This is a
  planning/runtime distinction, not evidence that Jack's Task was deleted.
- `lf task status LOO-298 --json` resolves
  `task_9756ded2ad49449090003636171df943`, checkout
  `/Users/jack/src/loopflow.data-model-one-table-per`, with an active local PR.
  It cannot supply the no-PR case by itself.
- `uv run python scripts/loopflow-dev.py run` succeeded: Swift built, the branch
  CLI compiled with release Home selection and validation-only migration
  authority, the app was signed and launched at
  `/Users/jack/Applications/Loopflow Dev.app`. Observed PID: 66039. The installed
  app at `/Applications/Loopflow.app` remained running separately.
- The development app's `LoopflowDevControl.json` selects
  `/Users/jack/.lf-machine/install/gates/1/lf`. `controlLfPath` uses that selection
  for queries ahead of the bundled helper. That installed CLI lacks `task files`
  and `ask --key`; its Session JSON has no `workspace` field.
- The bundled branch CLI has `task files`, but its `home id --json` fails:
  `store /Users/jack/.lf/loopflow.db belongs to another installation; use its
  installed lf or this build's branch data directory`.
  Changing only the app's helper pointer therefore cannot establish the requested
  main-Home path.
- `lf install promote --help` describes `--from-build` as promotion into a
  disposable installed Home. Source inspection confirms a new development store
  under `.lf-dev/installed/<installation-id>/loopflow.db`. No promotion was run;
  that path is not the requested main-Home demonstration.
- Opening `loopflow://task/LOO-298` explicitly with Loopflow Dev returned success
  from LaunchServices. This proves delivery of the open request, not successful
  Task rendering. Jack subsequently requested reopening; the same command again
  returned success. The conversational claim "Reopened Loopflow Dev on LOO-298"
  overstated the result: only app opening was established.
- The app's AppleScript screenshot command failed with handler error -1717.
  System Events window inspection failed with assistive-access error -1728.
  No agent-captured screenshot or automated native interaction is claimed.

## Jack's screenshot — 17:40 local time

Jack supplied a screenshot of Loopflow Dev showing **Planning unavailable**,
**Roadmap unavailable**, and **Work unavailable** instead of the Task workspace.
The visible decoding failure is `keyNotFound` for `interactions` at
`waves[6].tasks.items[6].flow.record.graph`. The sidebar still displays
**ORPHAN SESSIONS · 26** and Session rows. Their presentation does not prove those
Sessions lack Task placement; the planning read has failed and the older CLI
does not supply the new workspace association.

This screenshot confirms the user-visible consequence of the previously observed
CLI mismatch. Refreshing or repeating the deep link has not demonstrated recovery.
Jack supplied evidence of the failure, not approval or a request to change the
workspace design. Retained local image:
`.lf/tmp/task-workspace-demo/planning-unavailable.png`.

## Measurement boundary

The target remains p95 below 100 ms across at least 20 retained-surface actions,
with zero CLI/Git/network calls caused by those layout actions. Also retain the
separate hierarchy/workspace signals and before/after idle CPU and process counts
with several Tasks open.

The current `retained_workspace_action` endpoint is the next main callback, not
visible/usable completion. The fixture performance runner uses synthetic DTOs and
cannot establish configured Home/provider behavior. A short PID-scoped recording
of this launch is diagnostic only; no layout sample or multi-Task comparison is
claimed. Raw local receipts are under `.lf/tmp/task-workspace-demo/`.

Executed `record_live.py record --pid 66039 --seconds 30 --no-xctrace` at
14:58:06 local time on macOS 26.0.1 / Apple M4 Max. The report contains zero
retained-workspace, hierarchy or workspace-ready intervals. It records 23
`session list` reads (p50 873.51 ms; p95 949.47 ms), plus background activity,
process, roadmap and Wave reads. Those subprocess durations do not measure layout
latency. RSS was 127.2–127.8 MiB (0.5 MiB start-to-end growth). Separate process
snapshots counted five then one app/descendant processes, with app CPU readings
of 0.7% then 0.0%. These are point samples during polling, not an averaged idle
CPU comparison with several retained Tasks. Hitches/presentation were unmeasured.
Receipt: `.lf/tmp/task-workspace-demo/launch-recording/report.md`.

## Remaining walkthrough

| Experience | Required configured proof | This review |
|---|---|---|
| Checkout grouping | Independent Sessions from root, subdirectory and symlink alias group under one Task; sibling/nested checkouts remain distinct | Not exercised |
| Files without planning/PR | Browse, save/read back and refresh on a retained checkout with no active PR or Project metadata | Not exercised; do not delete live planning to manufacture the case |
| Symlink access change | Preserve a dirty draft when an open file or its parent becomes an internal symlink; disable editing, manual Save and autosave | Not exercised |
| Retained panes | Collapse/expand, focus/restore and Task return preserve shell processes, input, file draft, split ratios and selection | Not exercised |
| Participation | A second Task's Ask/Flow indication appears without focus theft; selecting it reaches the exact boundary | Not exercised |
| Ask retries | Same caller/key joins one question and returns retained feedback; only its caller resumes after Jack completes | Not exercised |
| Remote ownership | Owning Home supplies association without local canonicalization of remote paths | Not exercised |
| Latency/resource target | At least 20 configured actions with visible/usable endpoint and idle resource comparison | Not measured |

## Recommended next action

Revise and review the left pane before resuming the remaining walkthrough and
layout measurements. Jack's screenshot at 17:57 showed
the Open Task sheet remaining at “Finding Task…”, after the planning repair.
This is a counterexample to treating healthy planning as a working Task demo.
The exact CLI lookup succeeds, but an unscoped lookup also reports unavailable
planning from unrelated repositories and therefore cannot automatically select
its one match. A scoped lookup for LOO-298 still includes an unavailable engbot
Wave alongside the infrastructure match. Those observations do not by themselves
explain a sheet that remains loading. Jack requested reopening instead of reporting
a result from direct ⌘K navigation. The repaired repo-scoped link now opens its
one exact match even with another Wave unavailable. Unscoped incomplete or
ambiguous readings retain their chooser.

At 18:13, an agent-captured screenshot of the running app shows LOO-330 selected,
its checkout in the header, Show Files, Conversations, Collapse, Focus, New shell,
Files, and the task-design/review-design participation strip. This supersedes the
navigation blocker as the current observation. Image:
`.lf/tmp/task-workspace-demo/loo330-workspace.png`. Jack's subsequent visual
rejection and clarification are recorded above. Its terminal shows login output; that is not
proof of a resumed provider conversation. No layout-action or save acceptance is
inferred from these visible controls.

## Repair and local proof

The branch now incorporates main through `de074a2eb`, with adopted merge
`5355bba15`. Unit 1 association, participation and Ask keys were reconciled with
durable AgentSessions and numeric Flow occurrence keys. Ask caller links now name
the stable caller Session; retry keys remain scoped to the caller's captured input.
Shared Rust/Swift fixtures move together; absent interaction graphs are not defaulted.

The development installer bundles this checkout's CLI with development provenance
and removes its stale pointer to the published machine CLI. Existing source-build
isolation copies the selected installed store into
`/Users/jack/.lf-dev/worktrees/loopflow-growth-thoughts-1c80b40d4504/loopflow.db`.
The copy retains Home/Task identity and checkout paths, but not inherited execution
authority. This is Jack's accepted copied-Home demonstration, not proof of operating
the live main Home. Real checkout files remain real files; no planning was deleted
and no live provider continuation was attempted.

Configured observations using the installed branch helper:

- Roadmap JSON includes `graph.interactions`; LOO-298's file listing returns 41 entries.
- Six migrated Sessions had no explicit repository and were omitted by the repository
  filter. The reader now uses their recorded Wave repository when the Session has no
  repository; an explicit Session repository remains authoritative. All six appear
  in the repository inventory and all six carry Task workspace association.
- Native captures `repaired-planning.png` and `task330-copied-home.png` under
  `.lf/tmp/task-workspace-demo/` show planning and participation indicators without
  the original JSON error. They do not show the selected Task workspace.
- A 15-second capture was still loading; a 45-second capture with explicit copied
  Home paths rendered planning. These are observations, not startup latency passes.
- The AppleScript screenshot definition named `Loopflow.CaptureScreenshotCommand`,
  but both native build configurations use module `LoopflowMac`. Correcting that
  reference restores the app's existing screenshot command; the final screenshot
  above came from the running window, without accessibility automation.
- Retained Task lookup evidence now supplies the workspace's checkout during
  planning loading or when the selected historical Task is outside the current
  plan. Previously the breadcrumb could name the Task while its panes used the
  generic repository workspace until the plan arrived.

Executed local proof: 10 Rust Flow graph tests, 27 Session/workspace tests, the
focused raw Ask retry test, the retained-Session repository test, 14 shared DTO
tests, and 84 focused Swift tests passed. Swift transport/provider fixtures remain
simulated. The native fallback Xcode build-for-testing passed. Formatting, Clippy
with all targets and warnings denied, and whitespace checks passed. A duplicate
raw-key test introduced by concurrent reconciliation was removed; the retained
behavioral proof and Clippy passed again. All 10 WorkspaceDestinationTests also
passed, covering exact, historical, unavailable and overlapping Task links with
simulated transport. The scoped-link repair adds an eleventh test, including
both scoped and unscoped partial-planning behavior. The configured LOO-330 file
query succeeds as well. The final 11-test run also verifies retained checkout
controls after the Task disappears from current planning, and the final Xcode
fallback build-for-testing passed. The app was rebuilt and reopened with those
changes; a live 18:15 capture confirms Task files/checkout controls are available
while broader planning is still loading.

Review findings addressed: the UI must use its matching CLI, migrated Sessions must
remain discoverable without inventing repository identity, numeric occurrence keys
must stay consistent through graph projection, and a copied Home must not inherit
live execution authority. The native workspace now renders; the remaining
walkthrough and measurement gaps stay explicit above.
No PR mutation or push was performed by this demo. This independent Session has no
Flow completion/navigation verdict.
