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
and requested implementing it in Desktop. This supersedes the separate buttons,
range menu, and positive-only input choices below. One compact checkbox and label
shows Completed when off, N Days when bounded, and All Tasks when days is zero.
Click the off label to enable the remembered range (initially 7); click the active
label to edit and select the number in place. Enter/blur applies, Escape cancels.
Empty, negative, fractional and overflowing drafts retain the applied range.
The checkbox hides history and remembers the range. No permanent border or fill.
The checkbox, number glyphs and Days baseline must not move on entering editing.

### Delete — do not maintain

Remove the separate Show completed/All time toggles, redundant `allTime` and
persisted `daysText`, and positive-only validation. Preserve one applied day count,
zero for unbounded history, existing caller bindings, lifecycle predicates,
completion dates, counts and Session access. Native transient editing owns drafts.

## Problem and demo

Jack sees seven canceled duplicates among 13 Tasks on Growth. Current work must
be easy to scan, with completed Tasks hidden initially and history available
through Completed: initially 7 Days, another positive day count, or zero/All Tasks.

At the authored demo boundary, open Growth in the configured Desktop and refresh.
The seven canceled duplicates are absent from current work; LOO-309, LOO-316,
LOO-312 and LOO-306 remain reachable. Enable Completed, click 7 Days, apply 30, then apply 0 for All Tasks. Rows and the Tasks count change together; Show completed adds only successful
completions. The seven canceled duplicates stay absent at every time setting. Reopen a retained Task workspace and its Session. The review
judges this actual Wave interaction; headless fixtures do not claim visual approval.

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

Remaining: run the affected gate suites and configured app-build check below;
then demonstrate Growth and retained Session navigation at the authored review.
No provider edits, installation, publication, merge or visual approval is claimed.

## Reconciliation and remaining work

October 2 source review confirms the implementation and compression agree with
the reviewed behavior. Unplaced actions reuse the projected next-move reason;
the unconditional ready-to-start fallback is deleted. Swift lifecycle predicates
read facts independently of display labels. No schema migration or cancellation
timestamp was needed. The existing Task workspace association and Session
membership remain independent of Wave history visibility.

The shared fixture proves provider/storage/projection facts. The production-view
test exercises hidden, 7-day, 30-day, invalid and All-time controls and counts;
runtime and Session variants are synthetic. It checks all seven duplicate IDs
initially and at All time, but does not yet explicitly check each intermediate
window or enabled actions in the production view. Retained Session coverage
proves working-set membership, not reopening the native Session. Gate retains
those acceptance obligations below; source review does not promote them to passed.

Remaining work is affected gate acceptance and configured app compilation,
followed by the Growth refresh and retained-workspace/Session demo. The existing
Product memory now retains the lifecycle, recency and stale-recovery lessons.
No additional product decision is required by this reconciliation.

## Done when and gate

One cross-boundary synthetic scenario includes the seven canceled duplicates,
four retained targets, recent/old/unknown-date completions, a reopened Task and
a closed Task with unresolved execution. Feed provider facts through Rust PM
storage and projection, assert the shared DTO fixture, then load that same
shape through `RegistryQuery` into the production Wave view with ViewInspector.
Exercise Completed, 30 Days, invalid input and zero/All Tasks through the native
control and the actual production filter binding. Assert row
identities, outcome labels, enabled actions and displayed count, not just helper
return values. The seven canceled duplicates remain absent for hidden, 7 days,
30 days and All time; a canceled Task with unresolved execution remains visible.
Verify canceled history remains in the full inventory and retained Session access
survives filter changes. No window server, provider writes or UI automation are required.

Planned headless gate commands and expected results:

```sh
cargo test -p loopflow --lib pm::linear::
cargo test -p loopflow --lib store::sqlite::planning
cargo test -p loopflow --lib lf::commands::waves
cargo test -p loopflow --lib ops::task_flow
cargo test -p loopflow --test dto_fixtures
swift test --package-path swift --filter 'DTOFixtureTests|DesktopHeadlessTests|WaveDetailReadingTests|WorkspaceNavigationTests|RoadmapViewTests|TaskHistoryFilterTests'
uv run python scripts/test.py --loopflow
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Acceptance requires affected suites and configured app compilation at gate.
The focused TaskHistoryFilterTests suite and SwiftPM app compilation are covered
by implementation sanity checks; they do not replace gate or visual review.
Time tests use a fixed clock: exactly 7 days, one subsecond before, now, future,
timezone offsets, DST crossing, missing dates, another positive day count and
All time. Provider tests cover completed/canceled/duplicate/open/unknown states,
reopening, genuine null versus absent fields, older-revision replay, and equal
revision enrichment/conflict. Build unavailability belongs to capable gate/CI;
native visual judgment belongs to demo/review. Implementation runs only its
focused proof and changed-code build before the affected gate suites.

This serves Product's Wave → Task clarity and sustained Desktop-use KRs. It
does not itself prove three working days or three two-hour sessions, external
repository progress, a new performance target, or successful live deployment.
Excluded: another cancellation sweep, edits to retained targets, Project
rotation, automatic lifecycle recovery, Session retirement, and primary runtime.

Check: `git diff --check` — passed for reconciliation; recorded focused implementation/compression results reused from task-history-handoff.md; affected suites, final Clippy and configured build remain with gate, native judgment with demo/review.

## Gate update · 2026-10-02

Affected headless acceptance and configured app compilation passed. Gate added
all seven duplicate exclusions at every window, row-opening actions, invalid-input
feedback, and production Start-button assertions for canceled/completed/open Tasks.
The earlier pending gate notes are superseded by this result. Live demo remains
pending: the configured installed CLI still omits the new planning fields.
See [the durable demo instructions and limits](../docs/reviews/task-history.md).

Check: Rust affected suites + DTO fixtures — 145 passed, 2 intentional ignores; Swift affected suites — 68 passed, final Task history — 5 passed; fmt, Clippy, architecture, Swift boundaries, configured Xcode build-for-testing and diff check — passed; native Growth/Session demo pending compatible installed runtime and Jack Heart's review.

## Demo revision · 2026-10-02

Jack Heart supplied a Growth screenshot showing six current Tasks and no canceled
duplicates. Jack called the separate completed checkbox underdesigned and requested
integration into the Tasks header. The header now places its count on the left and
compact history toggle buttons on the right, with the selected time window inline.
Filtering behavior and validation remain unchanged. Visual acceptance of this
revision remains pending; no landing approval was given.

Check: `swift test --package-path swift --filter TaskHistoryFilterTests` — 5 passed; configured Dev app install/build and `git diff --check` — passed; header visual review pending.

## Approved inline implementation · 2026-10-02

The Desktop header now consumes the approved single checkbox/range control in
both Wave views through the existing binding. One persistent native NSTextView
displays and edits the number; Days and the checkbox keep their positions.
Long valid values scroll inside the available numeric space rather than widen
into the Task heading. Zero is the only unbounded-range representation.

Review found that the Days suffix must also activate editing; it now shares the
label action. Implementation review removed the custom text renderer; one actual
NSTextView now handles both modes. This removes the cell/editor comparison and
its frame/clipping ambiguity. The bitmap proof captures that same native surface
through identical viewport bounds, origin and scale, with selection paint removed.
Wave identity also owns the native control so drafts cannot cross Waves. The headless interaction proof uses the production binding and
AppKit control; layout proof covers one/multiple digits and Int.max, the same
text surface, native glyph baselines, rasterized glyph bounds, and selection.
Blur commits without changing first responder; Enter/Escape keep the text surface
in the key loop, and Tab/Shift-Tab commit and traverse normally. Native visual
judgment and windowed focus traversal remain with the caller's review; no GUI
was launched in this pass. Long numbers scroll within the compact numeric
viewport; selection and replacement still use the complete value.

Compression derives native editability from `editing` in `update` and reuses
`publish` when entering editing. The production-view proof reads row identities
once per checkpoint for exclusions, opening actions and counts; its redundant
final exclusion loop is removed. Review confirmed that the Delete list above
is satisfied: no separate All-time toggle or persisted draft remains. The shared
projection and retained Session paths need no further cut.

Check: `swift test --package-path swift --no-parallel --filter TaskHistoryFilterTests` — 7 passed (1.634s), SwiftPM Desktop app rebuilt after compression; `git diff --check` — passed; caller owns full queue gate and delivery, native judgment remains with demo/review.
