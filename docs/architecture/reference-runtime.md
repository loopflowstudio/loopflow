# Launch, Graphs and delivery

Part of the [Architecture Reference](../architecture-reference.md).

## Harness launch and Session captures

The execution cutover uses one LfSession admission and capture path for Task,
Wave, helper and direct callers.

SQLite owns Process history. Repository trace events live in
`.lf/journal/traces/<trace-id>/events.jsonl` and name their `trace_id` and `process`
node explicitly. Session captures retain the published `~/.lf/runs` directory
as one opaque physical encoding, selected through SQLite artifact keys. Current
and historical captures use the same root: there is no relocation, parallel
layout, alias or privileged conversion. Missing payload does not erase a resumable
Session's SQLite identity. `LF_CAPTURE_KEY` selects subordinate history;
Session/Process caller provenance supplies ancestry and mutation authority.


1. Admit the actual lf Process; resolve typed work without granting Graph authority.
2. Reserve the LfSession and its initial history/capture reference before
   provider launch. Claim its attachment and record exact publication state.
3. Publish immutable input atomically. An unpublished reservation is recoverable;
   uncertain publication/spawn evidence never permits a blind duplicate launch.
4. Start the AgentProcess, or connect to the one a live attachment holds. Record its
   identity and endpoint, distinct from the client's process and the attached LfProcess.
5. Append correlated provider outcomes and usage; retain missingness. The
   Graph process reads a deciding or routing step's answer from the turn that step's
   Process captured.
6. Settle the actual command's Process when the process completes, independently of
   whether its conversation remains open.

### Retained encoding inventory (LOO-370)

- `session_record::record_dir`, active-reader watches, ablation staging and
  preservation fixtures retain `runs/<shard>/<artifact-key>`. This is the single
  physical capture root; keys (including historical `run_` strings) are immutable.
- `ops/git_operation.rs` keeps serialized `run_id` / `lf_process_id` receipt fields
  while Rust names their actual Trace/Process owners. Released SQL, migration
  fixtures and `session_events` historical source labels retain their original
  bytes. Historical cohort readers in `scripts/context_ablation.py` still decode
  `launch`, `run_id` and `parent_run_id` from their frozen inputs.
- `LF_RUN_ID` / `LF_RUN_DIR` occur only in launch scrubbing, rejection fixtures
  and the checksum-pinned released-CLI preservation fixture. Current execution
  reads `LF_CAPTURE_KEY` and typed Session/Process provenance. Capture context does
  not confer Graph or Task settlement authority.
- GitHub workflow/check runs, release-run operations, gate execution receipts,
  launchd `RunAtLoad`, Swift attributed-text runs and ordinary execution verbs
  name other things. Published release notes, dated benchmark reports and chapter
  archives retain historical terminology. The old heading anchor above preserves
  documentation links, not a runtime interface.
- Current Wave JSON exposes `history: Evidence<SessionHistory>`; Rust, Swift and
  `wave_detail.json` share the required field without a fallback. Telemetry uses
  Session metric IDs and labels; ablation/check-cost use capture names. Session
  commands select durable Session/native conversation IDs; monitor's `--input`
  and replay select retained captures.

Production source against `8ea0bec9cf4b0c08ca17c52e57de059000a7b0e3`:
Rust **+545 / −659**, Swift **+64 / −64**, Python package **+0 / −0**,
scripts **+97 / −81** (net **−98** lines). Physical-line comparison includes
comments/blanks, excludes tests/fixtures, SQL, builtin prose and generated output;
Rust test-only attributed items are removed using its syntax tree. Rename pairs
are compared as one file, so moves do not count as deletion. This measures source,
not installed acceptance. Abandoned relocation probes survive in Git at
`6fcdbe9da0b47ef95f1f92ebdb259401a327cd46`; their contrary evidence remains valid.

Payloads may remain large immutable files. SQLite owns identity, attribution,
current control and searchable history. Current capture payloads may use files; ordinary readers select their exact
artifact through SQL, never by scanning retired layouts.
The retained operational process evidence is not a new lifecycle object.

## Tracked Work and Task Graphs

```text
lf task run ISSUE [GRAPH] -> place the worktree, take up GRAPH, the Wave's, the repository's or `code`
                         -> lf --task ISSUE graph run as a child: one lf process,
                            started again while a Graph process fails (three at most)
                         -> step processes, one child process each
                         -> current, completed or stopped
```

Every Graph naming a Task is equally its work; none is selected over
another. `task run` returns to its launcher once its Graph process registers,
leaving output in `logs/graph-<id>.log`; `-b` blocks and prints. The Graph process holds the cursor in memory and
advances it from each step's exit; the position a step ran at is on that step
Process's argv.

A Graph process that dies leaves dead Processes as history. Nothing restarts or resumes
it. Liveness comes from OS process evidence, and missing evidence stays uncertain.
Only live or unknown Processes retain checkouts and exact controls; a provider
turn without a completion is history. Execution does not veto authorized Task
completion or cancellation. Git/PR delivery still needs its own evidence. Fresh
work starts another Graph after inspecting existing execution.

Independent helpers may carry the same Task and independent Git/PR
operations. Binding to done Work assigns a conversation without reopening it.
Legacy review boundaries remain readable history when their Work is terminal.

Chapter rotation preserves Task, LfSession, PR and worktree
identity when moving started unfinished Tasks. Local selection, membership and
pending effects commit atomically across the selected Waves. Provider delivery
converges independently; there is no cross-system atomic switch.

## Durable communication

```bash
lf comment INF-123 "keep the public name" --steer
lf --wave infrastructure session steer "preserve native history"
lf session steer "retain source attribution"
lf session steer SESSION "inspect the last failure"
lf session steers SESSION --json --limit 32
lf session steers SESSION --after 42 --json
lf session steers SESSION --source STEER_ID
lf session steers SESSION --retry STEER_ID
```

Steers retain immutable source revisions and authorship. Task comments and
definition acquisition save steers in their existing planning transaction; their
Linear delivery receipts remain separate. Repository and Wave steers are
Machine-local and report unavailable shared planning exchange for those scopes.

Each Session selects its steers from its repository, Task association and
Wave ancestors. Peers receive independent receipts. Saving a steer starts no turn;
an idle Session receives pending steers on its next supported provider boundary.
Startup, user continuation, tools and compaction share the receipt path. Task
brief references contain no consumed steers.

The provider capability belongs to the selected AgentProcess and native
conversation, survives client attachment handoff and fails after replacement.
Child callbacks cannot consume parent steers. Shared Session dispatch exclusion
lets startup hooks finish while native stdin is blocked; takeover remains exclusive.

A boundary reserves at most eight steers and renders at most 10,000 bytes
including reference context. Oversized text has a marked UTF-8 excerpt and a
complete immutable source. Only the selected page is materialized.
Receipts distinguish pending, prepared, uncertain and emitted/unconfirmed.
Successful stdout proves local emission only. No current adapter manufactures
confirmed acceptance. Explicit retry creates another attempt without waking work.
An exclusive, nonblocking Session fence proves a prepared emitter has exited
before its reservation is cancelled; elapsed time grants no retry authority.

OpenCode gives each context hook an adapter invocation UUID. Internal stream retries
reuse that model request; later hooks add fresh system context
and do not reselect emitted steers. Providers without an observed authenticated
callback report that limitation; already-open providers without declarations need
a provider start.

### Questions and sessions

These examples use current command spellings; the lifecycle contract is above.

```bash
lf -b implement
lf session list --interactive false --task INF-123 --json
lf session connect SESSION
lf session rename SESSION "Migration review"
lf session bind SESSION --task INF-123
```

Every conversation has one LfSession regardless of launch surface. Connect
uses the existing AgentProcess where possible; client replacement retains its active
turn. Name, feedback, native identity and history survive reconnect and recovery.
A suggested title cannot overwrite a human-assigned title. CLI and Desktop use
the same action and availability reason.

Ready saves feedback and keeps the conversation open. Complete persists its
closed state and exact feedback before teardown. A Graph review returns
feedback for the following decision; it never selects that decision's edge.
Pane close, provider exit and readiness do not complete the review.

The desktop terminal pool keys on stable LfSession identity. Rename, bind,
reconnect and replacement retain the pane and draft when reusing that surface.
Recovery after AgentProcess exit does not claim preservation of text never submitted
to Loopflow without separate UI evidence.

## Graph execution

```bash
lf task run INF-123                  # Project's Graph
lf task run INF-124 incident  # explicit override
lf graph example                    # with or without Task attribution
```

The Graph process compiles the graph, every router and Xor alternative, and holds it
with the cursor and return counts in memory. Definition changes affect new
Graphs. A past Graph keeps the graph its Graph process recorded at launch. Backward edges stay finite graph structure; future loop passes are
not preallocated. Node IDs are local typed identities, not path strings.

Taking an Iterate edge updates the Graph process's cursor and return counters. The
node key and per-edge iterations on each GraphProcess step row identify the pass,
including overlapping backward edges. Neither loop passes nor authored subgraphs
create Graphs or lifecycles.

Task and taskless execution use one Graph runner and need no synthetic
planning records. A migration archives each earlier Graph row's name,
state and last cursor as a `legacy_flow` observation on the Sessions it opened,
then drops the Graph tables and the Session's Graph column. Configured acceptance
and the machine's current-state cutover remain separate from schema verification.

A decision returns Advance, Iterate or Blocked in the final answer of the turn
its step Process captured. Blocked records its reason and stops at the current
position. Only Advance and Iterate move the cursor. Failure or interruption
cannot submit a verdict. There is no file-backed cursor. Recovery of an
uncertain mechanical effect still requires inspection; cursor movement alone
cannot establish exactly-once external effects.

Finite Wave planning uses ordinary attributed agent conversations. Historical
Wave continuations remain preservation evidence and confer no new execution or
process-control authority.

## Task delivery algorithm

A Task binds planning to one managed checkout and one current PR, when published. Placement owns
its branch and base independently of publication; a research Task needs no PR.
A checkout on that branch identifies the Task regardless of its upstream; the
stored path supplies placement for explicit Task selection. Publishing continued work after a PR settles retains the previous PR as read-only
history and selects the new PR. Recommend a follow-up Task for a distinct outcome;
continuing in the same Task remains supported.

```text
Linear Issue
    |
    v
Task row ----> managed worktree ----> commits
    |                                  |
    |                                  v
    +-----------------------------> GitHub PR
                                       |
                              checks / repair / merge
                                       |
                         optional ordinary Task creation
                                       |
                              complete Task
```

1. `lf checkout` resolves one Linear Issue inside one Project and creates
   or reuses Task Work and its checkout placement without creating a PR. It starts no
   agent work.
2. Independent `lf --task ...` conversations may work in that substrate directly.
   `lf task run` places the Task, defaults its Graph, then runs it through the
   same `lf graph` path as any other launch.
3. `lf commit` snapshots the worktree. `lf pr publish` creates or refreshes the
   current PR without opening a browser.
4. `lf submit` leaves the exact-head merge click to a person. `lf arm`
   and `lf land` request exact-head auto-merge, record the landing, and
   return; `lf pr reconcile` checks recorded landings once. All operate on Task delivery state when it exists and require
   no Graph-driving claim or execution receipt.
5. PR landing is fenced by landing generation. A check repairs an unchanged
   incident once; a moved head requires fresh evidence. A blocked landing still
   observes merge, and explicit arm or land resumes it.
6. Verified merge alone leaves an open Task available for optional new Task creation.
   No follow-up disposition is required. `ship` runs gate and `land --wait-and-fix`;
   reaching `end` requests completion. A stopped Graph leaves the operator
   to inspect live work and retry completion alone when the outcome is satisfied.
7. `task complete` changes status without moving the Task's graph. Reaching end
   atomically requests completion; a failure retains arrival and retries only
   completion. Accepted Linear completion/reopening leaves execution intact and
   supersedes old completion intent. Ordinary reads execute no trigger. A PR-less Task
   may finish with retained artifacts; a Task with a PR needs authoritative
   merge. Dependent PRs belong to stacked Tasks.

GitHub remains merge truth. SQLite stores the observed PR/head/check/disposition
needed to resume safely; it cannot declare an unmerged PR merged.
