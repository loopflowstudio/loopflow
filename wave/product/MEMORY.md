# product wave memory

Renamed from `concerto` July 8, 2026. Product owns API/CLI, Mac,
iOS, agent and worker surfaces.

## Current direction after the October 7 Tasks

LOO-387–430 briefs and LOO-427 non-progress comments supersede older designs/help.
LOO-389's brief was read; comments failed on an unbound Initiative.

- **Machine replaces Home** (LOO-394); one OS user and data directory.
  `LF_HOME`, provider homes and stored opaque IDs keep their meanings. LOO-411
  replaces `lf ssh` with global `lf --machine LABEL COMMAND`, no alias or
  cross-version negotiation. Source owns current vocabulary, not older installed help.
- **Work first.** Jack Heart (October 8): agents are first-class users of the
  software engineering CLI. Desktop centers Work; orchestra language stays top-level.
  One repository window across machines, identified by Work. Jack accepted
  CLI Session+diff opening, inspection and arrangement. Execution
  and display machines differ; takeover is explicit. Partial below.
- Jack: delegation applies to open, unstarted and future Tasks. Started Tasks stay
  on their machine across later runs. Inheritance remains proposed.
- Jack: Desktop control is macOS-only (LOO-426/427); Linux errors name a terminal
  alternative before launch or Work mutation. Ordinary lf remains cross-platform;
  displaying and execution machines differ.
- LOO-416 owns saved per-Task panes and reattachment; LOO-426 owns Task/Session
  opening; LOO-402 owns Waiting navigation, including blocked shell panes through
  shared Swift while Session Waiting stays in Rust. LOO-403 owns shortcuts and
  palette, LOO-387 native draft preparation. Programmatic Desktop control must
  reuse these owners. LOO-415 owns the transparent relay and all lf status
  emission; LOO-398 is reading only. LOO-422 owns host status/title fidelity.
- LOO-418: a Task has zero or one PR; follow-through is another Task. Serial PR
  chains below describe the implementation being replaced. LOO-401 selects
  `self`, `config user`, app opening and HTML screenshot removal without a
  replacement. Desktop's native snapshot is separate. LOO-406 owns the accepted local
  planning lifecycle, preserving connected Linear repositories; LOO-412 owns exchange.
- LOO-428 retains argument delivery in its launch/output repair. LOO-429's later
  decision puts all assembled context in the system file: no split, fallback or
  lf-side interactive error wording. LOO-420 uses native invocation for native
  skills, translated ports across harnesses, builtins inlined. Later comments supersede the briefs.

[LOO-427's comparison](../../docs/reviews/terminal-command-comparison.md) records
command dispositions. Jack folded LOO-430/431/432 into LOO-427: identity, window
control and terminal I/O in one diff. October 8: design approved; implementation
authorized through demo review. Owned cmux probe cleaned up; no Desktop proof.

October 8: LOO-427 leaves unknown checkout location unavailable. `92cafe61d`
removes copied placements, preserves legacy overrides and routes known Tasks
without rewriting Machine defaults. Inheritance is unapproved; placement suites remain with gate.
Neither negative peer observation nor local SQLite reserves first start; runtime
must not synchronize. Seeded IDs prove routing, not exchange/admission.

`desktop text/key` reuse existing owners: Jack selected cursor insertion into
drafts with Enter separate; control-text/IME limits are reversible. Verified lf3
enables bounded reads. Headless builds skip native I/O diagnostics;
extraction/input remains unproved. Reads grant no input authority.

Jack's October 8 API: `--context` previews input, `--explain` is broader,
`--chrome` controls execution. Root `run`, `--max-turns` and `--no-loopflow` are
removed; Task skill/Flow selection survives. History owns activity, Process pages,
show/replay and usage; Desktop owns open/list. Bare `open` conflicts with PR opening;
Sessions use `connect`. October 9 local Task/Session-plus-Changes opening bypasses
execution routing and CLI preparation. Session/companion intent travels together; a partial Session inventory must not
retire other panes. Opening generations follow Session/Files/pane readiness and failures. Wave-owned `show`, `workflow` and `edit-plan` replace root roadmap/Project locally;
retained Project IDs and the snapshot/writer owners survive. Desktop sends Wave identity.
Flow previews expose initial input; future/loop input stays unavailable. Broader explanation remains. Discord is unchanged;
`--steers-after` filters nodes, not restarts.
`repo_root` (default `~/src`) and paths resolve on the selected Machine, never as
portable identity; remembered names remain open. Scoped IDs survive dispatch.
Preview/routing use read-only SQLite; unreadable is not absent. Preview/launch share
assembly and resolved operator scope; only launch writes excerpts.

Jack selected local planning/Git sync: 406 owns writes/Linear, 412 exchange.
Imported completion cannot move Workflow or clean execution. Preserve pending
edits, semi-live comments/completion, causal reopening and mutation IDs.
`29777b8cb` scopes Desktop sync to repository, not selected Task. Integrated
LOO-406/#1503 includes Linux reconnect and observed-reopening regression evidence
(Infra memory); composed Git/Desktop proof remains. Linear's unconditional writes
can overwrite unseen reopening: readback proves observed state, not atomicity.

Jack selected user-keyed Git plans by default, explicit shared opt-in, Linear wins;
otherwise host preference then last-write-wins with recoverable edits. Code remotes
grant neither identity nor publication permission. Joins select/publish nothing; Wave selection inherits to descendants.
Multiple destinations must not split the repository window.
Private-ancestry holds retain journals/dependents and local moves;
peers keep old values, so omission is not convergence.

LOO-427 composes LOO-412's `19e31f64d` journals, common receipts, unplaced-Wave
reads and destination-level Desktop Work stream. Mixed Linear/Git stays disabled.
**Retained uncertainty constrains effects, not saves/acquisition.** Save clocks
order intentions; unresolved losers hold delivery. Complete lists alone settle
order; primary-only moves retain input. Provider facts retain separate savepoints,
ages/baselines and causal frontiers. Import reuses portable fields.
Entity revisions cannot order relationships; scalar/list replay cannot clear
freshness or unseen notices. Prior evidence: `7d12ab10a`, this heading.

October 9: `d4fbca0ae`/`a73198197` bind roots, retaining locators, Work and execution,
publishing nothing. Journals exclude RepositoryId/delegation; imports allocate identity.
Desktop reads locators atomically, retaining native owners. Scene equality misses
A→B→A; per-path fences miss concurrent locators. Opening/restoration share one
observation fence; headless fixtures prove no native usability.

**Unmapping is not association:** it can enable duplicate creation. `66dd3c44f`
composes LOO-412 through `de3c84b08`: correspondence/full-ID lookup preserves
physical IDs, private selection, effects and execution; import creates no requests.
Store/CLI/migration checks pass, not Git acquisition.
`2afcfba1a`: lookup, mapping and record reads share one snapshot;
subsequent calls recheck. Observation reserves nothing.

Uncomposed `3c67b29b1`: creation-origin import and exact-Linear-fact causal links;
fixtures unexecuted. Equal text is not causality; links retain per-origin heads and
private dependencies. Joint projection remains unfinished; mixed exchange/first-start
refusals and TaskSource exclusion stay.

#1512 (`3e1e6245c`): live connect hands off; dead-driver resume starts an engine
on native history. Local resume is not import-triggered launch; native proof remains.

October 9 (`e3ca861b1`): observe visibility before Session readings; callbacks
masked closure. Expose pre-registration failures; Task links await mounted pages.
Request IDs fence registration, not later arrivals. No native/compositor proof.

October 9 planning reads use read-only storage and skip Process admission;
missing-registry/checkpointed-WAL tests exposed fallback writes. Resolve chapters
once on the operation's store; Workflow reads/writes have separate APIs. Preview
shares launch admission, Workflow, URL validation and definition syntax; only
launch prepares Work. Linux previews report platform impediments. Explicit-Machine
previews skip probes/credentials; entry validates addressed identity before early
readers and suppresses fallback Process writes. Parsed transport retains scoped IDs.
Checkpoint both stores: an effect-free receiver cannot undo a sender's write.
Two-CLI fixtures cover routing/Flow previews, not configured SSH or exclusive start.

Session connection explanation shares selection and native-client refusal with
opening. JSON preparation is not takeover. Socket probes enter a live driver's
protocol: read recorded endpoints without probing, retain reachability
as unavailable, and re-resolve on opening. Fixtures preserve client receipts
and checkpointed stores, not native/provider acceptance.

## Terminal-host adoption (2026-10-07)

Jack Heart: Flow → Task → Wave, Desktop optional. Automated trials are account-free;
Jack starts real providers in dedicated windows. No host code is lifted.

Herdr/PTY fixtures passed taskless paths. Fresh Tasks require Linear; seeded Tasks
accounts: refusal is not adoption. Original lf
0.13.9's revision is unknown; source fixes cannot rebut release reports.
Owners: LOO-422 status/titles, LOO-428 launch/Flow noise, LOO-423 account/polish,
LOO-406 no-Linear planning; LOO-429 prompt placement overlaps 428's transport.

Jack prioritizes native interaction. The trial exposed forced batch steps
(introduced #1283); this branch forwards mode through skills and Task launches.
28 Flow tests pass. Jack saw native step two in cmux and confirmed herdr's Flow,
background question/clearing and resize/input trials. Successful provider exit
advances; a deliberate Continue/Stop action remains unresolved. Native resume
and herdr Task publication remain unproven. No host integration was added.

Native resume's **Loopflow operating guide** is one repeated observation;
first-content naming stays a hypothesis. Preserve native hooks/titles before
adding outer Flow metadata that could obscure attention. Jack authorized landing
the fix after these demos; remaining trials keep LOO-421 open. [Findings](../../docs/reviews/terminal-host-trial.md)
preserve evidence and limits. No external-progress proof.

## Live Home reconciliation

LOO-367, Intelligence backlog, test Waves, LOO-343 and LOO-380 remain unverified;
no rendered proof. Prior account: `f881ae647:wave/product/MEMORY.md`, this heading.

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

## Workflow and Flow alignment (LOO-386, 2026-10-07)

Jack Heart approved LOO-386/#1492, retaining Workflow/Flow and restart-to-start,
then requested gate and Task-completing landing. Review/approval:
`9669d67d5:scratch/workflow-review.md`; earlier detail: `97bf9dbcb`, this heading.

Independent namespaces retain kind/name. Restart keeps the graph/history;
Project selection affects future take-up. Latest FlowProcessDetail grants no
control authority; selection, execution and TaskRunControl stay separate.
LOO-400's `e467ea995` supplies Process/LFID; old captures remain readable.
Wave-owned commands supersede Project commands. A reset Task cannot advance
from an older Flow's arrival. Catalog, gate failures/repairs and reruns:
`a73198197:wave/product/MEMORY.md`, this heading. Approval proves no native,
live-planning, configured-provider acceptance or chapter KR.

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

**October 5.** Jack rejected parallel Task-worker drivers: Task launch must enter
ordinary `lf run`; `-b` blocks and callers background it. Steps are plain commands,
tracked only by their driver; launch-time PR preparation is unjustified. October
7's Process vocabulary supersedes the FlowExec/FlowSession naming discussion.
Original feedback and record proposals: `f5f742058:wave/product/MEMORY.md`, this
heading. Task Workflow and Flow execution remain separate owners.

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
- Jack retained Flow, with edges named by their Flow/skill and unique per
  departure node. One Task run can start several Flow processes. October 7's
  Workflow/Flow decision supersedes the naming draft; original steers and
  retired alternatives: `af79d4762:wave/product/MEMORY.md`, this heading.
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
- Earlier `feature`/`code` recipe and commit-push decisions: `942dab5d8:wave/product/MEMORY.md`, this heading. Historical recipes grant no review or delivery authority.
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

Jack Heart, [LOO-383](https://linear.app/loopflow/issue/LOO-383): the Wave
conversation operates; keep started Tasks moving, leave unstarted backlog alone,
and inline the operate procedure in each session skill. `task/session` stays
plain; its primary wrapper remembers the ID (LOO-364). Jack approved the demo
and requested PR-linked reports grouped as waiting on him, moving and stuck.
Leaving unarmed merges to Jack is unreviewed.

PR #1439 supersedes stopped-Flow continuation: a Task run starts fresh execs;
reconcile never resumes. The agent's unreviewed policy waits on Jack at nodes
and reruns failed edges for three attempts. Historical decisions and the dated
disabled-cron observation: `6448e3c9e:wave/product/MEMORY.md`, this heading.

An operator once reported `next_move.owner: wave` as someone else's handoff:
name the reader's own responsibility explicitly. Nothing re-invokes a
conversation. Inspect every Flow/unfinished Exec against `lf ps --json` before
launching again. The [review](../../docs/reviews/session-operate-prompts.md) has
simulations and two read-only model runs, not installed proof. Open conversations
keep old instructions until `lf session replace`.

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

Capture uses lf-owned discovery and `skill <name>` to avoid Flow collisions;
presentation retains `(Home, checkout)`, both layouts and Task selection. A successful app build
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

October 7's #1490 (`d8ec7c4fe`) supersedes stream-only Waiting: validated OSC 7501
reports observed by Desktop override stream inference in Rust. LOO-384 remains
for non-reporters; reading Claude's transcript is Jack's open choice. Detached
observation and lf emission remain unfinished (Infrastructure's LOO-398/394).
Provider adoption and composed native-pane acceptance remain unproven; this does
not establish status in cmux or herdr.

## Current Tasks and completion history (2026-10-02)

LOO-371's October 5 snapshot measured warm 152 ms/reopen 38 ms, cold 8.4 s;
no Task-link sheet. OCR split an ID and falsely failed a sweep: verify the observer.
Own benchmark process groups; overlapping runs prove nothing. Full population,
failed attempts and limits: `96a42755e:wave/product/MEMORY.md`, this heading, and
[the evidence](../../scripts/benchmarks/desktop-performance/20261005-task-open/README.md).
October 7 retired performance acceptance; LOO-408 owns installed settlement.

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
process evidence grants neither retirement nor Flow settlement. Full Desktop acceptance remains unproved.

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

Jack’s LOO-365: `6c1bc2029:wave/product/MEMORY.md`, this heading. Taskless repairs,
unarmed-PR reporting and missing Desktop watcher view remain unreviewed;
no real failing landing was proved.

## Skill reduction decisions (2026-09-28)

Jack requested a smaller library, then restored debug as the default in examples.
Debug investigates and fixes a cause; unbreak restores a workflow. That distinction
is the implementation interpretation, not Jack's explicit wording. Neither needs
clipboard input. 5whys remains explicit systemic investigation; reduce and polish
remain optional surveys, with no accepted removal. Research includes conversational
codebase questions. Realign reconciles memory; pr-message owns PR authorship.

Historical recipes/consolidation: `c7359a7415f29c181b8e5bf2383cb5ba683d0586:wave/product/MEMORY.md`,
this heading; superseded by October 6–7's Workflow decisions.
The consolidation retired duplicate memory skills (PR #1319), expand, and older
Task/governance report pipelines. QA repairs authorized defects while independent
audits stay read-only. Delivery retains distinct publication, reviewer merge,
bare landing and Task-completing landing. Missing launch history proves neither
disuse nor caller authorship.

Jack requested single S1–S5 skills and identified their sequence as VSM. The
`vsm-operate` design composes delivery, coordination, capacity, adaptation and
identity with one shared pass note; each skill can act independently. Repository
scope is default unless the request narrows it. Whether wave/operate uses that
sequence remained open. Jack's “soften, dont harden” rejects numeric move caps;
October 5's pairs decision requires covering started work before choosing no action.
Task findings may challenge Wave purpose and Wave findings repository direction;
accepted decisions return to owners without adding control authority or making
operation a prerequisite for independent Tasks.

The design and evidence linked in that archive
retain decisions and simulations. September 28 passes lacked planning reads, not
backlog; five-step VSM/incident handoffs, installed adoption and delivery lack
configured proof. No KR or external progress is earned.

## Named participants and review feedback (curated 2026-09-25)

Original attribution/continuation sources and dated gate receipts:
`35e759aaf:wave/product/MEMORY.md`, this heading. AGENTS.md owns the naming rule;
curation changes no ownership or acceptance.

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

Name-attribution gate and fresh Jack/Maya/unknown CLI prose are recorded at
`f881ae647:wave/product/MEMORY.md`, this heading; no native, live Task-write or
review-Flow proof. Native review resume
still needs anonymous → named → corrected → unknown participant acceptance on
a differently named Home, preserving historical authors and review authority.
LOO-297's attempted read failed on missing Linear credentials; local Task absence
did not establish remote absence. Any sentence repair requires a fresh authorized
issue read. No Task was filed or changed by these records.

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
Chapter history remains part of that direction; the current contract uses past
Linear Projects, with no Chapter table. Navigation stays Wave → Task.
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
- **Measure first selection without pre-reading the target.** Corrected review
  measured 54/54 selections at 60–97 ms after real startup reads, checking exact
  bytes after capture. These AppKit endpoints had warm caches: no physical-click,
  compositor or p95 proof. Earlier 2,652 ms and failing populations remain
  unexplained; later passes do not erase them. ARM SHA acceleration reduced
  hashing cost while retaining install authority; priority/load changes did not
  explain the interval. Sizes, widths and failed receipts survive at
  `35e759aaf:wave/product/MEMORY.md`, this heading.
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

### Measurements and acceptance (reconciled 2026-10-07)

Jack Heart retired arbitrary numeric performance targets, soak requirements and
further optimization while the product surface changes (LOO-408, comment
`1e58addc-7bac-4fb8-bb38-1f8e36c7f45b`). LOO-304 closed; LOO-371/376/375 retain
only supported settlement after installation, with their historical uncertain
Session/Process evidence preserved. LOO-378 is explicitly paused, its substantial
unpublished work retained. Earlier p95, twenty-trial and soak obligations below
are historical evidence, not instructions to restart that work. This does not
rewrite the chapter's authored KRs or claim native/provider acceptance.

Keep hierarchy navigation and Task-workspace interaction as separate observations,
with exact identities, source/build/population, rendered endpoint, retained input
and failed attempts. The opt-in desktop-performance runner and hash-verified
[baseline](../../scripts/benchmarks/desktop-performance/20260924-capture-input/README.md)
remain useful instruments: synthetic Work, three owned cat PTYs, forced bitmap/text
capture and PTY replies. They establish neither compositor hitches nor provider
continuation. Its Monitor scenarios no longer compare because that pane was deleted.

Jack rejected the accumulated composition; earlier captures are not acceptance of
its replacement. Configured provider continuation, retained draft/focus and combined
pane behavior remain separate proof. AX role, input focus, lock and permissions
are different facts. LOO-291's original trial ledger, D2 obligations, LOO-300
performance allocation, observer corrections and failed attempts are preserved at
`35e759aaf:wave/product/MEMORY.md`, this heading; do not silently revive retired
performance acceptance from that ledger. Ask completion proof was superseded by
Jack's October 1 removal; Task conversations retain ordinary review judgment.

LOO-185 remains parked without human-selected Discord friction. LOO-284's shared
contract and LOO-293's passive output/history/provider continuity were not completed
merely by integrating primitives. Their dated allocation and evidence are in the
same archive; adapt remaining views to current Task panes, never restore Monitor
or the retired Watch route. Historical inventory does not belong in current
navigation and must not be deleted. Earlier authorized removal of `engbot` grants
no new store-cleanup or installation authority.

## Chapter decisions and review lessons (2026-09-23)

The accepted start, not the summer draft, is frozen at
[the chapter start](../../.lf/chapters/20260923T000959Z-502f011b/start.md)
(23 September–21 October). Gate 2 accepted planning, not publication. The later
September 24 hierarchy decisions govern. Historical KR counts and dated review
mechanics remain at `b1e3f623a:wave/product/MEMORY.md`, this heading.

- Product value is human-selected external progress in any three of Cube,
  Etude, Kata and Hootro. Sessions, planning churn and Loopflow self-hosting
  alone earn no credit. Preserve external capacity; Small/Medium/Big are review
  heuristics, never runtime limits.
- Product's dogfood revealed execution work: Infrastructure now owns execution,
  Intelligence evidence, and Product the external experience. Finish validated
  foundations instead of restarting them.
- Freeze the exact Project/KR union before review; recompute totals from rows.
  Definitions and KRs are separate verdicts, and current rosters cannot prove
  historical completeness. Missing duration evidence stays unknown.
- Keep observations beside beneficiary and consequence. Carry, learned, not
  prioritized and misplaced are different findings. Review proposes; accepted
  start applies. A report plus execution settlement establishes completion.
- PM's overwritten snapshot cannot recover KR history. The start ledger is a
  forward boundary; retain later wording as dated evidence. Append-only provider
  revisions remain an option when a real review needs unavailable lineage.

## Work and continuity (reconciled 2026-09-23)

- Work identity and domain ownership follow AGENTS.md. The September 23
  prepared-Run and internal-Project account is archived at
  `0e9b9b705:wave/product/MEMORY.md`; launch attribution grants no control.
- Steers remain durable authored inputs; retired Chat/mailbox details share that archive.
- **Another Work perspective is an ordinary Run; interactive work is a
  Session.** Launch `lf --task ISSUE : <question>` when another agent perspective
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

July–August incidents: `b908182f5:wave/product/MEMORY.md`; Steer delivery remains provider-dependent.

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
- Machine-local execution, Work continuity, provider history, and direct process
  control have separate owners. The current boundary is documented in
  `docs/architecture/{execution,planning,machines}.md`; earlier topology questions
  are historical, not authority.

### Planning authority and historical migration lessons

- October 8's LOO-406 decision supersedes provider-only planning: the common local
  writer owns records/APIs, with optional Linear synchronization; Linear wins
  observed conflicts. No file mirror or Desktop plan becomes another authority.
  Current reads expose one chapter and direct Tasks; historical Projects retain
  provenance.
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

### The `lf` / Machine spine

`lf` owns commands and durable reads. Global `--machine` routes the command to
the target machine's `lf`; observation grants no execution authority. The retired Discord,
lfd and resident-cron contracts remain in
[git history](https://github.com/loopflowstudio/loopflow/blob/c418953634bd101f51878d2be2b40fb3facafabd/wave/product/MEMORY.md).

## Shared planning and runtime vocabulary

Wave → Task is public planning; Chapter/Project identity stays internal and
historical. Shared status, roadmap, cached plan and Rust/Swift fixtures change
together. AgentSession owns interactive Session/headless Run continuity and
history; Process owns an lf invocation; FlowProcess records one Flow driver's graph
and steps. A Task run may start several Flow processes. No separate Run owner is
restored because identifiers happen to coincide.

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
- Measurements: `77c9f9273:wave/product/MEMORY.md`, this heading, and
  `scripts/benchmarks/desktop-performance/`. Path normalization dominated drawing;
  reopening hid first-update launch cost. Stripping with a retained dSYM remains
  Jack's open choice; its measured gain traded away crash-frame names.
- October 7: Jack retired LOO-376's performance acceptance; LOO-408 owns its
  installed settlement. Prior cold-cache/refresh and duplicate-read observations
  remain evidence, not new optimization obligations.

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

### Task observation and retired Watch

Desktop launched two LOO-293 helpers while its worker still produced output.
Missing `lf ps` rows or a finished launcher do not prove idle execution.

LOO-293's bounded history, passive native output and configured proof remain;
use current Task panes, not the deleted Watch/Monitor view. Join history by exact
captured step, never skill name or current YAML. Earlier scope and retired
FlowPosition mechanism: `2c23acbb8:wave/product/MEMORY.md`, **Task observation and
Watch**. Integration is not acceptance.

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
- **Released migrations are immutable.** The Product/Intelligence `061` collision
  left existing databases missing a column. Test upgrades from the released frontier;
  AGENTS.md owns one draft per Task. [Incident history](../../.lf/chapters/20260923T000959Z-502f011b/sources/wave/product/MEMORY.md).
