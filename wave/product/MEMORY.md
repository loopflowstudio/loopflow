# product wave memory

Renamed from `concerto` in the 2026-07-08 wave/project/task restructure. The wave's
scope widened past the Mac app: product now owns the shared API and every surface
(CLI, Mac, iOS, agent turns, workers). Older “Concerto” notes mean the Mac surface.

## Task delivery boundary (LOO-418, 2026-10-07)

Jack Heart decided: a Task has zero or one PR, never a serial chain. Follow-through
is a follow-up Task, filed after landing before the source Task completes.
Dependent PRs belong to stacked Tasks and may start before their parents merge;
research can end without a PR. This supersedes older serial-chain/keep-open
directions below. Jack authorized kickoff and its design review boundary, not
landing. `scratch/make-a-task-up-to.md` is a draft, not accepted implementation.

Base `626789dcd` includes #1488: bare land already defaults to completion, while
`TaskFollowUp` keeps an obligation on the source Task. Replace that writer as
well as rotation; retain existing obligations and multi-PR history. LOO-385's
no-PR/Workflow scope overlaps; its inspected planning was unstarted, not closed.

Source inspection found Process admission before `--task` placement, while
Started and Flow membership use recorded cwd. A disposable trigger probe
reproduced the warning with caller cwd and accepted target cwd; historical
launches and the corrected public CLI remain unverified. Stack sync already
preserves child code through parent updates/squash; deliberate scratch isolation
explains missing designs. Handoff must preserve child-specific scope. Proposed
post-merge filing, waiting, due-date return and migration mechanisms await review.

## Live Home reconciliation (2026-10-05)

Jack Heart requested cleanup first (LOO-380). Desktop/CLI 0.13.3 shared one
Home; supported operations reconciled completion/cancel writebacks and stopped
Flows. Full receipts and recency caveat: `7c3072d64:wave/product/MEMORY.md`.
Unresolved then, not rechecked: LOO-367's handoff, Intelligence's backlog
Project (LOO-366), test Waves, LOO-343's dead claim; no rendered proof.
This branch preserves the sidebar fix; parent #1439 has landed.

## Reactive workspace (2026-10-05)

Jack Heart requested that the open workspace show committed changes without a
refresh (LOO-382). In design review he decided: one landing; Desktop reacts to
the store only, with Linear arriving through sync (he prefers webhooks for
scheduling it, unowned); latency budgets are targets pursued for about an hour,
and a miss ships with numbers. Later that day: no follow-up Tasks.

- Polling was the largest writer in the store: about 63,000 of 65,043 daily
  Exec rows were Desktop's own reads, and that volume made the reads slow.
- A domain follows what a write changes on screen, not its table. Transcript
  lines were 164,192 of 168,336 daily Session events; counting them would
  re-read everything for ten hours a day.
- A reading restates when it was taken in several fields. Comparing bytes
  without removing them sends a frame for every reading.
- Git facts are not commits. Jack (2026-10-05): show them within seconds,
  by watching, not a tighter clock. A linked worktree commits without
  touching a file in its checkout: watch its Git metadata directory too.
- A receipt key does not say what a row is. Provider attempts share
  `:events.jsonl:` with transcript lines; exempting the key silenced about
  1,270 displayed rows a day. Exempt the types readers skip. Usage moves
  its own revision, which only Wave detail reads, at most every 10 s.
- An unreadable store must not read empty.
- Merged October 6 (#1452): one `lf monitor work --watch` per window (renamed by #1463)
  replaces every timer loop. Reader on a copy of Jack's store: commit to
  frame 0.3 s for a Task, 0.13 s for a Session; a file written in a
  checkout about 2 s. The window was not re-measured.
- Measure before merging readers: the second reader per window, listed as
  a taboo, used 0.2% of a core and 25 MB. It stayed.
- Jack (2026-10-05): which Waves and Tasks are current must be simple and
  fast, and he wants derived state in the database. None is stored: sampling
  put 83% of the read in one per-Task statement, and reading it in bulk met
  300 ms. Sample first.
- The outline lists started Tasks only (2026-09-25). LOO-382 asked that a
  new Task appear there; Jack kept the rule for now (2026-10-05).

## Task conversation and Workflows (LOO-353, PR #1439; 2026-10-04 to 10-06)

Jack Heart approved landing on October 6 after three Desktop looks at a fixture
Task in a private Home: "good enough. approved", then "lets get this landed."
The approval covers the Task view as shown: no real provider, populated store
or live Home (LOO-391). The design, every unreviewed choice and the
demo notes are at `e67cdc62f:scratch/` (`focus-on-your-own-work.md`,
`questions.md`, `task-workflow.md`, `demo-task-workflow.md`).

**Jack's decisions, October 4.**

- One ongoing Task conversation holds design and review; headless work is
  ordinary `lf -b`. More conversations may be opened deliberately. Substantial
  implementation inside the conversation is discouraged.
- Product says Session for interactive and Run for headless; both keep
  AgentSession identity and history through mode changes.
- Waiting is the one attention state; `--waiting` replaces `--needs-me`;
  `--interactive` stays a mode filter. False positives are acceptable. Claude
  uses only binary stream-json, never SDK, hooks or permission hosts.
- Delete all Task-worker machinery, mutable Flow switching and automatic
  database-backed recovery; the caller owns recovery. Delete `lf session ready`
  and `complete` with their handshake, under no new name.
- Task membership is owning Home, resolved checkout and explicit binds,
  excluding repo/Wave scopes. Hiding or selecting never terminates or approves.
- Operational Flows hold autonomous loops and XORs only; deciders use
  `loop-or-next`.

**October 5.** Jack rejected the first cut's front door: it "should have been
DELETING the task worker APIs and routing more things through the basic (e.g.
flow -b) apis."

- `-b` prints and blocks; the caller backgrounds ("cant you just background
  with &"). `--mode` is taken back.
- `--task` and running from the Task's worktree are one path with one set of
  checks. A Task helper may place and fill defaults, then enters `lf run`: "i
  dont want to introduce parallel paths or drivers." Launch-time PR
  preparation is unjustified.
- "I hate __ and hidden arguments." A step is the plain command and "doesnt
  need to know its part of a flow"; Flow logic and tracking live in the driver.
  Every Flow is tracked alike, none primary for a Task, read-only to all but
  its driver; the only control is ending the driver.
- Two records: FlowExec, append-only per driver, and a mutable outer one owned
  by this Task. "I dont think we need the FlowSession datatype."

**October 6.**

- "lets just call TaskWorkflow Workflow"; nodes and edges, not stages. The
  graph is "fixed upfront when it's loaded and never mutated"; position is
  live state in the store. Person and agent both choose XORs and loop-backs
  through one Task command and matching Desktop buttons; no choice ends the
  conversation.
- "projects have workflows instead of default"; a Task's own choice wins.
- "no separate Task is ready state": ready at `start`, active between, done
  at `end`; the abandoned mark stays.
- "run is one conceptual attempt, exec is one lf invocation." `lf task run`
  moves the Workflow and `lf run` does not: "worth it to keep lf run simple."
- Dropping per-node steer acknowledgement was "potentially bad": it had
  stopped runaway token counts. Restored as `--steers-after`.
- The first Task sheet was "kinda yucky." Wanted and built: the Workflow graph
  in the header, one Task Session plus shells, and one **+** menu adding a
  shell, Files or the Flow exec log as panes. Monitor, the Sessions sidebar
  toggle and the Flow chip are deleted.
- Follow LOO-382: the Task page reads only the stream's `task` part.
- Deferred by Jack: Waiting for native terminals, "not the right product
  experience, but OK to defer" (LOO-384); no-PR completion and a reopened
  Task that cannot run (LOO-385); naming inner Flows (LOO-386); a clipped
  Flow exec diagram (LOO-392).

**The agent's choices, unreviewed by Jack.**

- One `lf task run` retries a failed Flow exec three times with no pause;
  exit 3 (blocked or stopped short) is not retried. `lf task move ISSUE end`
  replaces `lf task complete`; correction is `lf -b session resume ID MESSAGE`,
  three turns at most.
- The driver learns a step's Exec by polling every 10 ms; killed in that gap,
  the step goes unrecorded. A killed driver's run reads as running forever.
- `lf` allows a second `lf task run` while an edge runs; only Desktop's
  disabled Start keeps two drivers out of a checkout.
- The answer contract is message text; provider structured output is deleted.
  Any `lf` command can be a `cmd:` step. `feature` and `code` are workflows
  only, and `lf run feature` fails naming `lf task run`.
- A Task that has not run on this model has no Workflow and reads `not_ready`:
  every started unfinished Task on an existing Home, until its next run.
- An edge that runs nothing ends a Task over uncommitted changes and keeps its
  checkout; an empty unpublished PR is retired at `end` (reverses part of
  W2-151). Risk: if Linear completes an issue when its PR merges before `lf`
  settles the landing, the Task stays active and flagged until forced.
- `feature` ends at a published PR with no design-review pause; `ship` lands
  after the demo; `code` equals `pursue`. Main's `lf flow end` (#1435) is
  dropped. `lf commit -p` pushes; plain commit stays local.
- Waiting is one reading per Session, saved at most every 5 s: an unanswered
  question, or no open tool call and a hand-back or 120 s of quiet. Codex
  approvals count as questions; OpenCode permissions do not.
- `tasks.primary_session_id` and `lf session ensure --task`; Desktop asks once,
  into an empty workspace only. A lone headless skill Run is not in the Flow
  exec log (Jack's open question).
- From merging main's #1463: a Wave's selected Project gates an unstarted Task
  at `lf task run`; a rotation holding a checkout refuses a Flow launch and
  each step; a Project's `workflow:` may be empty, and its Tasks read not
  ready. `lf task restart` is not kept. `lf` still reads a Project's `flow:`
  line until rewritten.

**Lessons.**

- Settle model, store, CLI and Desktop contract before building a record. The
  first Workflow derived position from append-only rows and Jack sent it back.
- Check main's history before calling a flag missing: `-b` had been removed by
  #1356, not left unbuilt.
- Deleting a record exposes its hidden jobs. The Flow row carried a Started
  trigger, per-node steer acknowledgement, the S1–S5 note key and command
  normalization.
- Skill prose narrating a builtin Flow drifts when its YAML changes.
- `INSERT OR REPLACE` fires no update trigger; a revision rule comparing old
  and new rows needs an upsert. Waiting by quiet is no write, so the stream's
  reader keeps that clock.
- Run touched suites whole before trusting a slice: twelve slices passed
  focused tests and whole suites then failed three. SQL in strings compiles
  against a dropped column; grep every dropped name.
- When main renames types under conflicting hunks, take one side and apply the
  rename from the two trees' token sets.
- Diff a plan rewrite for dropped items: a queued cleanup slice vanished in
  `f9e74029e` with no reason recorded.

**Evidence limits.** Headless tests and a fake provider only. Unshown: a real
provider step (the JSON contract with no schema request), a
driver killed mid-turn, the migration on a populated store, Desktop Start
against real `lf`, a cron-fired Flow. September 30's workspace proof items
(provider continuation, remote owning-Home association, cross-Task focus,
symlink transitions, twenty layout actions against p95 <100 ms) are also
unshown and have no Task. Gate, October 6 (`scripts/test.py`):
Rust 2265 (one stale migration test repaired), Clippy, Desktop, website
pass with `LF_*` cleared. Four `test_checkout_refresh` tests fail on Jack's
host: branch `lf wt prune` refuses a Home lacking the draft; CI owns them.
Eleven `ops::chapter` tests fail only in `cargo test --lib`.

## Session and operate pairs (2026-10-05)

Jack Heart's decisions on [LOO-383](https://linear.app/loopflow/issue/LOO-383):
the Wave conversation is operations, and “reliably finishing stuff ive started
and isnt blocked on me” is the goal.

- Operators keep every started Task moving and leave unstarted backlog alone
  “for now at least.” A defined Flow with steps left proceeds without asking.
  A Task whose Flow ended before landing waits on Jack; how a Task changes its
  Flow is wanted and undesigned.
- Each session carries its operate procedure inline (“just make it work
  reliably”). The branch composes session file plus operate body at build time.
- `task/session` is a plain skill; a primary Task Session is “just a smaller
  wrapper around this that saves that id in a field” (LOO-364).

Lessons: the observed failure copied `next_move.owner: wave` into the reply, so
the operator reported its own queue as a handoff. A status label naming the
reader's own scope must say so in the prompt. Nothing re-invokes a
conversation. On Jack's Home that day the minute check was disabled and
Product's declared `wave/operate` cron was not installed. Before continuing
work, check every Flow and unfinished Exec against `lf ps --json`, or a
second driver starts.

Jack approved the branch in demo review and asked for the operator report
grouped as waiting on him, moving and stuck, with PR links. Leaving an unarmed
merge to the person is the branch's choice; Jack did not answer.

PR #1439 changed the contract: `lf task run` always starts a fresh Flow,
nothing continues a stopped one and reconcile never resumes. The agent's
unreviewed replacement: a Task at a node waits on Jack; a failed edge is
rerun, three more attempts.

Limits: the [review](../../docs/reviews/session-operate-prompts.md) holds
simulated walk-throughs and two read-only model runs. Installed behavior is
unshown; an open conversation keeps its old text until `lf session replace`.

## Capture and configurable New Session (2026-10-03 UTC)

Jack Heart accepted capture on October 1, then selected New Session and the
Linear-style compose row on October 2. Jack approved the searchable skill-picker
prototype and requested pursue through its human demo boundary, preserving the
existing opening prompt. New Session launches; choosing a skill only updates the
next launch and persists per repository. The default is `capture-tasks`.

Tasks express intention. Favor cohesive behavioral promises; several Tasks across
Waves/repositories are supported, never a quota. Launch scope is a revisable clue,
not a filing boundary. Capture stays in its repository/Wave conversation, files
self-contained briefs without workers, and leaves operation to owners. Wave
conversations read the same skill without changing their existing authority.
Keep design and general/Task conversations. After an uncertain write, reconcile
in its destination and retry identical input there; preserve each successful filing.

The branch uses lf-owned discovery and explicit `skill <name>` launch syntax to
avoid Flow-name collisions. Repository presentation retains `(Home, checkout)`
identity and both layouts without changing Task selection. A successful app build
proved insufficient when its configured CLI lacked the skill: configured runtime
and shipped packaging need their own evidence. Six focused post-sync tests cover
selection, scope and production-control layout/error behavior; native picker
interaction, provider/draft continuity and real cross-repository capture remain
unproven. Prototype approval supplies no native acceptance or sustained-use KR.
The accepted design and dated demo evidence are retained at
[the branch checkpoint](https://github.com/loopflowstudio/loopflow/tree/25f183992969f59b42f9765d7594638cc91652dc/scratch).

## Session working set and Waiting (2026-10-03, revised 2026-10-06)

Jack Heart requested unfinished interactive participation in ordinary API and
Desktop Session navigation (LOO-372). Task association still includes all work.
Filtered absence cannot release native surfaces or clear drafts and selection.
Explicit completion remains separate from turn completion.

Waiting can only come from a stream `lf` owns, so a native `claude` or
`opencode` terminal never shows it (LOO-384). Reading Claude's transcript is
Jack's open choice. Tests replay recorded streams, no live provider.

## Current Tasks and completion history (2026-10-02)

Jack Heart's LOO-371 direct-open work measures against isolated SQLite backups.
Own benchmark process groups; overlapping runs prove nothing. October 5, same
snapshot/`lf`, five samples, load 32–65: warm 152 ms and reopen 38 ms versus
9–10 s; one window, no Task-link sheet. Cold stays 8.4 s behind one `lf` read.
Fast OCR split `LOO- 368` and timed out a 9/9 sweep: verify the observer
before blaming the product. LOO-376 owns startup and `launch.py`;
extend it for Task links. See [the evidence](../../scripts/benchmarks/desktop-performance/20261005-task-open/README.md).

Jack Heart requested current work without obsolete duplicates, completed Tasks
hidden initially, and Show completed with 7 days, positive N days and All time
(LOO-369). Coordinator guidance limits that control to successful completions;
it is not attributed as a separate decision from Jack. Canceled/duplicate history
remains in the shared inventory and Linear. Window-local filters affect displayed
rows and their count, never Task lifecycle, Flow settlement or Session membership.

The observed Growth snapshot already contained canceled states; the Task summary
dropped them. Preserve provider state separately from successful completion and
carry actual completion time. Unknown dates belong only in All time; reopening
makes planning current despite an old timestamp. Equal-revision storage enrichment
may fill a genuinely absent completion-date key, but cannot change an observed
null or date. Snapshot acquisition and unrelated updates cannot establish recency.

Jack relayed the coordinator finding that settled Tasks with removed checkouts
still carried recovery conditions. That condition alone must not bypass history
filtering. Current planning, nonterminal runtime, unresolved pinned Flow execution
and observed unsettled files/commits remain visible; open Sessions stay reachable
independently. This agrees with the Task workspace's checkout association contract.

Jack Heart subsequently approved the inline checkbox/number prototype and
requested Desktop implementation. Completed enables the remembered range
(initially seven days); the active label edits N Days, with zero shown as All
Tasks. Enter/blur applies, Escape cancels, and invalid drafts preserve the range.
One native text surface handles display and editing to preserve glyph placement;
long integers scroll within the compact viewport. Wave identity bounds drafts.

The earlier affected gate and configured app compilation passed. The later inline
revision and compression have seven focused passing tests and SwiftPM compilation;
queue gate and native visual/focus review remain separate. Jack's Growth screenshot
shows six current Tasks without the seven duplicates. The isolated offline capture
has no successful completions or Sessions, so it cannot prove recency or native
Session continuation. Live refresh and retained-Session review remain unproven.
See [the review evidence](../../docs/reviews/task-history.md). These observations
provide no sustained-use KR or external-progress credit.

## Ask removal decision (2026-10-01)

Jack Heart requested ordinary headless failure without an escalation conversation,
reporting command, notification queue, or required handoff. The responsible Wave
operator reads existing status and logs, resolves authorized impediments, and
uses its ongoing Wave chat for necessary judgment. Taskless failure returns to
its caller without creating a Wave or conversation. Known failed work stays
stopped until explicit recovery; retry retains the Flow position and conversation.

Jack clarified “no, Task sessions stay”: Task conversations and authored human
reviews retain their feedback and completion contract. Persistent versus one-off
Task conversations remains open. Historical Ask rows become ordinary conversations
without replacing the Wave chat or granting Flow settlement authority. Older Ask
identity and caller-release notes below describe the retired model.

The local Ask removal implements this boundary with provider simulations and
store recovery coverage; full Rust and Desktop acceptance remains with gate.
A successful provider turn is not reclassified from prose alone. This records
Product’s shared Session contract, not a Task or Wave placement decision.

Converted Ask history follows ordinary conversation rules; its former kind alone
creates no attention obligation. Conversion itself never completes a Session.
Later confirmed owning-driver exits can retire unassigned, non-primary
conversations; Task/Wave conversations and Flow reviews remain open. Missing
process evidence grants neither retirement nor Flow settlement. This is source
inspection, not full Desktop acceptance.

## Task workspace and primary Sessions (updated 2026-10-01)

Jack Heart accepted a Task workspace that keeps Sessions, shells and files
together while background Flows prepare the next interactive stage. Direct
Flow reviews remain their own conversations. Jack’s October 1 correction makes
all conversations in a Task checkout Task Sessions, regardless of attention or
Flow membership. Repo and Wave Sessions retain their explicit scopes and are
excluded. Any number of Task Sessions is allowed. `session list --orphan` filters
Sessions without Task association; it provides no creation opt-out. Desktop keeps
that diagnostic inventory under Debug → Sessions, outside ordinary navigation.

Earlier primary-runtime planning selected eager repo/Wave Sessions and a Task
conversation on launch. The October 1 correction supersedes any interpretation
that a Task has only one Session or that it owns every other conversation. Repo owns onboarding and last-resort help,
including without Waves or PM; Wave combines autonomous operation and emerging
design; the Task Flow authority it once operated is deleted. Primary repo/Wave
Ctrl-C replaces the conversation. TaskSession Ctrl-C remains undecided and must
never silently restart its Flow. Ordinary reads remain read-only.

The automatic Wave-wake plan is superseded by the Ask removal above
(`f57abb655:wave/product/MEMORY.md`). Upstream `3dc89bc9a` removed the resident/listener and chat
bridge; do not restore that service or assume its journal provides recovery.

The Task workspace implements Rust-derived checkout association
and a paged directory browser independent of Project hydration and PR diff bases.
Keep `(Home, resolved checkout)` location separate from Run attribution and Flow
membership. Batch placement reads must propagate failure, preserving stale UI
inventory. Readable symlinks are read-only; refresh access before equal-revision
shortcuts so an identical-content replacement disables editing and autosave
without losing drafts. Comparison failure must leave directory browsing usable.

#1369 shipped the workspace: Home-aware retained panes, reassociation and
participation projection. Remove only confirmed absent Sessions from panes and
Undo; a repository-local inventory cannot establish absence in another
repository, and a membership move must keep the same native surface. File
visibility derives from retained preference plus zoom, without a second focus
backup. Its transport fixtures were simulated; the proof left unshown is listed
under the LOO-353 section above. The branch-status account is at
`e67cdc62f:wave/product/MEMORY.md` under this heading.

Selection is split-owned: ordinary selection preserves other splits and
Command-click changes visibility without terminating Sessions. New arrivals
preserve focus and manual layout. October 6 removed the Sessions sidebar.
Primary Sessions runtime moved to LOO-364.

Jack also accepted ordinary Projects independent of optional chapter coordination
(LOO-366), and a Task admission/completion audit (LOO-367). Task supplies purpose and
continuity to Flows, not separate execution semantics. See
[the dated review and retained design references](../../docs/reviews/task-workspace.md)
and [research](../../docs/reviews/independent-operations.md). These decisions do not
authorize an automatic Project reset, Task cancellation or claims of measured gains.

## CI watcher decisions (2026-10-01)

Jack Heart's decisions on [LOO-365](https://linear.app/loopflow/issue/LOO-365),
in the order he revised them:

- The watcher is an optional helper Desktop owns, or a command a person runs.
  "not this always on 24 7 server that we expect to always be running."
  Correctness never depends on it.
- CI repair needs no owning conversation: the clock starts one ci-fix run,
  deduplicated per PR and failing head. TaskSessions stay conversations with Jack.
- It is repo-wide and does one job. "A PR with no Task is reported, not
  repaired." Any future watcher is its own program, not a plug-in to this one.
- One command, three ways to run it: a terminal, a launchd service, and Desktop
  per open repository. A second copy never repeats a fix.
- "i would prefer this watcher service to the one minute cron." The branch
  retires the cron's repair path only; the cron still resumes Flows and settles
  merges. Deleting the cron outright remains Jack's stated preference.

`lf ci watch` implements this on the LOO-365 branch: REST polling with ETags
detects, and the existing landing check confirms and admits the repair, so the
landing lock, generation and incident reservation are the only claim. Not yet
proven against a real failing landing. Open choices the branch made without
Jack's confirmation: a standalone `lf land` PR with no Task is still repaired;
a failing Task PR nobody armed is only reported; watcher state has no Desktop
view yet (LOO-353).

## Skill reduction decisions (2026-09-28)

Jack reported uncertainty among repair and implementation entry points and
requested a smaller library. Preserve useful operations before composing
process, consistent with Intelligence's
[realign direction](../intelligence/MEMORY.md#reconciliation-and-reusable-skills-branch-evidence-2026-09-28).
Design draws out intent; kickoff turns that intent into an implementation plan
and remains the first product-Task step. Jack confirmed code stops locally,
pursue at reviewed progress, and feature after kickoff/design review, pursue,
queue and Task-completing landing. These endpoints must stay distinguishable.

Jack subsequently requested bringing debug back and making it the default in
demos and examples. Debug investigates a reported failure and fixes its cause;
unbreak prioritizes restoring the broken workflow.
This distinction is the implementation interpretation of that request, not a
new distinction explicitly stated by Jack. Neither requires clipboard input.
Explicit systemic causal investigation stays 5whys. Incident composes
unbreak → 5whys → launch-plan; launch-plan is a planning skill, with no same-named
landing Flow. Expand is removed. Reduce and polish stay optional surveys; Jack
has not selected their removal or conversion into editing passes. Research also
answers conversational codebase questions. Realign owns memory reconciliation;
PR authorship belongs to pr-message and delivery commands consume it.

The local consolidation applies
[Retire duplicate memory skills · PR #1319](https://github.com/loopflowstudio/loopflow/pull/1319)
and removes the older Task-specific and governance report pipelines. QA selects
proof from the affected behavior and repairs authorized defects; an independent
audit stays read-only. Delivery skills leave mechanics to lf and retain separate
publication, reviewer-owned merge, bare landing and Task-completing landing.
Retained launch history supports prioritizing code, queue, design, ci-fix and
review-open-work; missing history does not establish disuse or caller authorship.

Jack requested single S1–S5 skills and clarified that their sequence is VSM.
The source `vsm-operate` Flow composes five sequential agent invocations for
delivery, coordination, capacity, adaptation and identity. Each skill can
investigate and act independently; one shared pass note carries evidence forward.
Repository scope remains the default despite ambient Wave attribution, unless
the request narrows it. Whether `wave/operate` uses that sequence remains open;
its separate operation is retained. Jack's “soften, dont harden” correction
rejects a numeric cap on moves. The October 5 pairs decision above replaces
“one or two useful moves”: no action is valid only once started work is covered.
Task findings can challenge Wave purpose and Wave findings repository direction;
accepted decisions return to affected owners without acquiring another control
authority or making operation a prerequisite for independent Tasks.

The September 28 finite passes ran without chapter/Task reads: incomplete
planning evidence, not an empty backlog. Their account is at
`f9e74029e:wave/product/MEMORY.md` under this heading.

The [committed design](https://github.com/loopflowstudio/loopflow/blob/9619d803ee9765de907cc529791507e822c21b57/scratch/skill-consolidation.md)
and [evidence record](https://github.com/loopflowstudio/loopflow/blob/9619d803ee9765de907cc529791507e822c21b57/scratch/skill-consolidation-evidence.md)
preserve the source decisions, simulations and verification limits. Recorded
branch checks only: five-step VSM handoff, the incident handoff, installed
adoption and delivery lack configured proof; no KR or external progress is earned.

## Named participants and review feedback (curated 2026-09-25)

Curated from the retired [name-attribution record](https://github.com/loopflowstudio/loopflow/blob/1a691ac6a222b95c46859c9c06d162d6442950a4/.lf/name-attribution.md)
and [continuation record](https://github.com/loopflowstudio/loopflow/blob/1a691ac6a222b95c46859c9c06d162d6442950a4/.lf/directions/task-continuation.md).
These preserve detailed dated evidence; curation changes no Task ownership or
acceptance state. AGENTS.md and operating guidance already own Jack's naming rule.

- Personal `user.name`, the current participant and each request's original
  author are different facts. Repository config cannot name every caller.
  `LF_USER_NAME` carries launch participation; forwarded empty stays unknown
  instead of adopting a remote Home owner's name. Detached work starts unnamed.
  Native resume must deliver corrections and clearing to the model, not merely
  change its environment; the update grants no review approval.
- Store each text's author in its existing journal/provider record. Preserve
  unknown on replay; provider IDs own identity and names are display data.
  Publishers/editors need not be authors. No second name store, registry,
  historical backfill or output-replacement filter is needed. Chat projections
  must not fill absent historical names from the current participant.
- Capture a name only for authored text. A preference-read failure once blocked
  a bare Mac interrupt. Control inputs without text must remain independent of
  display preferences. The repaired production loopback path demonstrated that
  regression, not just a mock call.
- A failed-demo conversation is useful evidence, never a success claim.
  Runnable prototyping and Flow alternatives belong to
  [LOO-297](https://linear.app/loopflow/issue/LOO-297).
- Fixture renders do not prove tab/window interaction or native recovery.

The prior name-attribution gate passed its affected suites; full Swift package
and native-rendering proof remain absent. Fresh real CLI generations demonstrated Jack/Maya/unknown prose,
not a live Task write or the review-bearing design Flow. Native review resume
still needs anonymous → named → corrected → unknown participant acceptance on
a differently named Home, preserving historical authors and review authority.
LOO-297's attempted read failed on missing Linear credentials; local Task absence
did not establish remote absence. Any sentence repair requires a fresh authorized
issue read. No Task was filed or changed by these records.

## PR authorship and retired UX research (2026-09-25)

- PR openings must explain the user's experienced change. “Try it” is a user
  action and visible result; automated evidence belongs in Checks. Prompt,
  cached copy, renderer and refresh behavior all participate in this contract.
  The [authorship slice review](https://github.com/loopflowstudio/loopflow/blob/033e0758504390e5db9d254fed4964b0dd6da4bc/scratch/prs-and-tasks-review-slice.md)
  records local proof and an agent reader exercise, not human reader validation
  or a live handoff. The [frozen sample](https://github.com/loopflowstudio/loopflow/blob/033e0758504390e5db9d254fed4964b0dd6da4bc/scratch/prs-and-tasks-sample.json)
  and [research](https://github.com/loopflowstudio/loopflow/blob/033e0758504390e5db9d254fed4964b0dd6da4bc/scratch/prs-and-tasks-research.md)
  retain unresolved roadmap wording and PR #1276/#1277 overlap questions;
  sampled rewrites are proposals, not shipped claims.
- Chapter display switches to live state after a complete successful status read,
  including no chapter. Cached authored content is fallback for a failed read
  with a visible stale warning; history remains available independently.
- The local `ux-research` flow is retired (detail at
  `80686b43b:wave/product/MEMORY.md`). Simulated personas generate hypotheses,
  not customer evidence; enduring conclusions belong here after validation.

## Workspace redesign decisions (2026-09-26)

Jack rejected the first native build of the calmer workspace on sight: "It just
looks a lot worse than the mocks. Plus I bet we can even improve on the mocks."
The retro found the build structurally faithful to direction D and tonally
unfaithful: neutral ink where the mock was warm, a shadow halo under every
panel, chrome no mock ever had (July's `WaveLensView`, system buttons, a rose
tab strip, a blue-grey pane header, a green Complete capsule), and a sidebar
assembled from four passes with no reconciliation. LOO-291 implements the fix as
four slices: warm token ramp and surface ladder; one sidebar row component;
one Session toolbar; the seven-step type ramp on Wave and Task pages.

Decisions that outlive the branch (Jack's, verbatim where quoted):

- Light mode only for now; dark is a follow-up "when we're at a happy place".
- Serif for the repo name and page titles, including the Task title ("I'm
  kinda liking the serif task title now"). Sidebar Wave rows are sans
  ("sans-serif wins") with no glyph, no count, no dot ("nothing at all is
  even simpler"); a needs-you dot may return later.
- Rust derives Task grouping from checkout identity, including previously
  unbound Sessions; Swift consumes it without rewriting Run attribution. The
  global Session browser is diagnostic.
- One universal bind from the room, ⌘K and Task rows ("worth making the
  design and architecture simple and universal if it takes a little extra").
  Bind changes a Run's Task, never its name, panes or Flow membership.
  September 30's location grouping is independent of this attribution operation;
  it needs no bind to present a Session under its checkout's Task.
- The Session view has one toolbar: the breadcrumb bar, worktree as a quiet
  mono chip, no pane header with one pane, split/close/zoom by keybind and a
  hover trio, terminal edge-to-edge in warm charcoal. "Similar tool sets on
  both the run and the session" must be collapsed or progressively disclosed.
- The September 26 two-Flow-view baseline is replaced by October 6's Workflow
  graph in the Task header; exact captured occurrence identity still holds.
- The September 30 primary-Session direction supersedes deferring the waveless
  beginner: repository onboarding must work before any Wave or Task exists.

State-of-the-art baseline adopted: one row component per sidebar level;
state glyphs only when they mean something; a surface ladder instead of
shadows; a seven-step type ramp (Cormorant 34/26/17, Lato 13/11, mono 12);
blue means running and loop region and nothing else; only the running node
animates. Research sources and Jack's decisions are preserved in
[the branch's design history](https://github.com/loopflowstudio/loopflow/tree/be7a02db0/scratch).

Jack separated follow-ups LOO-298/299/300 (detail at `9771ff7d7`) so LOO-291 can deliver S1–S5, the recorder,
signposts and assign-on-change fixes. The accepted Run/Session tables and universal
bind are future implementation, documented in [Infrastructure memory](../infrastructure/MEMORY.md#data-model-and-performance-decisions-2026-09-26).
Chapter history becoming navigable is part of that approved model; the current
Wave → Task UI is a scope limit, not a prohibition on a Chapter object.
The [scope handoff](https://github.com/loopflowstudio/loopflow/blob/be7a02db0/scratch/deferred-work.md)
records the split. Local proof does not establish merger or Jack's acceptance.

Trace removed visual features through their state and undo model
(`536b0fd56:scratch/compress-pane-state.md`).

## Task files beside the conversation (2026-09-28)

[Read and edit task files beside the conversation · LOO-327](https://linear.app/loopflow/issue/LOO-327/read-and-edit-task-files-beside-the-conversation)
implements Jack Heart's minimal native browser. Jack approved the updated demo
and authorized queue preparation, publication and landing in the
[recorded approval](https://github.com/loopflowstudio/loopflow/blob/611ed031a031e6798652a15b5d250ba0f132a4e8/scratch/file-browser-demo-approval.md).
That supersedes earlier publication holds; approval alone does not establish
merge, Task completion or individual scenario coverage. The
[design and evidence ledger](https://github.com/loopflowstudio/loopflow/blob/611ed031a031e6798652a15b5d250ba0f132a4e8/scratch/file-browser.md)
and [demo feedback](https://github.com/loopflowstudio/loopflow/blob/611ed031a031e6798652a15b5d250ba0f132a4e8/scratch/file-browser-demo-feedback.md)
preserve the failed attempts and superseded proposals. Current usage lives in
[the Mac README](../../swift/README.md) and [CLI reference](../../docs/lf.md).

- **Task placement owns both surfaces.** The September 30 internal cut expands
  the original `scratch/` and `diff` navigator to paged immediate directories,
  with optional Changes over the same documents. It sits beside retained
  Ghostty, with the recorded PR link above only navigation
  and aligned headers. Task context shows its recorded worktree without an
  independent selector. One document per Task/path in the window's existing
  workspace registry shares draft, selection and Undo across both sources,
  File/Diff, sheets and Sessions. Browser visibility survives relaunch; unsaved
  drafts remain window-local. File browsing, read and save do not require a PR
  or hydrated Project; comparisons still require their recorded base.
- **Comparison authority stays in `lf`.** Parent means the active Task PR's
  recorded base, HEAD its resolved worktree commit. The exact SHA pins list and
  patch reads; membership is net difference, including staged/untracked changes
  and rename source paths. Read-only Diff includes the unsaved File draft without
  modifying disk or index. Local reads use recorded placement directly, without
  planning sync, Task reconciliation or Run creation.
- **The filesystem is the shared editing protocol.** Jack chose autosave by
  default, an explicit-save option, and disk winning overlapping edits without
  a conflict dialog. Preserve disjoint local edits. Incoming revisions rebase
  Undo around the surviving local draft, coalescing earlier fine-grained history
  so Undo cannot restore replaced LLM text. IME defers application and Save.
  Observe files and parents across atomic replacement/recreation; a quiet period
  coalesces writes but cannot prove an arbitrary writer has finished. Deletion,
  invalid bytes and I/O failures retain drafts and truthful unavailable/error state.
- **Atomic exchange provides recovery, not compare-and-swap.** A writer with an
  old descriptor can mutate the displaced inode after a successful revision
  check. Deleting that inode can lose the later bytes. Retain displaced files,
  submitted drafts and receipts under Git metadata; never automatically restore
  over another writer. Recovery is uncapped and inspected explicitly, outside
  ordinary content reads and typing. Preserve complete UTF-8/BOM/line endings
  within the 1 MB bound; previews never authorize Save. No arbitrary-writer
  exclusion, extended-attribute or power-loss guarantee follows from this design.
- **Measure first selection without pre-reading the target.** Jack required
  ordinary local file selection under 250 ms. Corrected review measured 54/54
  selections at 60–97 ms for 72 B, 13,594 B and 265,764 B files at 520/800-point
  widths after real startup reads. Exact bytes were checked after bitmap capture.
  These are AppKit endpoints with unflushed OS caches, not physical clicks,
  compositor frames or p95. Startup remained 4–5 seconds. Earlier 2,652 ms cold
  command and 12/18 and 18/18 failing populations remain unexplained; later passes
  do not erase them. ARM SHA acceleration reduced measured executable-hashing
  cost while retaining install authority. Priority/QoS/load controls did not
  explain the slow interval.
- **Native primitives and overall approval have separate evidence.** TextKit
  proved useful selection, Find, Undo and retained-terminal behavior; pathological
  million-byte lines remained slow. CodeEditTextView's comparison failed endpoint
  visibility and wrapped-line responsiveness, so no dependency or smaller file
  bound was adopted. Real filesystem and configured CLI/native checks covered
  reconciliation and Save. Physical IME/mouse, compositor performance and nested
  provider Home continuity were not individually established by those checks or
  Jack's overall demo approval. A wrapper's returned launch argv must retain its
  Home context; successful initial launch does not prove nested commands do.

## Accepted calmer workspace (2026-09-25)

Repo → Wave → Task → named Session is the public hierarchy. Project is internal
chapter ownership. Optimize one repository: connected header/sidebar, started
Tasks only, bottom search. Starting work is durable evidence, independent of
Session/process liveness; inspection never starts Task execution. The accepted
September 30 desktop discovery operation ensures primary repo/Wave conversations;
ordinary list/status reads remain read-only. Wave detail holds its
objective, Current KRs and complete Task plan, including unstarted Tasks.

The accepted visual direction is D: sidebar A, center structure B, mood C.
Use warm cream/burgundy, light selected-row tint/edge, serif page titles, sans
body and literal lowercase mono Flow skills. Task title appears once; its issue
ID in the breadcrumb links to Linear. Use Description; collapse real Comments
with their count below it. Dated updates belong in comments, current scope and
blockers in Description. Omit ELSEWHERE, Local work evidence and preview jargon.

The September 25 pinned-Flow, Resume and Stop & restart presentation is retired
by October 4's correction. Its details remain at
`9d72242ce:wave/product/MEMORY.md` under this heading. Retain exact graph
occurrences and independent return counts, including nested loops.

New session sits beside the Task title, independently prepares its checkout and
opens a conversation without starting its Flow. One open Session enters directly;
several lead to named choices. Session names seed from the invoked skill or the
recovered magical-musical generator (history had no animal list); inline rename
and later agent suggestions preserve human names. Session view omits Description.
Exact membership distinguishes current, earlier, past, independent and unknown;
its chip reveals the exact graph occurrence, never a newer same-named invocation.
Recent Runs load on demand; provider and summaries come from recorded evidence.

Navigation retains the existing checkout layouts and native surfaces. Explicit
conversation focus moves breadcrumb and Rename together; companion focus keeps
conversation context. Outstanding rename and delayed New-session feedback retain
the originating identity. Neither navigation nor a matching cwd authorizes client
transfer. Task/Session completion and closing a view remain different actions.

September 25's promotion receipt and superseded composition remain at
`6c44d9792:wave/product/MEMORY.md` under this heading. Jack rejected the
composition September 26; provider input/retention remains unproven. A missing
chapter may mean the wrong Home: preserve the chapter-bearing Home before rotation.

### Identity, retention, and counterexamples

The retired prepared-Run/review and `lf runs --active` contracts remain at
`9d72242ce:wave/product/MEMORY.md` under this heading. Preserve the independent
lessons: preparation proves identity, never liveness; release launch locks before
waiting on a provider; discover native clients from existing receipts, without
requiring a new marker. Deduplicate exact captures, retain live history past age
caps, and keep missing ownership evidence explicit.

- Native interactive history can follow an originally headless Run. Retained
  client namespaces preserve Session discovery after exit; explicit resolution
  removes it. Preserve launch provenance and resolve declared issue/slug subjects
  through shared Work binding. Passive output must also follow native continuation;
  choosing journal output solely from original launch mode loses that history.
- Rust owns legal actions and Work paths; Swift dispatches them. Shared Wave
  ancestry keeps historical Project-bound Sessions reachable across chapter moves.
  Retired Ready/Iterate mechanics remain at `6c44d9792:wave/product/MEMORY.md`.
- Pane ID alone cannot restore a Task's choice: the pane may now contain another
  Task's Session. Retain expected content and save direct Sessions only for their
  exact selected Task subject. Refresh retained Task evidence with each successful
  planning publication; selection-time text can be stale by a transfer gap. Cancel
  asynchronous historical-reference reads on navigation so late results cannot
  replace the newer Task or repository selection.
- Monitor needs its own AppKit responder: clearing focus to nil can select a visible
  terminal. Hidden terminals release input; resize/polling must not steal search
  focus. A retained native view owns focus requests and latest title. Verify ordinary
  dispatched input stays out of both PTYs while Monitor is focused, then require
  exact retained draft and child replies on return. PTY echo alone is insufficient.
- Retired completion/Approve/Iterate UI lessons remain at
  `6c44d9792:wave/product/MEMORY.md`. Action errors must preserve live terminals;
  delayed results retain their originating workspace, and external disappearance
  invalidates hidden Undo.
- Shell-to-Session attachment uses actual client terminal identity verified against
  stdin's PTY, never cwd/title or an inherited marker after external handoff. Multiple
  attached Sessions keep individual actions. Completing one leaves its shell usable;
  closing/restoring a launch pane must not replay the original command.
- Align configured read/action Home and database paths explicitly. The recorded
  mixed-Home app combined successful planning/Session reads with another Home's
  process observation. Preserve selected Home/account context while clearing inherited
  execution/terminal markers at GUI launch; `roadmap --all` ignores ambient Wave scope.
- Ghostty mapped CoreVideo display-link error -6661 to OutOfMemory. The same artifact
  created surfaces with its existing timer renderer. Probe that capability once and
  fall back only on failure; nil surfaces alone do not diagnose physical memory or
  an unavailable desktop. Resolve appearance at the window so native controls and
  custom foreground/background colors agree.

### Measurements and acceptance that survive scratch cleanup

- Keep **hierarchy_interaction_ms** and **task_workspace_ready_ms** separate. Measure
  accepted input to correct rendered/usable rows or destination content, with exact
  identities, truthful active/empty/error state, retained focus/input and frame hitches.
  Retained terminal switches are not provider startup. Capture read/decode/projection/
  layout/presentation phases under one interaction ID without adding product widgets.
- The opt-in `uv run python scripts/desktop_performance.py run --output <new-dir>`
  uses 8 Tasks/4 Sessions and 256 Tasks/128 Sessions, two checkouts and three owned
  cat PTYs. Its eleven scenarios use synthetic active-Run DTOs and forced native
  bitmap capture/text verification plus PTY replies. This is an intrusive capture/input
  endpoint, not compositor presentation or hitch proof. The concurrent writer
  completed 462/462 source-stable observations; the hash-verified [baseline](../../scripts/benchmarks/desktop-performance/20260924-capture-input/README.md)
  now survives scratch cleanup. Preserve begin/end records, failed and
  unstarted attempts, host/build/population/endpoint compatibility, observer overhead,
  and source drift. Twenty successful comparable samples are required for its p95.
- The Monitor reader and pane this baseline exercised are deleted (#1452,
  #1439); its two Monitor scenarios no longer compare. Capture/input receipts
  never established compositor hitches or provider costs. Detail at
  `e67cdc62f:wave/product/MEMORY.md` under this heading.
- Jack's September 26 split assigns further performance work to existing LOO-300,
  superseding the earlier proposal to file two optimization Tasks. Keep hierarchy
  navigation and Task workspace interaction as separate measurements, consume the
  stable runner and baseline, and preserve identities and retained terminals in
  comparable before/after evidence. The split does not waive missing proof.
- The human rejected the accumulated composition as confusing. Earlier provider,
  editor Cancel/rejection, viewport, and empty-Monitor receipts are bounded evidence,
  not approval of the simplified UI. Preserve configured positive Run appearance/exit,
  combined-pane input and human composition confirmation as remaining acceptance.
  AXWindow role, exact input focus, lock status, and permission are separate facts;
  old failures do not diagnose a new runner. Never replay retired mutation probes.
- LOO-291 retains ten human-selected external-work trials, an authorized directive
  edit, and twenty long-lived-registry trials against published budgets. The external
  workflow/text remain unprovided. LOO-251’s promoted-Ask blocked-caller proof
  was superseded by Jack Heart’s 2026-10-01 Ask removal; retained Task reviews
  still require completion proof. D2’s fourteen-day/twenty-open readiness
  obligations remain. Local PTYs, one cached
  population, or AX count timings do not satisfy these. Earlier fallback attempts
  stopped at resource preflight; the September 25 supervised Xcode compile later
  passed. Neither compile supplies the missing verdict or authorizes removing
  another checkout's active build.
- LOO-185 remains parked until human-selected Discord use is blocked by provisioning
  friction; canvas work supplies no new activation evidence.
- LOO-284's shared contract is implemented within LOO-291, with remaining configured
  all-kind acceptance; keep its identity and prevent duplicate implementation until
  delivery is reconciled. LOO-293 retains passive output, Flow history, all-provider
  continuity, paging/bounds and its configured human demo. Adapt its presentation
  into Monitor's multiplexer instead of restoring a separate Watch route/tree.
  Neither Task is complete because primitives were integrated here.
- Current-Wave filtering belongs to shared CLI reads. Historical `list` retained
  abandoned Task/Project/snapshot evidence and could not be forgotten; exclude it
  from current navigation without deleting history. `engbot` was removed through
  the supported operation in the earlier authorized installation. No raw store
  cleanup or new installation follows from memory curation.

## Chapter decisions and review lessons (2026-09-23)

The accepted September 23–October 21 start, different summer/start KR populations,
and detailed review lessons remain at `626789dcd:wave/product/MEMORY.md` under
this heading and [.lf/chapters/20260923T000959Z-502f011b/start.md](../../.lf/chapters/20260923T000959Z-502f011b/start.md).
Plan acceptance did not authorize publication; later decisions govern hierarchy.

Product value is selected material progress in three of Cube, Etude, Kata and
Hootro, never planning churn or self-hosting alone. Preserve external capacity;
Small/Medium/Big are review heuristics. Dogfood legitimately discovered execution
work; Infrastructure now owns execution, Intelligence evidence, Product experience.

Freeze exact historical Project/KR membership and report Run IDs before counting.
Current rosters and overwritten snapshots cannot recover missing lineage or
duration proof. Preserve dated revisions; unknown stays unknown. Reports connect
observations to beneficiaries and distinguish carry, learned, unprioritized and
misplaced work. Review proposes; accepted start applies. Aggregate per Wave,
never restore a Project operator. Report plus Run settlement establishes review
completion; archive authority requires explicit Task binding.

## Work and continuity (reconciled 2026-09-23)

- Work identity and domain ownership follow AGENTS.md. The September 23
  prepared-Run and internal-Project account is archived at
  `0e9b9b705:wave/product/MEMORY.md`; launch attribution grants no control.
- Steers remain durable authored inputs; retired Chat/mailbox details share that archive.
- **Another Work perspective is an ordinary Run; interactive work is a
  Session.** Launch `lf --as <work> : <question>` when another agent perspective
  is useful. Ask was removed by Jack Heart’s 2026-10-01 decision. Failed work
  retains its normal logs and outcome; necessary judgment belongs in the existing
  Wave chat. Authored review Sessions return feedback to their recorded Flow
  boundary, with navigation owned by the following decider. There is no agent
  exchange row, answer lane,
  or dedicated answer controller.
- **Wave memory is file-only.** Applicable ancestor `MEMORY.md` files are read
  oldest-first. There is no live memory stream, and recent Wave Chat is not
  ambient Project/Task prompt context.
- **Environment configures a process; it never decides what the process is.**
  Work identity comes from durable state; execution and signal authority must
  be established at their owning boundary, never inferred from a Run id,
  inherited endpoint variables, or a surviving terminal.
- **Backlogs are allowed.** Linear Tasks may exist without a Run; open Runs are
  not the Wave's roadmap.

### Earlier runtime findings (July–August evidence)

Resident-era incidents remain at `b908182f5:wave/product/MEMORY.md`.
October 4 retires automatic recovery. Keep unknown liveness explicit; missing
Flows must not silently retry. Durable Steers survive provider/app exit;
mid-turn delivery is provider-dependent.

## Model (design invariants)

- `lf` and durable store projections define the product API; Mac and iOS
  consume that model rather than inventing a parallel lifecycle.
- **Terminal-only and agent-embedded Loopflow are normal primary modes.** Chat
  is an optional shared steering and observation surface, never a prerequisite
  for operating Loopflow or an onboarding funnel every user must enter.
- **Wave Chat is asynchronous steering, not a low-latency support chat.**
  Durable inputs preserve order and may wait behind existing Wave work; surfaces
  show delivered, queued, and working state instead of implying an immediate
  conversational reply.
- App surfaces navigate, present, and Steer Work. A view, terminal, provider
  process, or listener is never the source of Work or review-playhead truth.
- A provider session is AgentInvocation continuity, not Work identity.
- Home-local execution, Work continuity, provider history, and direct process
  control have separate owners. The current boundary is documented in
  `docs/architecture/{execution,planning,homes}.md`; earlier open topology
  questions below are historical context, not an alternate authority model.

### Planning authority and historical migration lessons

- Linear owns authored chapter and Task content. No local `projects/*.md`, issue
  mirror, or roadmap table is authoritative. Shared current reads expose one chapter
  summary and direct Tasks; historical Project identity remains readable provenance.
- PM writes resolve stable provider edges, not names or Issue prefixes. Task creation
  selects its Wave's current Project; ordinary Task saves cannot restore an old
  parent after transfer. Wave-linked Initiative identity remains in GOAL frontmatter;
  repository configuration owns the Linear Team.
- `lf pm show --no-sync` is cache evidence, not proof of current provider state.
  `synced_at`, snapshot generation, and source freshness have different meanings.
  A rejected write can follow a successful provider mutation if readback failed;
  retain the draft and report the uncertainty instead of asserting nothing changed.
- Repository Team migration uses the supported `lf pm reteam` operation and its
  complete ownership preview, reconciliation and receipts. July's pending PRD-44
  migration was a dated state, not standing permission to modify a live store.
  Retain old completed/canceled Work and original provenance across migrations.

### The `lf` / Home spine

`lf` owns commands and durable reads. Remote execution uses the target Home's
`lf` over SSH; observation grants no execution authority. The retired Discord,
lfd and resident-cron contracts remain in
[git history](https://github.com/loopflowstudio/loopflow/blob/c418953634bd101f51878d2be2b40fb3facafabd/wave/product/MEMORY.md).

## Shared planning and runtime vocabulary

Wave → Task is the public planning model. Chapter/Project identity remains internal
and historical. Shared status, roadmap, cached plan and Rust/Swift fixtures must
change together; hiding a Project array only in Swift retains the obsolete contract.
Run records launch provenance and can be prepared before a provider starts; Session
names human continuity/boundary; Exec supplies process ownership evidence. None can
substitute for another merely because identifiers coincide.

## Swift data path — RegistryQuery is the single reader

- **All data reads converge on `RegistryQuery`** (subprocess `lf … --json`,
  daemon-less) — including Waves, status, roadmap, Sessions, Activity, usage,
  and recent Runs. The
  HTTP-to-lfd-as-API path is **deleted**: `LocalWaveService` (~1500 lines) and
  `WaveServiceProtocol` are gone; ~22 consumers rerouted onto RegistryQuery.
- Unknown wire status must surface explicitly, never silently become pending.
  The former lfd integer-enum comparison is retired with that API.

## Performance — launch renders before any read (2026-10-04)

Jack Heart requested Desktop open on a usable workspace with one loading
vocabulary and one refresh path (LOO-376). On Jack's Home no `lf` read fits the
1000 ms budget (`roadmap --all` 14.5 s), so a returning launch shows saved wire
text first; that latency has no owning Task (LOO-375 owns `wt list` only).

- Save the reads' wire text, restore through the live decoder, and strip
  liveness and legal actions before saving. Display evidence, never authority.
  Keep it inside the Home; a different `lf home id` drops content and selection.
- One model owns refresh; views that each start a loop supersede each other's
  reads. A view that builds its own model reopens the blocking path.
- Jack's delivery contract (2026-10-04): land on autonomous checks and honest
  benchmark evidence; rendered startup is post-merge validation, not a gate.
- Receipts: `scripts/benchmarks/desktop-performance/`. First frame went
  850 → 617 ms under heavy load (20261005-first-render). Profile before
  guessing: the cost was getters re-normalizing paths per render, not drawing.
- A benchmark that reopens one bundle hides the launch Jack gets. Both recorded
  real launches were first runs of a new version: 625–790 ms before `main`.
  20261005-first-launch: the system charges a new binary about 390 ms, partly
  by file size, so no post-update launch meets 400 ms through app work.
  Stripping local symbols saves 40–100 ms but unnames crash-report frames;
  shipping it with a retained dSYM is Jack's open choice. Reopened: 430 ms
  first frame (4 samples, loaded host).
- Still open on LOO-376: quiet-host proof of 400 ms; cold file cache and
  selection/repository-change refresh scenarios; unsaved launch waits on `lf`;
  `session list` runs 2–3 times (refresh owner: LOO-382/LOO-304).

## Sessions projection and native resume (reconciled 2026-09-24)

The older Ask/Ready/Complete and Task-review control contract is superseded by
October 4's Task conversation correction. Its dated implementation and proof
notes remain at `16fa9742591e3edfc0ed42c913c64347c02ffeb5:wave/product/MEMORY.md`
under this heading. Retain these independent constraints:

- `lf session list --json` owns the shared Session projection; Desktop owns no
  second queue, title store, liveness model or resolution state.
- Provider history owns native resume. Row selection focuses the retained
  terminal; **Move here** explicitly replaces an exact Loopflow-owned client
  using PID and birth evidence. Resume preserves history, not unsent TUI input.
  `session open --json --replace` prepares argv without stopping the old client.
  Closing a pane or provider exit resolves nothing.
- Desktop retains terminals across selection, with explicit splits and reversible
  Close view/Undo. Viewing, running, elsewhere, opening and retry remain distinct.
  Canonical Git common-directory identity keeps Task checkouts out of repo roots.
- Configured provider continuation and native focus/pane reconciliation remain
  unproven by mocks, launch screenshots or empty inventories. The deleted review
  handshake is no longer an acceptance requirement.
- LOO-251/284/291 retain native, shared-action and planning proof obligations;
  the archived reconciliation establishes neither completion nor new follow-ups.

### Task observation and Watch (2026-09-23)

The dated worker-evidence, FlowPosition and capture-binding notes are at
`67cd68157:wave/product/MEMORY.md` under this heading; the October 5 cut
replaces their mechanism with Exec evidence. What still holds:

- **Incomplete observation caused duplicate implementation.** Desktop launched
  two bound `implement` helpers into LOO-293 while its original worker was still
  producing output. A completed launcher or a missing `lf ps` row cannot
  establish idle execution. Unknown liveness is not idle.
- **Watch belongs to Product / Desktop's existing LOO-293.** Keep one connected
  stage diagram and labeled all-Run feed, stage/Run filters, Follow live, and
  completed history together, including passive native interactive output.
  Join history by exact step identity, never by skill name or current YAML.
  LOO-293 retains native capture, bounded history and its configured demo;
  nothing on this branch validates or settles it.

### Terminal ownership and input (branch evidence, 2026-09-22)

- Each window owns a repository workspace registry, which retains checkout
  layouts and one terminal view pool across outline/content and repository switches.
  Each view owns its Ghostty surface. A global Session-ID surface registry caused
  one window's release to destroy another's terminal; it is deleted. Do not
  share an NSView between windows or evict a hidden Session to save memory:
  freeing a surface ends its PTY. Window close/app quit still end embedded
  clients; history resumes, unfinished drafts do not survive.
- Session identity, provider process, local opening/error state, terminal view,
  and pane placement have different lifetimes. Keep them separate. In particular,
  `.prepared` bridges async opening and surface creation, and an open failure
  stays visible until retried or superseded. Shell panes also survive navigation,
  and closing an outer checkout slot only hides its entire inner layout. Explicit
  inner close/process exit closes that shell. September 30's Task workspace removes
  the separate Task terminal owner and uses these retained shell/Session lifetimes.
- `TerminalIdentity` carries Session or shell purpose through
  views, pools, and bell/title/close notifications. Input policy follows the
  enum case, never string prefixes. Resolve callback identity from surface
  userdata while its handle is valid; deferred notifications carry values.
  Late callbacks must not discard replacement views. A dead/released view must
  never relaunch a retained `--replace` command and steal a client back.
- The displaced launcher needs **stop intent after liveness ends**. Record
  moved/completed before signaling; consume it to print a clean handoff message.
  An unmarked SIGTERM remains an error. Clear stale intent when publishing a new
  client so PID reuse cannot inherit it. This record is not duplicate liveness.
- AppKit offers key equivalents to sibling views. Only the first responder may
  consume terminal Command-V; a focus border alone proves nothing. The regression
  dispatches through a parent and reads both real PTY buffers. Copy reads actual
  Ghostty selection; core clipboard requests must receive a completion callback.
- Session clipboard images use the provider's Ctrl-V image shortcut. Copied file
  URLs win over image bytes and insert escaped paths; raw dropped image data is
  saved as readable PNG before insertion. Retain temporary data through provider
  consumption. A path appearing is not proof of an image attachment: inspect
  the composer. Shell paste uses paths, not provider shortcuts.
- The 2026-09-22 interactive demo confirmed retained Session switching, copy/image
  input, and explicit Warp handoff. A later demo exposed split paste misrouting;
  the automated real-PTY correction passes but its final reviewer confirmation is
  still pending. No all-provider matrix exists. JSON prepare-without-kill
  has source/Swift contract coverage but no focused Rust behavioral proof yet.

### Shell command blocks and build fidelity (2026-09-22, revised 2026-10-05)

- Reviewer acceptance is **visible grouping before interaction plus one ordinary
  click anywhere in a completed command/output region selecting both**. Invisible
  OSC 133 metadata, triple-click gestures, or tooltips do not meet it. Warp is the
  benchmark; preserve Loopflow's palette. Provider Session panes remain outside
  shell-block semantics.
- Jack Heart's October 5 report (LOO-381) added: colors lost in provider CLIs,
  a context header and indentation per block, a red background from the real
  exit status, and one selection at a time. Jack asked for more space between
  blocks and Warp's small dim directory header (drawn in the overlay from a
  concealed prompt row), then rejected burgundy selection beside red: red
  means failed only, selection is cream. He approved the demo overall and
  asked that it land; no per-check report, "the bar on the right" unresolved.
- Block chrome never takes selection or focus from the shell: a right-click
  keeps the selection under it, any key reaching the shell ends a block
  selection, Command chords keep it. Fills keep the dimmest palette color at
  3:1. Open: Option-letter inserts the composed character.
- **Desktop inherits its launcher's environment.** Opened from an agent shell it
  carried `NO_COLOR=1`, `PAGER=cat` and `TERM_PROGRAM`, and every pane lost
  color. The GUI drops those variables at launch, beside the execution markers.
  A prefix scrub also caught provider credentials `lf` reads; keep them by name.
  Not the palette or Ghostty config.
- **The terminal owns block state.** A Swift-side `(id, text)` snapshot produced
  two competing highlights and stale copies; pointer-derived ids died on reflow.
  The `lf2` patch keeps exit status on the prompt row and one text-or-block
  selection in Ghostty's `Screen`; Swift holds only hover. Upstream also splits a
  block when its command line soft-wraps; the patch treats the wrapped row as a
  continuation. A new artifact version accompanies every patch change;
  publication needs authorization. Jack authorized `lf2`; published 2026-10-05.
- SwiftPM pins the published, checksum-verified patched artifact, not a local
  build path. `swift/GhosttyKitPatches/` plus `loopflow-dev.py ghostty-build`
  carry the patch and now run its Zig tests. Avoid synthetic multi-click
  workarounds.
- Headless tests cover the launch environment, block layout and style, and the
  zsh hooks. The real-PTY click/drag/resize test needs a display and has not run
  for `lf2` (filter by `GhosttyTerminalInputTests`; display names do not
  match). Custom prompts, bash and fish headers, and on-screen overlay
  alignment are unproven. Provider `TERM` stays
  `xterm-256color`; changing it needs separate fidelity evidence.
- **Build parity remains broken.** `project.yml` copies resources but still builds
  Ghostty-disabled stubs, unlike SwiftPM (LOO-280). Binary/resource revision
  facts remain duplicated, the build recipe does not regenerate the shell
  payload, and missing resources disable all terminals.
- Still deferred until the core interaction is accepted: separate command,
  output and last-command actions, multi-selection, bookmarks, sharing, exit-code
  or duration badges. Upstream API contribution may reduce patch maintenance
  later (ghostty-org/ghostty#11747).

### Shared viewing boundary

Native launch plus explicit Move here stays the main path. Optional
simultaneous Warp/Loopflow viewing (second attachment view-only, explicit
**Take control**) is unbuilt; compare an opt-in tmux configuration with a
transparent PTY relay before changing the contract. The proof list and tmux
findings are at `f8fe8aff0:wave/product/MEMORY.md` under this heading. Filed
2026-09-22: LOO-280 build/resource parity; LOO-281 shell blocks and visual
proof; LOO-282 client provenance; LOO-283 the shared-viewing comparison.

## Learnings

- **Reshape proven code; don't rebuild beside it** (code only — a rewrite loses
  hard-won correctness). Does NOT apply to the charter: stale framing is a
  liability, so GOAL.md/roadmap get rewritten freely while MEMORY is curated. The
  fresh `RepoSidebarWindow` re-derived the burgundy sidebar / create sheet /
  terminal panes and got each subtly wrong; the proven components already encode
  the right style + behavior — adapt them.
- **`loopflow-dev.py` builds from the worktree it runs in.** Repository
  discovery collapses linked worktrees to the canonical main checkout.
- `cargo test -p loopflow dto_fixtures` filters by test name; use
  `--test dto_fixtures` to run that integration file. Headless runs set
  `LF_RUN_ID`, `LF_AS` and `LF_FLOW_ID`; Rust tests that launch or assert journal ids
  must clear them or `cargo test -p loopflow` fails only under agent runs.
- **Historical migrations demonstrated the shared-store blast radius.** Product
  and Intelligence collided on `061`; editing an already-applied migration left
  existing databases without a required column. Preserve released migrations and
  test upgrades from the released frontier. The incident detail and June's
  superseded remote-client recipes are in the
  [pre-chapter memory](../../.lf/chapters/20260923T000959Z-502f011b/sources/wave/product/MEMORY.md). AGENTS.md owns the current one-draft-per-Task rule.
