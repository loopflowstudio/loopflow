# LOO-672 — Ready Task conversations

Accepted design, reconciled November 4, 2026 against the working tree above
`3ef57e8573`, recorded upstream through `5519340b41`, and fresh LOO-717/498 Task status. Jack Heart approved the revised November 4 design: “ok, proceed.”
Implementation and review publication are authorized through interactive demo;
shipping, merging and installation are not. The pre-infrastructure plan is
superseded and preserved at
`a32c385044:scratch/research/loo-672-pre-infrastructure-plan.md`.

## Accepted outcome and constraints

Task conversations open without a Start or Open click after launch, selection and
restart. Jack Heart clarified during the November 4 re-demo: “but the conversation
should be auto stated and opened” and “thats teh whole pointof this task”. For
working Tasks (idle, active, moving), automatic preparation creates the primary
Session if absent and starts the native conversation if it has no history, or
resumes existing history. The previous explicit-Start rule is superseded.
Recent unfinished Tasks stay warm for fast input; returning lands at the bottom.
Jack selected that memory tradeoff on November 4. Completed Tasks release on
departure through LOO-717. Close removes warm intent; refresh cannot undo Close,
but a deliberate later visit may reopen the conversation.

Jack selected takeover without Fork on November 4, preserving the last-screen
and explicit **Take back** interaction. Selection and polling never transfer
another client's input. The overlay explains the holder using Rust's labels and
prepared command; without a saved screen it shows the explanation alone. The
independent CLI fork remains available on the same Task and checkout.

Viewing, prefetch and restore never start a Task Graph or replay interrupted or
captured input. Creating the initial native conversation for a working Task is
now explicitly within the automatic-open requirement. Keep this distinct from
automatically executing Task work. Do not expand prefetch to backlog: unstarted
Tasks and unassigned started Tasks. Draft assumption, not Jack's decision: automatic
startup requires a Task owner (`questions.md`). Missing account or Machine evidence stays an actionable failure; the
selected Task must not fall back to a different checkout or Machine.

The selected Task opens first after relaunch, followed by the bounded saved warm
set where safe. Saved layout and status are display evidence, not retention or
readiness. Cold return paints existing history while reconnecting the surviving
owner or resuming the same provider thread; connecting and failure remain visible.

Jack Heart added on November 4: “make sure to prefetch working tasks on load”
(source: `loo-672-load-prefetch.md`). Fresh repository state discovers idle, active
and moving Tasks, including those absent from saved recent history. The selected
Task leads, then visible tiles, then saved recent working Tasks, then other working
Tasks by their shared update time; other saved unfinished Tasks fill remaining space. Prefetch
uses the existing reveal queue and location validation, preserves Close, and
never waits before rendering. Eligible working Tasks with no Session or no
provider history are automatically initialized and opened within the same bounds.

The initial engineering limits are eight recent unfinished Task conversations
across repository windows and two concurrent preparations. Visible panes remain
retained, with exceptions reported. These are provisional values, not a measured
memory budget or numbers chosen by Jack. No NSView is shared between windows.
Eviction releases Desktop's viewer and hold; protected work and other hosts may
keep the provider running. Shells retain their independent lifecycle.

## Implementation and remaining work — November 4

Owned working Task load, selection and prefetch now choose the existing
non-replacing Start intent. The same registry queue, location observation and
Session store prepare both absent Sessions and metadata-only Sessions. Retry carries
the workspace's intent; only **Try again** on a consumed first launch sends `.resume`.
Other Work still reveals history.
`RoadmapTask.conversationIntent` excludes unstarted, unassigned, shipping and
terminal Tasks from initial conversation creation, and load prefetch discovers only
Tasks it would start. A Session that reveal left unavailable opens automatically
once its Task's intent becomes Start. Hidden warm panes mount the same terminal
view, so `.prepared` launches the provider off-screen; only a rendered run proves it.

Rust retains primary-scope admission and per-Session launch locks; only Take back
replaces an input holder. Non-replacing startup refuses to replay a consumed
capture when no native history survived; **Try again** and explicit CLI resume are
its recovery. A Task primary's opening message makes its first turn a reading pass.

### Delete — do not maintain

- Removed history-only reveal as the automatic policy for working Task load,
  linked selection, prefetch and Retry. Retain reveal for other Work and the CLI's
  explicit `--reveal-existing` contract.
- Replaced `missingTaskConversationStartsExplicitly` and the sidebar refusal
  assertion with automatic preparation of both absence cases. Preserve location,
  no-provider-during-preparation and identity assertions on the surviving path.
- Removed old Start-click usage guidance and contradictory implementation history
  from this plan. Detailed prior designs, screenshot repairs and check evidence:
  `3ef57e857394d5a97271ea00b6048ad60c22bf9a:scratch/loo-672-ready-conversations.md`.
- Renamed the warm queue to `openWarmConversations`/`openings`: it opens with each
  workspace's intent, so `reveal` names only the history-only intent.
  `RoadmapTask.conversationIntent` is the one working-Task policy.
- Preserve explicit Start as recovery outside eligible working Task navigation;
  preserve Rust's Start/Reveal/Take back intents, Close and late-result fences,
  selected-first scheduling, input claims and LOO-717 retention. No new launch,
  migration, retirement, provider quota or snapshot owner is introduced.

### Concept review — November 4: implemented

Reviewed the dirty tree above `3ef57e8573` against Jack Heart's corrected requirement.
Jack: “launching a Task Graph or replaying old Task input through navigation is not”
authorized, and an idle Task with a stopped run was the counterexample.

1. A Task primary's first turn without a request is a reading pass: `task_session.md`
   says so, `TASK_PRIMARY_MESSAGE` opens it, and `first_turn` selects it by primary
   scope. Repository and Wave primaries keep their prior text.
2. Only a refused reveal fetches `taskConversationStart`; a failed Start clears any
   offer. The pane shows `conversationError` first, with Retry on the workspace intent.
   Explicit Start stays for Work that opening cannot start.
3. The opening intent is optional: nil follows `task.conversationIntent`, a value is
   honored and recorded on the workspace.

Open warm conversations on moving Tasks also receive settled Graph progress as provider
turns, so turn cost continues after load. Measure that with the first turns.

### Concept review — November 4, second pass: implemented

1. **Queued Tasks show opening.** `WorkModel.conversationIsOpening` renders
   "Opening conversation…" in Task panes and Dashboard tiles for an eligible working Task
   waiting behind both slots or the Sessions read. Close, a conversation failure, a Start
   offer, a failed read, saved-work display or a missing location keep the existing pane.
2. **Steers reach the reading pass as recorded direction.** `TASK_PRIMARY_MESSAGE` and
   `task_session.md` say to report them and act when the person asks there.
3. **Folder trust cause and repair.** `restore_task_checkout` ran its own
   `git worktree add --force`, and only `worktree_add_inheriting` wrote `loopflow-created`,
   so no Task checkout carried the marker since #1648. It now calls `mark_lf_created`.
   `UseExistingWorktree` adopts checkouts, so existing `.git` directories are not backfilled.
   Unmarked checkouts keep Claude's dialog; the launcher records it as `pending_input`, so
   the Session reads Available before any hook can run. No wire field was added.
4. `first_turn`'s comment names seeded continuations.

### Realign — November 4: refused reveal repaired

Compress found that a Task with no Session kept its refused reveal's error and Start offer
after it became working, which meant one Start click. `WorkModel.openWarmConversations` now
clears both when the intent moves from reveal to Start, so one automatic Start follows; a
Start failure then stays visible with Retry. `claimedTaskLeavesRefusedReveal` fails without
the repair. Third pass item 3 covers a reveal already in flight, through the queue and
through a link.

### Concept review — November 4, third pass: implemented

Two surfaces contradicted “no Start or Open click”; all three items followed from accepted
decisions, so none needed a new choice from Jack.

1. **Visible tiles join the queue.** Candidates and queue keys are selected, then visible Tasks,
   then recent, still under two preparations; `presentView` and `presentWaveTiles` run the queue
   after presenting. A visible tile with no Session and reveal intent stays out: it has nothing to
   reveal and keeps its Open button. `.pending` panes show the queued spinner too.
2. **A consumed first launch recovers from the pane.** `session_actions` learns `launch_consumed`
   and offers `MoveHere` labelled **Try again** in place of Open, so no wire kind or field is
   added. `launchRetry` (MoveHere without Open) and `heldElsewhere` replace Swift's bare MoveHere
   checks, because MoveHere alone used to mean a holder. Sessions reconcile straight to `.failed`,
   so refresh prepares nothing and only the click sends `.resume`; Retry beside Take back stays on
   the workspace intent. `open_without_replacing` keeps its refusal. A prefetched workspace still
   validates its location once, so the retried conversation is retained.
3. **A superseded refusal does not land.** A failed opening whose intent moved records neither
   error nor offer, and the queue's `defer` retries with the current intent. `openLinkedTask`
   follows the same rule: it ends the link's opening and runs the queue, so a link opened during
   a claim costs no click either. `supersededLinkRefusalDoesNotLand` fails without this.

### Concept review — November 4, fourth pass: no change

Reviewed the same tree by reading code, not running it; nothing changes and no accepted decision
is unimplemented. Counterexamples that held: a Task evicted from warmth stays out, because `prefetch`
skips existing workspaces, so two repository windows cannot refill each other; a failed Machine read
shows the window's error, not a spinner; Start always returns a launch command, so the queue cannot
spin on `.pending`; every `moveHere` reader separates Try again from Take back; `capture_is_prepared`
and the listing's marker read agree when the capture is missing. One consequence goes to demo as an
assumption in `questions.md`: opened conversations count as Available.

### Verification and review still required

Real Claude opened an lf-created Task checkout in a pty with no prompt.
`prepare-redemo.py` ran to completion on fresh data: all four Tasks idle and owned, every
checkout carrying the created marker, the run stopped, no live process. Its comment is
recorded; that the conversation reports it is left to demo, as are rendered hidden-pane
dialog reporting and its clearing after an answer.

Focused checks cover automatic working Task preparation for absent and prepared
Sessions, identity across refresh/selection, bounded fresh-load prefetch excluding
unassigned Tasks, unavailable-to-Start transition, visible tiles past the warm limit,
superseded refusals through the queue and a link, Close, elsewhere ownership,
consumed-input refusal and its **Try again**. Gate owns affected suites and
static analysis after implementation. Direct consumers reached: WorkModel linked
opening, SessionsWorkspaceRegistry, SessionsStore and Task panes; Rust
`human_session`, prepared-command transport and primary initialization.

The configured Claude/Codex demo must start with fresh disposable data and no
manual Start clicks. Verify load prefetch, selection, relaunch and stable native
identity, then exact draft continuity, cold return and explicit Rex Take back.
Include an idle Task with a stopped run and an earlier Task comment asking to start it:
after load its conversation reports status and that comment, and `lf ps` shows no new Graph.
Measure first paint separately from usable input and Desktop + lf + provider
private footprint with twenty visited Tasks; list visible exceptions and protected
providers. Eight warm conversations remains provisional.

Each automatic start
sends the primary first turn, so a load can spend up to eight provider turns; the
original "no new agent turn" acceptance holds only for existing history. Measure
this with the footprint.

Warm slots are not filtered by person: a teammate's owned Task on an unreachable
Machine fails quietly and occupies one. Desktop has no local person identity to
compare. Acceptable while Jack is the only owner; revisit before shared repositories.

Headless model/CLI checks are an equivalent for preparation, not native
rendering/input acceptance. The caller's Graph owns gate, candidate regeneration,
publication to existing PR #1816 and interactive demo. No main-environment
installation, merge or shipping. PR #1816's description must drop any claim of a Start
click, and its head (`0acc950924`) lacks this tree. 

`ae3c91abee` (#1818) reaches this branch only through the parent. It changes `attaches`,
not `open_without_replacing`: a retained owner now attaches even when it does not answer.
`open_without_replacing` reads `answering` directly, so at that sync confirm a retained,
unresponsive owner with no native history refuses rather than counting as a first thread.
`86fea8578f` (#1822) extends `attaches` for `NoOpenSession` owners and also leaves
`open_without_replacing` unchanged. `c59cfcc631` (#1807) adds `lf task create --worktree`;
those adopted checkouts stay unmarked, which matches the trust assumption in `questions.md`.
`a5000ef889` (#1805) makes `config.agent()` fallible beside `conversation_launch_args`.
`2cce79a27b` (#1780) deletes `regressedIn` beside `conversationIntent` and edits `ops/task.rs`;
`5519340b41` (#1824) is install and migration preflight. Neither changes opening policy.
A trial merge of committed HEAD with origin/main (`5519340b41`) conflicts in `bin/lf.rs`, `transport.rs`,
`transport/lifetime.rs` and `rust/loopflow/tests/README.md`; this tree's uncommitted changes were not in that trial.

Proposed Wave memory, not written here: replace "existing-history reveal" and
"navigation/Retry reveal" in the LOO-672 notes with automatic Start for owned working Tasks,
and record **Try again** as the one-click recovery of a consumed first launch.

### Preserved evidence and ownership

The parent LOO-717/#1811 integrated at `b81df60579` owns lifecycle, host-exit
recovery and per-process retention evidence. LOO-498 owns Rex integration.
Completed infrastructure 708/709/710/712/720 provides thin viewers, protected
retirement, VT history and separate appearance/input ownership. LOO-672 does not
reimplement it. Product GOAL and MEMORY are read-only for this step.

The prior screenshot demonstrated an absent Session stuck at Open/Retry and a
local Machine routed through remote lookup. Both were corrected before this pass.
A later launch-intent repair stopped Start from silently acquiring Take back
semantics; contention tests execute the full prepared argv, including env wrappers.
Earlier tests passed through `0acc950924`; their native/manual Start evidence does
not meet Jack Heart's corrected automatic-opening requirement. Failed screenshots
and logs remain in `loo-672-demo.md` and `demo-evidence/`; full prior verification
and contrary evidence remain at the precise Git reference above.

Verification found one production gap on November 4: a Session frame read before
`confirmMachine` was stored under the unconfirmed Machine key and then lost, so a
quiet fresh repository never opened its working Tasks. `confirmMachine` now replays
Sessions. Saved shipping and unstarted Tasks keep history-only reveal.

Reconciliation found three more on November 4, all repaired: an unassigned working
Task would have launched a hidden conversation behind its Graph Start offer; a
Session left unavailable by reveal stayed stuck after its Task became working; and
a preparation reply arriving behind its Session frame took focus back from the pane
the person had moved to. The last surfaced as an intermittent
`taskLifecycleUpdatesOpenSidebar` failure and now has a deterministic test that fails
without the fix. `WorkCacheTests` failed 2 of 6 runs on the mock's arrival order of
two concurrent reads; `boundedSelectedFirstPreparation` proves admission order, so
that assertion now checks the prepared set.

Checks, November 4, through realign: `cargo test -p loopflow --lib ops::human_session::` 37 passed (compress; Rust code unchanged since); `scripts/test_desktop.sh --filter 'WorkCacheTests|WorkDestinationTests'` 89 passed after the link repair, and `WorkspacePanesTests|SessionsStoreTests|WorkspaceRetentionTests|WorkNavigationTests|DesktopPaneControlTests|DTOFixtureTests|WorkCLISubscriptionTests` passed at compress; clippy, gate, candidate and rendered native proof (idle Task, stopped run, no new Graph) deferred to the Graph.

Checks before publication, November 4: see `loo-672-demo.md`, “Current candidate: automatic opening”. Clippy and the 714-test Desktop suite passed; `scripts/test_desktop.sh` crashes in `ghostty_config_finalize` at some environment sizes, independent of this change. Gate and rendered native proof remain.
