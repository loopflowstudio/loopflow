# LOO-905 — Readable Task identity through planning recovery

## Problem and outcome

Desktop can show “Task unavailable · task_<internal id>”, “Summary · Planning
unavailable” and “Planning unavailable” alongside “Loading work…”. The Task
brief records this across Loopflow and manabot on November 14 and after reopening
completed LOO-673. Those are reported observations, not a measured reproduction
in this checkout. Their duration and the LOO-673 conversation failure are unknown.

Keep known Task names and identifiers readable while independent Work readings
arrive or recover. First reads say loading; saved facts say updating/last known;
failed reads name the failure and offer recovery. A missing full Task record
must not turn an already named list member into an opaque ID.

This serves Product's usable native workspace and sustained Session access KRs.
It does not establish either KR, external-project progress, or a launch latency
improvement. LOO-831 owns current planning within 500 ms; this fix must work with
arbitrarily delayed planning. LOO-903 owns backlog redesign. Preserve #1851 /
LOO-434's open delivery interaction choices.

## Findings and current architecture

- Rust's local planning store is authoritative. In
  `rust/loopflow/src/store/sqlite/selections.rs::plan_in`, Task `ViewContent`
  already receives the local Task's title and identifier. Tracker delivery is not
  needed to name a locally created or reopened Task. Missing facts instead produce
  `title == work.id`, no identifier and an unavailable explanation: that is not a
  human-readable title and must not leak through the new presentation path.
- `WorkViewPlan.Content` in `swift/Loopflow/Models/WorkSelection.swift` carries
  those fields today. `WorkModel.applyTaskList` retains a repository/Machine- and
  Wave-scoped `WorkReading<WorkViewPlan>`. `sidebarTaskIDs` uses that reading's
  members and states, but `WorkNavigator.taskRow` names them only through
  `model.task(id:)`, which reads the separate roadmap/selected Task evidence.
  Thus Task-list-before-roadmap is a concrete source path to the reported labels.
  It is not yet proof of the incident's complete timing or cause.
- `WorkModel.workspaceTitle` already consults selection-plan titles and retained
  workspace titles, but omits the independent Task-list content. Sidebar, title
  and breadcrumb consumers need one consistent display decision.
- `WorkCache` already saves successful roadmap wire text, Waves, Session pages
  and selection inside a Machine; it does not save Task-list or selection plans.
  `WorkModel.restore` makes saved roadmap names available before I/O. Keep this
  cache and its quieting rules. No additional identity cache is needed for the
  first slice: live Task-list titles bridge the stream-order gap, existing saved
  roadmap titles bridge relaunch. Saved membership is not current membership.
- `publishRoadmap` / `planningFailure` already retain last-good planning and cache
  on failed/truncated replacements. `WorkReading` distinguishes loading,
  available, and unavailable-with-last-good. `workStatus` currently considers
  roadmap and Sessions, omitting Task-list failure; `WorkNavigator` also renders
  an empty-list assertion while no list has answered. These gaps need correction.
- `keepWorkCurrent` owns reconnect/backoff. `refresh()` sends a refresh to an
  existing observation or performs one-shot reads. Reuse these, including request
  floors and scope fences; add no view-owned polling or second subscription.
- `openLinkedTask`, `mountedTaskPage` and `taskOpening` already distinguish request
  acceptance, page mounting and native Session readiness. A readable row cannot
  mark an opening usable. Existing readiness, cancellation and takeover rules stay.

The README's promise of saved rows and one truthful failure status is stronger
than the present independent Task-list/sidebar handling. Update the implementation
and its tests to meet that promise, then clarify the README's partial-read states.
The Wave's prior failures with truncated planning and stale actions remain relevant;
existing retention tests must keep passing.

## Chosen mechanism — this slice

One indivisible Swift presentation repair; no schema, DTO or protocol change.

1. Add a small model-owned Task row presentation derived from existing readings,
   not a mutable inventory. Resolve by the exact repository/Machine scope and
   Task Work key: current Task record, the shown Task-list/selection content,
   then known saved/retained identity. Resolve planning IDs and runtime Work IDs
   through existing mappings; never match names or parse IDs into identities.
   A fresh named list member can update an older retained name. Track the source
   of display text so saved/failed evidence is visibly last known, not current.
   Reject content whose only title is its opaque Work ID. With no known name,
   show “Loading Task…” during an outstanding read and “Task unavailable” after
   failure; internal IDs belong in diagnostics, not the row heading.
2. Replace the sidebar's three nil-based unavailable strings with that presentation.
   Use it for workspace titles and breadcrumbs where the same identity is shown.
   Show a description preview only when a description exists; never manufacture
   “Summary · Planning unavailable”. A title-only member gets planning progress
   or failure context rather than a fake summary. Preserve genuine Session
   preview provenance and independently observed Session status.
3. Derive list loading/failure from the scoped Task-list reading, not from a nil
   Task lookup. A successful empty list says no working Tasks. A first pending
   list says loading; a failed list says unavailable. Saved planning says Updating
   or Last known, even when Sessions are still loading. Coalesce common failures
   into one workspace notice; surface independent list failures even if planning
   and Sessions succeeded. Keep readable last-good rows in place.
4. Put a Retry action beside the actual failure notice, using the existing model
   refresh owner. It must recover the failed scoped list as well as planning,
   work with a live subscription, and remain usable across disconnect/reconnect.
   Extend the shared refresh path if it fails that scenario; do not launch a
   separate reader from the view. Clear the notice on successful replacement.
5. Add focused stream-order and production-view tests, update the startup wording
   in `swift/README.md`, and retain opening-readiness regressions. No new provider
   launch, claim, Graph execution or destructive recovery follows from display data.

## Delete — do not maintain

Delete the raw-ID heading expression in `WorkNavigator.taskRow`, its synthetic
summary fallback and the unconditional missing-record metadata error. Replace
their behavior in place; keep Task metadata/actions, keyboard slots, ordering,
Session previews and retained panes. Do not add a Legacy/New path, a title store,
another read loop, or stale legal actions. Cached identity is display-only; existing
action freshness checks still apply.

## Alternatives and failure modes

- Waiting for the complete roadmap before rendering Task-list members hides useful
  local facts and couples legibility to LOO-831. Use the existing independent titles.
- Persisting complete plans introduces stale states, argv and location authority
  needing another stripping contract. Existing quiet roadmap storage covers known
  relaunch names; add no plan persistence in this Task.
- Replacing the Work reader or tracker sync does not address the demonstrated
  nil-to-unavailable rendering. Only change shared transport if a focused recovery
  test exposes a defect in its existing refresh contract.

Success: a delayed read becomes unremarkable; names remain stable, and recovery
changes context without disturbing the conversation or draft. Failure: retained
names masquerade as current Task state, cross into another repository/Machine,
or suppress a real failure. Test those transitions directly. A fresh complete
reading that removes a Task must not resurrect it from saved membership; an open
retained conversation may remain reachable with last-known identity.

## Acceptance and demo

Focused implementation check: add `TaskPlanningReadabilityTests` using the existing
controllable Work feed and production leaf views; run
`scripts/test_desktop.sh --filter TaskPlanningReadabilityTests`.

Gate runs once:
`scripts/test_desktop.sh --filter 'TaskPlanningReadabilityTests|TaskRowMetadataTests|WorkCacheTests|WorkModelTests|WorkNavigationTests|WorkOpeningCLITests|WorkDestinationTests/composedOpeningWaitsForNativeSessionAndComparison|WorkDestinationTests/conversationAttachmentRecovery'`.
This builds the Desktop test target and bundled CLI and runs with WindowServer
denied. Expected: all selected tests pass; display diagnostics are skipped rather
than counted as native acceptance.

The regression scenarios must cover:

- Two repository models sharing a Machine cache, restored named Tasks, delayed
  planning and independently arriving Task lists/Sessions in both orders. Inspect
  actual row and breadcrumb text at each controlled frame; no opaque heading,
  false unavailable message or premature empty-list assertion.
- No cache; Task list first; no name anywhere; genuine empty list; missing-facts
  content with `title == work.id`; repository, Wave and Machine switches while
  late replies arrive. Unknown identity stays unknown without exposing IDs.
- List-only failure, planning-only failure, whole-stream disconnect, truncated
  replacement and recovery through both a later frame and the real Retry button.
  Retain names, selection, panes and drafts, label stale facts, and remove errors
  after recovery. A complete removal must not repopulate current membership.
- Locally created and reopened completed Tasks with pending tracker delivery:
  local names appear before provider writeback. Exercise the real local planning
  writer through a private fixture Machine and its Desktop consumer (extend
  `WorkOpeningCLITests`); no live tracker mutation is required. Opening remains
  opening until the mounted Session is input-ready, or reports its own failure.

Demo: `uv run python scripts/loopflow-dev.py run` launches the candidate. In review,
reopen saved Tasks in two repository windows, observe delayed planning, recovery,
and an isolated reopened completed Task. Record elapsed launch-to-named-row and
launch-to-current-reading separately from conversation input readiness. Capture
the visible labels and recovery interaction. This diagnostic requires a display;
it is not an unattended gate prerequisite. Do not reopen or mutate live LOO-673
merely to recreate the historical report. If its opening failure remains, record
the distinct stage/reason rather than assigning it to planning without a trace.

Done when startup never falsely declares a pending Task unavailable, known names
survive refresh/failure, true failures have working recovery, newly created and
reopened local Tasks remain identifiable during tracker delay, and Session access
keeps its own truthful readiness. Native timing/render observations remain separate
from headless test results; missing native observation is reported as unverified.

## Checks and remaining work

### Concept review — November 14

Outcome: iterate to implement. No open product choice. Keep the single derived
Task presentation, existing cache and refresh owner; refine what its context means.

The interaction has three independent facts: the Task's readable identity, the
reading being recovered, and the conversation's input readiness. Opening a named
Task keeps its conversation and draft; Retry refreshes readings, not execution.
A list failure changes membership confidence, not a name supplied by a successful
roadmap. A roadmap failure does not stale a name supplied by a successful list.

Next implement direction (proposed, not built):

- Carry the chosen identity source's freshness through `taskPresentation` instead
  of reducing roadmap, list and selection failures to one `failed` Boolean.
  Resolve current readable identity before retained identity, keeping the existing
  preference among current sources. Keep `WorkReading` available until after the
  display source is chosen; flattening `.value` first loses this distinction.
  Saved identity says Updating; retained identity after its source fails says
  Last known. A successful independent source may immediately clear that label
  while another read's error remains in the workspace notice with Retry.
- Distinguish absent Task details from a loaded Task with no description.
  `rowDetail` currently renders Loading planning when a current full Task has an
  empty description, no live excerpt and no identity context. Omit that preview
  line once details are loaded. Never use missing prose as a progress signal.
  Preserve shipping context and genuine Session previews/status independently.

Counterexamples for focused checks: a current roadmap name with a failed list;
a fresh list name with a failed roadmap; failed retained list content alongside a
current selection-plan name; a loaded Task with an empty description; and a
title-only list member whose details really are still pending. In each recovery,
only the successfully replaced source clears its error. Saved names and membership
must still yield to a complete current removal. Repository/Machine scoping,
late-reply fences, panes, drafts and input readiness remain required.

No DTO, schema, persistence or transport change follows. This is presentation
derived from existing readings, without a new Task lifecycle or polling loop.
No skill instruction needs changing. Proposed README clarification after repair:
“Last known describes the reading supplying the displayed name. Other read
failures remain visible beside Retry. A Task without a description has no summary
line.” Keep the broader recovery and native-demo checks below.

Next show the same named row through independent list/roadmap failure and recovery,
then a loaded description-free Task. The walkthrough is
`scratch/concept-review.html`. This review ran no new app or test checks; the
results below belong to the earlier implementation run.

- `swift build --build-tests --package-path swift` — passed, including the app
  and all test consumers of optional Task metadata.
- `scripts/test_desktop.sh --skip-build --filter 'TaskPlanningReadabilityTests|TaskRowMetadataTests|WorkOpeningCLITests/localCreatedAndReopenedNamesPrecedeRoadmapDelivery|WorkModelStreamTests/incompletePlanningRetainsSavedWork|WorkCacheTests/failedRefreshKeepsSaved|WorkCacheTests/returningLaunchRestores|WorkNavigationTests/dashboardSidebarHeader'`
  — 25 tests in 6 suites passed in 31.144 s, WindowServer denied. Covers saved
  names in two repository models, list/Session ordering, real Retry, reconnect,
  stale one-shot replies, complete removal, and local create/complete/reopen
  before roadmap delivery. Preserved pane/draft/truncated-read checks pass.
- `scripts/test_desktop.sh --skip-build --filter 'WorkDestinationTests/composedOpeningWaitsForNativeSessionAndComparison|WorkDestinationTests/conversationAttachmentRecovery'`
  — 2 tests passed in 1.166 s, including 8 readiness outcomes and attachment
  recovery without replacing the retained Session or draft.
- `uv run python scripts/check_architecture.py` — passed.
- `uv run python scripts/check_swift_multiplatform_boundaries.py` — passed.
- `git diff --check` — passed.
- Gate owns the broader filter in Acceptance, plus `documented_commands::` for
  the README edit. The focused readiness results above remain applicable.
- Native demo/review remains unverified: rendered labels and Retry in two
  repository windows, launch-to-name/current-reading timings, retained native
  input, and pending tracker delivery. The isolated CLI check establishes local
  create/reopen identity without a roadmap read; it does not recreate a tracker
  timeout. No live LOO-673 mutation or provider-input success is claimed.

November 14 kickoff observations retained separately from current state:
`lf task status LOO-905 --json` showed the code Graph at kickoff, no PR, checkout
`3fe914c022f9b75e8568e5840674e4a5ce6321e7`, and no earlier plan.
`lf open --host desktop --file swift/README.md` exited 1:
“Error: task_faf6f1d7fda149a1a81ff2ade281f053 has no workspace in this Desktop window; nothing changed.”
No alternate editor opened.
