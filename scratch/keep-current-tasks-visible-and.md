# Keep current Tasks visible and completed history out of the way

2026-10-02 · LOO-369 · Jack Heart approved the inline prototype and requested Desktop implementation. Caller owns queue and delivery.
Jack Heart requested getting LOO-369 running and ready to demo, as relayed by
his existing control conversation. Coordinator guidance limits Show completed
to successful completions; this refinement is not attributed as a direct new
statement from Jack. Routine filter details below are implementation choices.
The shared projection and Desktop filter passed the earlier affected gate.
The approved inline revision is implemented locally; caller owns its queue gate
and the final Desktop review.

## Accepted inline control · 2026-10-02

Jack Heart approved `task-history-inline-prototype.html` in the live conversation
and requested implementing it in Desktop. This supersedes the earlier separate buttons,
range menu, and positive-only input proposals. One compact checkbox and label
shows Completed when off, N Days when bounded, and All Tasks when days is zero.
Click the off label to enable the remembered range (initially 7); click the active
label to edit and select the number in place. Enter/blur applies, Escape cancels.
Empty, negative, fractional and overflowing drafts retain the applied range.
The checkbox hides history and remembers the range. No permanent border or fill.
The checkbox, number glyphs and Days baseline must not move on entering editing.

The separate Show completed/All time toggles, redundant `allTime`, persisted
`daysText`, and positive-only validation are removed. One applied day count
represents the range; zero means unbounded history. Native transient editing
owns drafts. Existing bindings, lifecycle predicates, dates, counts and Session
access are preserved.

## Problem and demo

Jack sees seven canceled duplicates among 13 Tasks on Growth. Current work must
be easy to scan, with completed Tasks hidden initially and history available
through Completed: initially 7 Days, another positive day count, or zero/All Tasks.

Jack Heart's October 2 Growth screenshot shows six current Tasks and no canceled
duplicates. The prepared offline capture retains all thirteen provider items,
including the seven cancellations and four requested retained targets. It has no
successful completions or Sessions. The matching packaged CLI projects those
facts correctly; the older installed CLI is not a prerequisite for this isolated
demo. [Demo evidence and review path](../docs/reviews/task-history.md) distinguish
snapshot refresh from live provider refresh.

Remaining native review covers the approved inline control, number/suffix
stability, keyboard focus traversal, and reopening a retained Task workspace and
its real Session. A live Growth refresh remains unproven. The offline capture
cannot demonstrate completion recency or Session continuation; synthetic checks
cover recency and membership without claiming that native experience.

## Findings and cause

- The installed `lf roadmap --wave growth --json` still reports the duplicates
  as `completed:false`, `section:available`, with no runtime/workspace.
- A read-only query of the installed `pm_snapshots` item data found all seven
  identifiers (318, 315, 314, 313, 310, 308, 307) already `state:canceled` and
  `completed:false`, in snapshot `synced_at=1790935014`. The four retained target
  issues are `unstarted`. This establishes canceled-state handling as the cause
  in the observed snapshot, rather than an unobserved cancellation. No provider
  writes or new cancellations were performed.
- `pm/linear.rs::IssueFields::into_pm_item` correctly reserves `completed` for
  the completed workflow category. `lf/commands/waves.rs::task_summary` then
  discards `PmItem.state`. `task_section`, the next-move reason and
  `TaskFlowGate.plan_completed` consequently treat an unplaced canceled issue
  as available and startable. Changing the meaning of `completed` to include
  canceled would misreport success and affect completion consumers.
- `WorkSurfaceView.waveDetail` renders all roadmap Tasks and counts the raw list.
  The older `WaveDetailPane` also renders every Task. `RoadmapView` has its own
  completed-only labels/action rule. `WorkspaceProjection.inWorkingSet` uses
  completed plus Started and open Sessions. These consumers need a coordinated
  cutover; changing only the Wave row hides the defect elsewhere.
- Neither `PmItem` nor `PmTaskSummary` carries completion dates. Linear's
  [official schema](https://github.com/linear/linear/blob/master/packages/sdk/src/schema.graphql)
  exposes nullable Issue `completedAt` and `canceledAt` as their corresponding
  state transition times. Its [filtering guide](https://linear.app/developers/filtering)
  explicitly uses completion time for recent closed issues. No date inference
  from `updatedAt`, snapshot acquisition or runtime update time is necessary.
- This checkout uses normalized `pm_items` in `store/sqlite/planning.rs`; the
  installed store still uses `pm_snapshots`. Source and installed runtime differ.
  The source has the same lossy summary. Installed status for unplaced LOO-318
  returns `no Task exists`, so it cannot supply additional planning evidence.
- `put_item` rejects differing facts at equal provider revisions. Adding newly
  fetched completion-time field to an old persisted item can otherwise trigger this
  conflict without any provider edit. This is a required upgrade test, not a
  reason to weaken revision ordering generally.

## Chosen approach and source of truth

Linear owns planning state and transition dates. Existing PM storage owns the
captured facts; Rust owns the Task projection, execution evidence and legal
actions. Swift owns only history visibility and the selected time window.

Carry `state` and nullable `completed_at` through
`PmItem` → `PmTaskSummary` → Swift `PlanningItem` / `TaskPlanningSnapshot`.
Use RFC3339 timestamps consistently with existing projection times. Add the
fields to list and issue-detail GraphQL selections sharing `IssueFields`, and
validate non-null timestamps at ingestion. New provider responses must include
nullable keys; missing is not provider-confirmed null. Update all Rust/Swift
fixture producers and wire fixture assertions together, including task status,
roadmap and Wave detail. Do not add DTO defaults.

Keep `completed` meaning successful completion. Centralize provider terminal
classification (`completed`, `canceled`, `duplicate`) in Rust and reuse it in
projection and existing admission checks. Carry the observed state to consumers;
never classify from titles, descriptions or the word “Merged”. An unknown state
remains visible with unknown evidence. Old snapshots may have only `completed`;
that existing fact remains sufficient to recognize successful completion.

For an unplaced terminal Task, project Later, an accurate terminal next-move
reason and unavailable start controls. Never present cancellation as success.
Retain full history in the shared inventory; the Wave's default presentation
filters it. For placed work, keep execution, unresolved reviews, recovery and
Session access independent of provider closure. Do not change Task status,
settle a Flow, kill a process, remove a checkout or complete a conversation as a
side effect of this display change.

Old persisted JSON may lack the added completion date: absence means unknown until a fresh
observation. Permit enrichment at equal revision only for keys genuinely absent
in the stored JSON; once a nullable key was observed, differing same-revision
values remain a conflict. Older revisions must not erase newer facts. This is
compatibility for persisted planning, not a second store or perpetual wire
fallback. No timestamp backfill from unrelated events. No new SQL columns are
needed for these JSON fields; if implementation requires a migration, use this
Task's single draft and test from the released frontier.

## Visibility and counts

Use one small Swift history-filter value and one filtered collection for the
production Wave rows and their count. Default is hidden on first opening; retain
the chosen filter per Wave during the current window lifetime. No global saved
preference is needed. The existing alternate Wave detail uses the same rule.

- Completed initially selects 7 Days. Offer inline nonnegative integer editing;
  zero selects All Tasks (unbounded successful completion history). Empty,
  negative, non-integer or overflowing input shows feedback and retains the
  last applied range. Enter/blur applies; Escape cancels.
- The enabled control reveals successful completions only, using `completed_at`.
  Canceled/duplicate Tasks remain excluded at every time setting unless shared
  execution evidence requires them in current work. Label those exceptions
  Canceled/Duplicate while preserving their execution reason and legal actions.
  Keep canceled history in the unfiltered shared inventory, accessible through
  `lf roadmap --wave growth --json` and Linear; preserve existing Task workspace
  and Session navigation. Do not add a second cancellation-history control.
  Cancellation timestamps are unnecessary for this slice and are not added.
- Last N days is a rolling UTC duration: `now - N * 86400 <= timestamp <= now`.
  Capture one `now` per projection and refresh it on the existing refresh path
  and filter changes. Preserve subsecond precision. Exactly at the lower bound
  is included; future dates and unknown dates are excluded from bounded history.
  All time includes successful completions with unknown dates, labeled date unavailable. Reopening a Task
  makes it current regardless of a retained historical timestamp.
- Apply visibility in this order: preserve current planning and shared unresolved
  execution; hide remaining canceled/duplicate rows; apply the selected history
  window only to remaining successful completions. A historical timestamp,
  mere checkout existence or a terminal local status alone cannot decide visibility.
- Current planning rows remain visible even if their old local runtime is
  terminal: LOO-309 demonstrates why runtime abandonment alone cannot hide an
  open Linear Task. Closed planning with nonterminal or unresolved execution
  stays visible through existing shared execution/attention evidence. Open
  Sessions and retained panes remain reachable irrespective of the history filter.
- The Wave heading counts the filtered rows exactly and keeps the partial-data
  notice when inventory is truncated. Empty current work says “No current Tasks”
  while preserving Show completed. Empty bounded history does not claim that no
  older history exists. Failed planning reads keep the existing unavailable/stale
  presentation; they must not be converted into a truthful-looking zero.
- Preserve the roster's `active_tasks` execution metric as such; do not relabel
  it as the number of visible planning rows. Audit other badges that purport to
  count the displayed list and derive those from that same collection.

## Alternatives and failure modes

Title suppression is rejected: a renamed active Task would disappear and all
other clients would still offer execution. Treating canceled as completed is
rejected: cancellation would become success. Filtering terminal items out of
the provider query is rejected: history, Sessions and retained work would lose
their inventory. A new archive store or a completion timestamp invented at first
observation is unnecessary and untruthful.

Wild success: Jack scans current work immediately, retrieves a finished Task
without losing the workspace, and trusts both labels and counts. Wild failure:
an old canceled issue looks successful, a reopened Task vanishes, or adding dates
breaks refresh at the same revision. The acceptance scenarios below target those
failures directly.

## Implementation state — one coherent delivery

The provider and stored PM facts now carry completed_at; Task summaries retain
state. One Rust terminal classifier supplies admission, section and next-move
reason without changing successful-completion semantics. Equal-revision enrichment
inspects raw stored key presence and rejects changing an observed nullable value.

Desktop uses the same history filter for Wave rows and counts. Per-Wave window
state retains the chosen window. The alternate detail consumes the existing Flow
projection to preserve unresolved reviews. Open Session membership remains outside
history visibility. Missing planning no longer infers successful completion from
local abandonment.

Jack Heart relayed a coordinator review finding on October 2: settled completed
Tasks with removed checkouts had stale recovery conditions. The filter now ignores
that historical missing-checkout condition for terminal runtime. A nonterminal
runtime, unresolved pinned Flow, or observed unsettled files/commits preserves
current work. A removed checkout alone does not. Regression coverage includes
completed + done + missing checkout + recovery_required, as requested.

Simulated code review found and repaired two related gaps: the alternate Wave
reader had discarded Flow evidence needed to retain reviews, and the missing-plan
fallback called local abandonment successful completion. Neither now supplies a
false history or success signal.

## Reconciled evidence and remaining work · 2026-10-02

The earlier affected gate passed 145 Rust tests (two intentional ignores), 68
Swift tests, then five Task history tests with added production Start-button
assertions. Formatting, all-target Clippy, architecture, Swift boundaries and
configured Xcode build-for-testing passed. Gate closed the earlier gaps for all
seven duplicate exclusions at hidden/7-day/30-day/unbounded settings, row-opening
actions, invalid-input feedback and enabled Start actions. The shared fixture
crosses provider validation, storage, projection and RegistryQuery into the
production view. Runtime and Session variants remain synthetic.

The subsequent approved inline revision and compression passed seven focused
TaskHistoryFilterTests and rebuilt the SwiftPM app. One persistent NSTextView
handles display and editing; the custom renderer is deleted. Headless bitmap
comparison uses the same viewport bounds, origin, scale and clipping without
selection paint, and verifies nonempty glyph ink and native baselines for one
and multiple digits and Int.max. Long values scroll in the compact viewport.
The Days suffix activates editing. Blur preserves the next responder;
Enter/Escape retain traversal and Tab/Shift-Tab commit and traverse. Wave identity
owns the control so a draft cannot cross Waves. Compression derives editability
from editing state and reuses publication; production row assertions read each
checkpoint once. No separate unbounded toggle or persisted draft remains.

Source review confirms the plan, production bindings and tests agree. Rust owns
terminal facts and legal actions; Swift owns only the visible range. Unplaced
actions reuse the projected next-move reason. Missing planning does not infer
success from runtime abandonment. No migration or cancellation timestamp was
needed. Shared Task association and Session membership remain independent.

Remaining acceptance:
- Revised inline gate passed: 142 Rust tests, 100 headless Swift tests, required
  static checks and configured Xcode compilation. CI owns the full matrix.
- Native review judges the approved control's appearance, focus traversal and
  retained workspace/Session continuation. Jack's screenshot and prototype
  approval do not establish acceptance of the final native revision.
- Live provider refresh remains distinct from the prepared offline snapshot.
  Publication, landing and Task completion remain with the caller.

The decision-schema repair is now identical to active base 2064555c4; it is no
longer a feature-branch overlap. Historical failed-occurrence notes do not establish
current Flow state or successful recovery. No Flow navigation follows from this
reconciliation, and no additional product decision is needed.

The retained acceptance uses fixed-time boundaries (exact lower bound, one
subsecond before, now, future, UTC offsets and DST), missing dates, reopening,
unknown provider states, absent versus observed-null keys, older revisions and
equal-revision conflicts. Missing checks never weaken these requirements.

This serves Product's Wave → Task clarity. Snapshot and headless evidence do not
prove sustained Desktop-use KRs or external repository progress. Scope excludes
provider edits, cancellation sweeps, Project rotation, automatic recovery,
Session retirement and primary-runtime changes.

Check: bounded affected gate (`/tmp/loo369-gate.py`, standard scripts/test.py runner) + terminal admission regression — 142 Rust and 100 Swift tests passed; fmt, Clippy, architecture, Swift boundaries, SwiftPM and Xcode build-for-testing passed; `git diff --check` passed; full matrix to CI, native/live review remains with demo. Exact commands and evidence: docs/reviews/task-history.md.
