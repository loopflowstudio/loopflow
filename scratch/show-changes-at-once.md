# LOO-931: Changes show at once, reported through one Notice

The vocabulary (Machine, Subscription, Reading, Saved readings, Revision,
Pending change, Request ID, Notice, connected/blip/away) is the brief's table;
this plan uses those words and adds none. LOO-906 at `69d8b36466` is reference
code only.

## Demo

Desktop, a repository with three Working Tasks, LOO-2 selected. Click Complete:
in that frame LOO-2's row is gone, LOO-3 is selected, the Wave count drops by
one. Do the same against a remote Machine over the tailnet: same frame. Stop the
Machine, click Claim on LOO-3: the owner shows at once; ten seconds later the
change controls grey out and the list reads as last read. Start the Machine:
the claim goes out under its original request ID and the controls come back.
Complete a Task whose PR is still open: the row comes back and the Notice reads
"LOO-4 couldn't be completed: its PR #12 is open · Show". On the phone, swipe a
Task to Complete it; tap Auto-Ship and the Notice reads "LOO-5 Auto-Ship
requested".

`timings.py` shows the Complete → row gone journey for both Machines at about
one frame, with accepted and confirmed times beside it.

## Where the wait comes from today

- `WorkModel.performTaskAction` (`swift/LoopflowMac/WorkModel.swift:2691`)
  awaits the write, then `confirmWorkUpdate` (a checkpoint round trip), before
  anything changes on screen. Only Abandon hides first (`abandoningTaskIDs`).
- The Machine replies after `sync_after_save`
  (`rust/loopflow-execution/src/ops/planning_sync.rs:71`): `settle_completion`
  (`ops/task.rs:~2420`), `task_reopen`, `task_ownership` (also awaits Linear
  `assert_assignee`, `ops/task_ownership.rs:66-68`), `task_edit` (Move),
  `save_abandon` (`ops/task/lifecycle.rs:142`). There is no periodic planning
  loop; pending exports are durable rows and the next sync sends them, so a
  sync that runs after the reply loses nothing if the server dies.
- Abandon's PR and branch cleanup runs after the save and its failure is the
  answer ("Abandonment is incomplete; retry"). It stays inline: the row is
  already hidden, and the answer must still say whether cleanup finished.
- Writes run through `Write::run` (`rust/loopflow/src/lf/commands/write.rs`)
  inside `effect()` (`rust/loopflow/src/server/service.rs:~1406`). The answer
  is the command's JSON; nothing says which store revision it committed at.
- Frames (`SubscriptionFrame`, `rust/loopflow-execution/src/read_stream.rs:84`)
  carry `answers`, `unavailable`, `body`, `work`. The reader knows `read_at:
  StoreRevisions` but does not send it. A frame goes out only when the body
  changes or a request forced it (`checkpoint`).
- Both apps read Task lists as `WorkViewPlan`
  (`swift/Loopflow/Models/WorkSelection.swift:159`): Desktop's `task_list`
  part, the phone's `RemoteTaskObservation`. Desktop counts come from
  planning rows (`startedTaskCount` → `standupTasks`), not the list.
- Request-ID memory exists only for `task.create` (`creation_receipts`, with a
  fingerprint refusing a different payload under the same ID).
- Phone has no Task actions; its feedback is inline `error` strings.
- Wave click: `WorkNavigator.swift:211-215` sleeps 120 ms before `select`.

## Chosen approach

### Machine

1. **Answer after commit.** `sync_after_save` keeps its signature. Inside a
   served request it records the repository on the request's after-reply list
   (`journal::request`, next to `diagnostics`) and returns; the CLI path runs
   it inline as today. `effect()` drains the list after `run` and spawns each
   with `journal::request::spawn_blocking`, then returns the answer.
   `assert_assignee` moves into the same deferred job. `retirement::schedule`
   already schedules.
2. **Revisions on every write answer.** `effect()` reads the store's revision
   map before and after `run`. `ServerMessage::Response` and the write's
   `Error` both gain `revisions`: the domains whose revision moved, with the
   value after. Required, nullable (null for reads and server operations).
   Concurrent writes can add domains; that only delays a drop, never
   mis-drops one.
3. **Revisions on every reading.** `SubscriptionFrame` gains `read_at`: the
   revision map of the snapshot the reading used, null when there was no
   store. One map type, `Revisions` (domain → i64), read from every row of
   `store_revisions`, so `task_updates` and LOO-934's split domains appear
   without a type change. `StoreRevisions` stays the reader's internal wake
   test.
4. **Request IDs.** `ClientMessage::Request` gains `request_id`
   (required, nullable). With one, the server looks it up in a new
   `command_answers` table (`request_id` PK, `fingerprint` = sha256 of the
   operation JSON, `answer` JSON or `error` JSON, `revisions`, `answered_at`).
   A stored answer is returned as is; a different fingerprint is `invalid`;
   an in-flight duplicate waits on the first (in-memory map of request ID →
   watch channel). Answers older than a day are deleted on insert. An
   `outcome_unknown` (worker join failure) is not stored. Writing the answer
   is its own transaction after the command's; a crash between them reruns
   the command on a repeat, which is safe for these commands by value.
   `task.create` keeps `creation_receipts` (LOO-932 owns creation).
5. Protocol revision 7, `min_revision` 7: old clients get the existing
   incompatible error that names the update.

### Shared Swift (`swift/Loopflow`)

- **`Reading<Value>`** replaces Desktop's `WorkReading` and the phone's
  `TaskListReading`/`MobileTaskReading`: `value`, `readAt: Revisions?`,
  `failure: String?`, `lastGood`. A reading at a lower revision never replaces
  one at a higher revision for the same subscription.
- **`SavedReadings`**: wire bytes per (Machine, subscription key) under the
  app's cache directory, read through the live decoder. The phone's
  `tasks-<machine>-<filter>.json` files move onto it; Desktop saves its
  `task_list` readings through it. `WorkCache` keeps roadmap, waves and
  sessions (moving those is the closing names pass, not this Task).
- **`PendingChanges`** (`@MainActor @Observable`, one per Machine): request
  ID → `{ command, patch, answer: Revisions? }`. The one table:

  | Command | Patch on shown rows | Shown as |
  |---|---|---|
  | Complete | hidden from every list but Done; disposition done | gone |
  | Abandon | hidden from every list; disposition abandoned | gone |
  | Reopen | disposition open; section Working if owned and started, else Backlog | moved |
  | Claim | owner = this person; section Working if started | owner shown |
  | Unassign | owner nil; section Backlog | moved |
  | Move | Wave = target | moved between Wave lists |
  | Session rename | Session title | new title |
  | Start, Auto-Ship, Stop Auto-Ship | none | Notice "… requested" |

  The section rule in Reopen/Claim/Unassign is the only derivation the client
  does; a covering reading always wins over it.
  `apply(to: WorkViewPlan)`, `apply(to: RoadmapSnapshot)`,
  `apply(to: [SessionRecord])` return patched copies; counts read the patched
  rows. A change applies to a reading unless the reading's `readAt` is at or
  past every domain in its answer. It is removed once every live reading of
  its Machine is past it. Never by value, never on a timer.
- **Answers.** Success: store `revisions`, send a checkpoint for the parts the
  command touches (forces a frame even if the body did not change, so the drop
  needs no second mechanism). Error with no revisions = refused: remove the
  change, the row returns, raise the Notice. Error with revisions = partly
  done (Abandon cleanup): keep the patch until covered, raise the Notice with
  the error.
- **Link state per Machine**: connected, blip (lost, under ~10 s), away.
  Fed by `ServerLink`/subscription disconnects and the first frame after
  reconnect. Blip: outstanding changes stay in memory and are resent with
  their request IDs when the link returns. Away: change controls disabled,
  last readings shown read-only; outstanding changes wait for the link or
  quit. Nothing new on disk.
- **`Notice`**: `{ id, text, action: (title, handler)?, kind: event | condition }`.
  One `NoticeCenter` per window (Desktop) and per app (phone); one
  `NoticeView` in shared Swift: an overlay at the bottom, one line, at most
  one action. Events dismiss after six seconds; conditions (away) dismiss when
  they end. A new Notice replaces the current one. Posting the same key while
  shown refreshes it instead of stacking.
- **Next-Task rule** (Complete, Abandon of the selected Task): the next row
  below in the patched list order, else the row above, else the empty state.
  Selection happens in the click, before the write is sent.

### Desktop

- `completeTask`, `abandonTask`, `reopenTask`, `assignTask`, `moveTask`,
  session rename go through `PendingChanges` and return synchronously.
  `startTask`, `shipTask`, `stopShipping` send with a request ID and post
  "requested"; Auto-Ship's Notice carries Undo (Stop Auto-Ship).
- Sidebar rows, board tiles, Wave counts and the Task header read the patched
  readings. The planning-rows bridge in `sidebarTaskRows` goes: saved
  `task_list` readings draw the first frame instead.
- Wave click selects in the click; the pressed highlight animates over the
  already-selected row.

### Phone

- Task rows get Complete (swipe, trailing) and, in the workspace toolbar,
  Start (unstarted), Auto-Ship (Working) and Stop Auto-Ship (Autonomous),
  through the same `PendingChanges` and Notice.
- `TaskFeed` keeps `Reading<WorkViewPlan>` from `SavedReadings`; inline
  `error` text for load failures moves to the Notice.

## Delete — do not maintain

- `TaskRunDraft`, `WorkNavigation.taskRunDrafts`, `performTaskAction`,
  `abandoningTaskIDs`, the "Updating Task…/Task updated; refreshing…" header
  text and `acting` gating (`TaskWorkView.swift:~95-110`); the
  `confirmWorkUpdate` calls after Task commands (the checkpoint now belongs to
  `PendingChanges`).
- `ShipNotice`, `ShipNoticeToast`, `ShipNoticeOverlay`,
  `WorkNavigation.shipNotice` (`TaskWorkView.swift:481-550`, `RepoView.swift:71`).
- `WorkModel.dashboardError` and its readers in `DashboardView`,
  `WorkNavigator`, `WavesView`, `WaveStandupBoard`.
- `CreationRecoveryView` and its strip in `SessionsView.swift:1709`.
  `CreationRecovery` stays (LOO-932 owns creation) but posts its items as
  Notices ("LOO-9 was created · Open") instead of driving a strip.
- `WavePlanningNotice` and `WorkNavigation.expandedNotices`
  (`WaveStandupBoard.swift:160, 383`): a Task outside the plan raises one
  Notice per board load, action "Copy recovery".
- Inline load-failure text in views (~50 sites across `TaskWorkView`,
  `WaveDetailPane`, `TelemetryDashboardView`, `WavesView`, `WorkPalette`,
  `WorkNavigator`, `TaskHistoryView`, `TaskCommentsView`, `DashboardView`,
  `SessionsView`, `WorkActivityView`, `RoadmapView`, `PeerPlanningView`,
  `WorkSurfaceView`, `WorkspacePanesView`, `WaveStandupBoard`; phone
  `TaskListView`). A reading's transition into failure posts one event Notice
  keyed by its part; the content area keeps the last good reading or stays
  empty. Sheet input validation (New Task, rename fields) is not a load
  failure and stays inline.
- `WorkNavigator.swift:211-215` sleep.
- Phone `TaskListReading`, `MobileTaskReading`, `TaskFeed.savedURL`/`save`.
- Tests that exist only for the deleted surfaces: the `taskRunDrafts`,
  `abandoningTaskIDs`, `dashboardError`, `ShipNotice` and `CreationRecoveryView`
  cases in `WorkModelTests`, `WorkNavigationTests`, `DesktopHeadlessTests`,
  `WorkCLISubscriptionTests`, `DashboardWorkspaceTests`, `SessionControlsTests`,
  `SessionsStoreTests`, `WorkDestinationTests`. Their behavior (abandon hides
  first, failure restores, Undo stops shipping) moves to the new tests.

## Forbidden outcomes

- A second list evaluator: the client never computes membership beyond the
  table above. No `WorkMembership`, `ClientWorkDatabase`, outbox or request
  rows from LOO-906.
- Dropping a patch by comparing values or on a timer.
- Two Notice types, or a surface that keeps its own error string beside the
  Notice.
- An on-disk queue of commands.
- Two reading wrappers: `WorkReading` and `Reading` must not coexist.

## Slices

All in one PR; deepest first.

1. **Machine** (This slice): after-reply list and deferred `sync_after_save`;
   `Revisions` map; `revisions` on responses and errors; `read_at` on frames;
   `request_id`, `command_answers` migration
   (`uv run python scripts/new_migration.py command_answers`), protocol 7;
   DTO fixtures `response.json`, `reading.json`, a `request` with
   `request_id`, an error with `revisions`. Focused tests: a held
   `sync_after_save` (stand-in planning remote that blocks) does not delay
   `task.complete`'s answer; a repeated request ID returns the first answer
   without running twice; a mismatched fingerprint is invalid.
2. **Shared Swift**: `Reading`, `SavedReadings`, `PendingChanges`, `Notice`,
   link state, decoders for the new fields. Unit tests on `PendingChanges`
   with literal readings: hidden until a covering reading, kept through a
   stale one, removed on refusal, kept on partial failure.
3. **Desktop**: cut over and delete the list above. Held-reply tests (LOO-906
   `ClientWorkQueryTests` style): with the write held, the row is gone, the
   next Task is selected and the Wave count dropped; releasing with a refusal
   restores the row and posts the Notice.
4. **Phone**: actions, `TaskFeed` on `Reading`/`SavedReadings`, Notice.
5. **Journey**: `Perf` finishes Complete → row gone at the patched frame and
   records accepted and confirmed (covering reading) times;
   `timings.py` prints all three. Recorded on the local Machine and a remote
   one.

## Acceptance at gate (headless)

- `cargo test -p loopflow --lib server::` — held planning sync, request-ID
  repeat and fingerprint tests pass.
- `cargo test -p loopflow --test integration dto_fixtures::` and
  `swift test --package-path swift --filter DTOFixtureTests` — new fixtures
  round-trip.
- `swift test --package-path swift --filter 'PendingChangesTests|WorkModelTests|WorkNavigationTests'`
  — held-reply Complete, Abandon, Move, Claim, Unassign, Reopen, rename
  change the shown rows and counts before the reply; refusal restores and
  posts one Notice.
- `uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests/TaskFeedTests`
  (new file) — phone Complete hides the row before the reply.
- `rg -n 'ShipNoticeToast|dashboardError|CreationRecoveryView|WavePlanningNotice|taskRunDrafts|abandoningTaskIDs|performTaskAction' swift` — no matches.
- Live journey (demo, not gate): `scripts/benchmarks/desktop-performance/timings.py`
  shows Complete → row gone for local and remote Machine.

## Done when

On Desktop and phone, Complete and Abandon remove the row in the click and
move selection to the next Task; Move, Claim, Unassign, Reopen and rename show
in the click; counts follow. A refused change comes back with one Notice.
Start and Auto-Ship say "requested". The five old surfaces and inline load
failures are gone. Wave click selects in the click. The journey is in the
journal for both Machines.

## Wild failure, six months on

- Patches linger because a part never re-reads after the command (its body
  unchanged): prevented by the checkpoint after each answer, which forces a
  frame. A test covers a command that changes nothing.
- The section rule drifts from Rust's `TaskSection`: the reading corrects it,
  but a visible flicker would mean the rule is wrong. Keep the rule to the
  table; anything subtler waits for the reading.
- Notice spam from load failures on a flapping Machine: transitions only,
  keyed by part, and away is one condition Notice instead of N failures.
