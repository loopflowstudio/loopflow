# LOO-906: open the new conversation immediately

## Problem and intended experience

Task direction: “launching a new task should get to new session instantaneously”.

Jack Heart, in the scope clarification delivered through comment
`ad6ba5e7-bbb2-4973-951e-a022622fcf03`:
- “this is meant for deskto”
- “it takes like >10 seconds and there are a lot of intermediate states”

The clarification describes a LOO-904 screenshot containing “Reading workflow...”,
“Restoring workspace”, “Not started” with Start, and “Conversation change unconfi”.
The screenshot itself was not available in this checkout. These are reported
observations, not a measured timing trace.

Desktop's New Task Start should dismiss the sheet and enter the conversation
surface in the same main-actor turn, before any asynchronous creation work.
The submitted words stay visible there while preparation runs. One quiet
“Starting conversation…” status becomes the actual attached Session in place.
There is no intervening Task details page, workflow-loading screen, workspace
restoration screen, or second Start action.

Target: Start → conversation surface P95 under 100 ms in a local native sample
of at least 20 starts. Headless correctness requires the presentation to change
before a deliberately held creation response is released. Surface presentation
and native input readiness are separate measurements: this Task promises immediate
entry, not that Git, network or provider startup finishes in 100 ms. Record both.
Baseline: reported >10 seconds; no numeric distribution measured yet.

## Demo and done when

Run `uv run python scripts/loopflow-dev.py run`, press ⌘N, enter a request,
and press ⌘Return. The sheet disappears immediately into its conversation.
Keep creation, layout and workflow responses held in the headless fixture:
the submitted request and one starting status remain visible. Release them;
the same destination shows the Task's admitted primary Session with that exact
request as its first turn. No duplicate Task, Session or Graph is created.

Done when that journey works on success, storage failure, uncertain submission,
attachment failure, and navigation away; the four reported intermediate messages
do not flash during a healthy launch. A real unresolved recovery from another
operation remains visible. Existing unstarted Tasks still offer Start.

## Current architecture and constraints

- `NewTaskSheet.create()` (`swift/LoopflowMac/Views/NewTaskSheet.swift`) holds
  its sheet with “Starting…” while awaiting `WorkModel.createTask`.
- `WorkModel.createTask` (`swift/LoopflowMac/WorkModel.swift`) only dismisses
  after `RegistryQuery.createTask` returns, then calls `openTaskConversation`
  and finally `confirmWorkUpdate`. Existing tests already establish that final
  confirmation does not gate selection; moving that await alone cannot fix this.
- `RegistryQueryLocal.shared` saves immutable submissions through
  `CreationSubmissions.submit` before sending. `CreationJournal.swift` owns
  request-ID deduplication and same-Machine recovery. `record` itself awaits
  Machine identity, so even saving is too late a point for immediate navigation.
- `engine::creation::create` (`rust/loopflow/src/engine/creation.rs`) files the
  Task, ensures its primary Session and request, launches `task run`, then returns
  the admitted receipt after Graph registration. `ops::task::task_create`
  also awaits planning acquisition and sync. These are real work on the current
  synchronous receipt path; none belongs on the presentation path.
- `openTaskConversation` → `openTaskLink` → `prepareLinkedTask` can resolve
  Task planning, prepare/select the primary Session and reread Task planning
  before placing its terminal. The receipt's `session_id` is currently unused
  by `WorkModel.createTask`. Use the admitted identity when it becomes available;
  do not implement a second primary resolver. Exact Session placement already
  exists through `placeExactSession`/`installTaskConversation`.
- `SessionsView.workspacePanel` independently shows layout restoration and
  routes `awaitsStart` Tasks to `TaskStartOffer`. `TaskWorkflowBar.position`
  independently shows “Reading workflow…” or “Not started”. A fresh launch
  can receive these readings at different times; changing only sheet dismissal
  would expose more of these states.
- `CreationRecovery.refresh` (`CreationSubmissions.swift`) presents pending
  immutable submissions; `CreationPresentation` removes them only after a result
  mounts. Do not retire a submission when the starting surface mounts.
- “Conversation change unconfirmed” comes from the Session-mutation branch in
  `store/sqlite/creation.rs`, not Task launch. The reported screenshot does not
  establish whether it belonged to this launch or a separate operation. Never
  globally suppress recovery to make the launch look healthy.

The persisted source of truth stays `creation_receipts`, `tasks`,
`task_primary_sessions`, and `agent_sessions`; execution stays with the existing
Graph and held-terminal owners. Desktop's pending presentation describes only
which submitted creation this window is showing. It never invents a Task ID,
Session ID, workspace identity or workflow state.

## Chosen mechanism

One window-owned creation presentation, keyed by the existing request UUID,
spans submission through actual Session attachment. Keep it in `WorkNavigation`
with captured repository, Wave/graph choice and request. It is UI state, not a
new persisted Work entity or a wire DTO.

1. On Start, capture the original scope and allocate the UUID synchronously.
   Install the starting conversation presentation and dismiss the sheet before
   the first await. Prevent repeat Start for this presentation. The async action
   belongs to the model; dismissing the sheet must not cancel submission.
2. Render this presentation in `SessionsView.mainPane` before normal Work routing.
   Keep the sidebar available, the request visible and one launch status. Hide
   the prior Task's workflow bar while this destination is selected. Provide no
   fake active terminal or follow-up input queue before attachment.
3. Submit exactly once through existing `query.createTask` and
   `CreationSubmissions`; pass the captured cwd, never ambient selection after
   an await. Bind the receipt's real Task and Session IDs to this presentation.
   Prepare that exact admitted Session through the shared Session owner and use
   its returned `SessionRecord` for placement. Add a focused exact-Session query
   helper if needed using the existing resume operation; do not paginate unrelated
   Session history or select the primary again. Retain authoritative Task reads
   needed for placement, but perform them behind the conversation surface.
4. Retain the starting surface until layout and attachment are ready to display
   the terminal. Install it in the Task workspace, then transfer presentation in
   place. Gate workflow chrome on a fresh Task overview; omit unread chrome rather
   than fabricating a running Graph or flashing an actionable unstarted Task.
   Use real `ServerConnection`/terminal readiness, not socket receipt or a timer,
   to mark input ready. Do not make the conversation wait for optional files,
   workflow details or whole-repository confirmation.
5. Refresh planning in the background. A launch request token and existing
   navigation/destination generations ensure late completion settles its original
   request without stealing focus, reopening a dismissed surface, or replacing
   another window's newer creation. Navigation away does not cancel a submitted
   effect. Returning can show its existing recovery/result.
6. Reconcile by the original request ID on uncertainty. While this window is
   actively presenting that request, show its status/error inline, without a
   second recovery banner for the same ID. Other unresolved requests keep their
   recovery UI. Retire only when the real result mounts via `CreationPresentation`.

No backend asynchronous queue or receipt-contract change is needed for immediate
entry. Keep CLI/iOS behavior and Graph registration semantics unchanged.

## Failure and departure behavior

- Empty request: Start stays disabled, as today.
- Failure before saving: starting surface shows the original editable request
  and error, with an action to return to the existing New Task editor. Nothing
  was sent; a new submission is safe. Do not restore an old sheet over newer UI.
- Submitted but uncertain: retain the exact saved payload and UUID. Inline
  recovery uses server disposition; no second submit button and no invented
  absence when lookup fails. Restart uses the existing submission journal.
- Task filed but launch/attachment failed: retain its real identity and show
  the error with Retry opening/Inspect as permitted by the existing owner.
  Retry never recreates the Task or repeats the initial request.
- Missing provider, unavailable Machine, deleted Task/Session, or input owned
  elsewhere: show the authoritative explanation in the same destination.
- Leaving, switching repositories, opening a second sheet or closing the window:
  settle the original submission through existing recovery; no late focus grab.
  Closing before save has completed must not be described as a saved submission.

## Alternatives and risks resolved

- **Only accelerate Rust creation:** planning sync, checkout and provider startup
  remain variable. Useful future profiling, but cannot guarantee immediate entry
  and does not remove independent UI states. Excluded here.
- **Return a filed receipt early and launch later:** changes CLI, wire semantics
  and execution ownership without being necessary for this Desktop outcome.
- **Optimistic Task/Session DTOs:** rejected. They create false identities and
  make recovery and workspace placement harder.
- **Dismiss the sheet and immediately select an ordinary Task:** impossible
  before the Task exists, and later races `awaitsStart`/layout/workflow readers.
  A request-scoped presentation is required until the real destination is ready.

Wild success: focus enters the conversation once; the request stays visible,
preparation is quiet, and normal provider output takes over without another click.
Wild failure: a cosmetic loading screen hides failed execution forever, or a
late completion steals navigation. Prevent these with inline errors, two distinct
presentation/readiness timings, and departure tests with held responses.

## This slice: complete Desktop presentation cutover

One coherent change: introduce request-scoped presentation; replace sheet-owned
waiting and reply-owned navigation; render the conversation before submission;
carry it through exact Session attachment and recovery; remove launch-time
routing through unstarted/restoration/workflow placeholders. Keep the existing
normal Work routing for ordinary Task entry.

Delete—do not maintain: NewTaskSheet's long-lived `creating`/“Starting…” wait
as launch progress, and `WorkModel.createTask`'s receipt-time palette dismissal
and generic `openTaskConversation(receipt.task.id)` handoff. Replace tests that
require the sheet to remain on pre-save failure with checks for editable inline
failure and return-to-editor. Retain and extend departure, duplicate-submission,
recovery, ordinary unstarted Task, layout persistence and Session ownership tests.
Do not delete `TaskStartOffer` or restoration globally: existing Tasks use them.
Also remove the receipt-only palette generation counter and exact-Session history
pagination. Exact links prepare their named Session through the existing resume
operation and adopt that returned record.

Forbidden: fake Session records, another submission journal, a second primary
resolver, duplicated task.create + task.run calls, elapsed-time readiness,
global recovery suppression, or consuming native input before attachment.

Update `swift/README.md`'s New Task behavior and existing performance docs with
Start-to-surface and Start-to-input-ready endpoints. Extend `Perf`'s existing
agent-input trace using the request UUID; do not log prompt contents.

## Shipping docs

The implementation uses the existing exact-Session attachment owner behind the
starting surface. Native latency measurement remains for demo. A request's saved
submission retires only after the attached result mounts. A window-local set of
created Task IDs prevents stale unstarted planning from restoring a Start offer;
workflow chrome waits for a graph reading. Read these files from this checkout:

- `swift/README.md` — New Task actions, immediate entry, input readiness and recovery.
- `docs/server.md` — starting-view recovery, save failure and result presentation.
- `scripts/benchmarks/desktop-performance/agent-input.md` — separate surface and
  input-ready endpoints, native sampling and the proposed target.

No new open product question arose while writing. The 100 ms target remains a
proposal as recorded above, not a measured result or quoted requirement.

- `lf open --host desktop --file swift/README.md --file docs/server.md --file scripts/benchmarks/desktop-performance/agent-input.md`
  — exited 0 on 2026-11-14 and reported opening all three in Loopflow Desktop.
- `git diff --check` — passed; prose-only changes, no build or tests needed at this step.

## Gate checks (headless)

Extend existing `NewTaskTests` and `WorkDestinationTests`, using controlled
barriers rather than sleeps or wall-clock assertions:

- Hold creation before any reply. Assert sheet dismissed and starting
  conversation visible, with submitted words and no workflow/restoration/Start
  UI. Release receipt, hold Task/layout/Session reads in different orders, then
  release attachment. Assert the admitted Session is installed, the request
  appears once, and recovery retires only after the actual result mounts.
- Exercise save failure, lost answer, attachment failure/elsewhere, two windows,
  navigation away/round trip and a newer sheet. Check original request IDs,
  unchanged newer selection and no duplicate creation. Use production leaf views
  for headless inspection of chrome; mounted terminal rendering stays in display
  suites and is not a headless gate dependency.
- Run `cd swift && swift test --filter 'NewTaskTests|WorkDestinationTests|CreationSubmissionTests|WorkNavigationTests'`;
  expected: immediate presentation barrier and existing navigation/recovery tests pass.
- Run `cd swift && swift build`; expected: Desktop compiles.

No Rust changes are planned. If exact attachment requires one, retain the
existing creation receipt contract and run the named creation-first-turn test
in `task_graph_launch_tests` with inherited LF_* unset and stdin closed, plus
`cargo fmt` and `cargo clippy --all-targets -- -D warnings`.

Native performance sampling belongs to demo on a display host: record at least
20 starts with the existing performance recorder and new trace phases; report
surface P95 against 100 ms and input-ready separately. Do not claim a measured
speedup from headless view state or build completion.

## Verification and remaining work

- `swift test --package-path swift --jobs 4 --filter 'NewTaskTests|WorkDestinationTests|CreationSubmissionTests|WorkNavigationTests'` — app built; 134 tests passed. Fixtures were corrected to intercept only creation, supply a Machine for native readiness, and prepare named Sessions instead of scanning history.
- `swift test --package-path swift --jobs 4 --filter 'WorkDestinationTests.newTaskSelectsTheTaskAndOpensItsConversation'` — passed both cases: native readiness remains covered until layout returns, including attachment failure and retry.
- `git diff --check` — passed.
- Native sampling remains for demo: at least 20 starts; surface P95 and input-ready
  timing separately. No native speedup has been measured here.
- Gate owns the broader suites, including documented command examples.

## Concept review — 2026-11-14

The shared interaction rule is immediate local presentation, followed by the
existing command owner and reconciliation. Starting a conversation, admitting a
creation and attaching native input are different facts; people need one place
that progresses through them, not a page for each fact. The request UUID joins
presentation to the saved submission; the admitted Task/Session IDs replace no
invented identity. Keep the current journal, receipt contract and execution owner.
No backend queue or new persisted lifecycle is implied.

### Next implement direction (within the launch design)

- Bound the launch override to reconciliation. `createdTaskIDs` currently gives
  every Task created in this window an indefinite exception to ordinary Start
  routing and workflow chrome. Replace that historical membership with pending
  reconciliation for the admitted creation. Clear the exception after a current
  post-admission Task reading catches up, using the existing checkpoint/scope
  ordering; a cached or unrelated reading cannot clear it. Attachment readiness
  alone cannot clear a stale-planning exception. Once reconciled, later legitimate
  unstarted/no-Graph readings must use ordinary Task behavior. While observation
  is unavailable, show its explanation rather than silently suppressing workflow
  forever. Do not require a Graph to exist as the only way to finish reconciliation.
- Creation recovery and attachment failure describe different facts. A matching
  recovery item must not hide `launch.error` or its Retry opening action after
  an admitted Session fails to attach. Present the original request once, one
  current attachment status/action, and the receipt's recovery action when needed.
  Deduplicate identical text; never deduplicate away the action that can recover.
  Retrying attachment uses the admitted Session, never a new creation UUID.
- Leave ordinary Task routing and other requests' recovery intact. No change to
  CLI behavior or builtin skill instructions is needed for this Desktop action.

Counterexamples for implementation: an admitted Session becomes usable before a
fresh Task frame; a Graph ends or is legitimately absent after reconciliation;
an admitted receipt is rediscovered by the recovery poll while attachment is
failed; navigation changes before either reading returns. The first retains
launch presentation rules until observation catches up; the second uses normal
Task UI; the third keeps Retry opening available; the fourth never steals focus.
Storage failure retains the words for Edit request; uncertain submission retains
its saved UUID for lookup. Neither failure becomes an invitation to create twice.

### Completion: stated direction and open product choice

Jack Heart:
- “another thing i will add is that completing a task should make it disappear immediately”
- “can we talk about what are the fundamental causes of so many intermediate states and non-immediate reactivity?”
- “is it about slow sql? insufficient reactive UI archtiecture?”

The immediate-disappearance direction is not implemented by this patch. Completion
currently goes through command completion and `confirmWorkUpdate` before its normal
projection changes. Abandonment already applies local row removal before cleanup.
This supports a sequencing diagnosis; it does not establish SQL cost, subscription
latency or provider startup cost. Those need separate timings. The 100 ms launch
surface target and native input-ready timing remain unmeasured here.

Proposed completion usage text (unbuilt): “Complete removes the Task from the
working list immediately while the change is saved. If saving fails, the Task
returns with the error. If saving succeeds but refresh is unavailable, the update
remains pending confirmation.” The local removal is presentation, not a fabricated
completed Task DTO. Keep the existing completion API and its Graph/Process semantics;
a failed refresh after acceptance must not be treated as a rejected command.
This is a design proposal for subsequent implementation, not added delivery scope.

Open product question: when completing the selected Task, should its conversation
stay open while its row leaves the working list, or should focus move immediately
to the next Task (the Wave/repository when none remains)? This changes whether
completion interrupts an active conversation. The quoted direction settles row
removal, not this navigation choice. On command failure, restore the row and show
the error without taking focus back from a later selection in either design.

Show next: `scratch/concept-review.html`. Headless review outcome for loop-or-next:
**open product choice** — selected-conversation behavior on completion, as above.
The launch reconciliation/error changes are ready as implement direction; this
review changes no production code and starts no execution. Native launch samples
remain with demo. Prior test results above are historical; this review reruns only
`git diff --check`, which passed before these notes were written.
