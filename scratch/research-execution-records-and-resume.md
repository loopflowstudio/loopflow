# Research: execution records, SkillInvocation, and resume

2026-09-28 · LOO-298 · Independent research requested by Jack Heart.
Research and proposals only; no implementation or scope approval. This note does
not steer Cut I or request background landing supervision.

**Superseded proposal:** Jack subsequently selected Exec, AgentSession and
FlowSession and removed Run as a separate product object. The recommendation
below is retained as research history, not approved implementation direction.
[The working design](data-model-one-table-per.md) governs. Source observations
and measured counterexamples remain evidence within their original scope.

**Original recommendation:** give a retryable skill execution an identity distinct from
both its Runs and its conversation. If Jack wants that identity to exist inside
an ordinary Flow as well as independently, `SkillInvocation` is a child with
optional Flow membership, not a subtype of the whole Flow. Keep Flow navigation,
Session continuity, command execution, and delivery settlement with their existing
owners. The important decision is what stays the same when Jack says “continue.”

## System understanding

### Supervisor transport validation — 2026-09-28

Jack requested direct plan clarification and validation. The supervisor tested
the actual installed Codex 0.157.1 app-server using a private CODEX_HOME, an empty
working directory and a local synthetic Responses server without credentials.
Only fixture-owned processes were stopped. No real model, installed Loopflow
Home, shared provider daemon or existing conversation was used.

- Two WebSocket clients connected to one engine through a private Unix socket.
  The second resumed the same active thread and retained it after the first
  disconnected. Its `thread/read` still reported active and the engine was alive.
- Native `codex resume --remote unix://PATH THREAD` displayed the original
  synthetic prompt and Working state without a lock screen. After the TUI was
  closed, the second protocol client still observed the active turn.
- That client interrupted the original turn; it became idle while another
  thread on the same engine remained active. The engine remained alive.
- An earlier raw-JSON Unix-socket probe timed out: the transport requires a
  WebSocket Upgrade handshake. A subsequent native proxy probe also timed out;
  its cause remains unestablished. These failures are retained separately.

Reproduction and raw evidence:
`/tmp/lf-connect-plan.nrPdX2/native-ui-probe.py`, its `native-ui-probe/`
directory (`results.json`, `client-a.json`, `client-b.json`, `tui-locked.txt`
and raw PTY bytes). The inherited filename `tui-locked` does not describe the
successful remote TUI's observed state. Earlier probes and their results remain
beside these files. Run the script from a fresh directory using
`uv run --no-project --with websockets python native-ui-probe.py`.

[Official protocol documentation](https://learn.chatgpt.com/docs/app-server#protocol)
describes WebSocket framing over Unix sockets; its terminal-UI section documents
`--remote`. This is provider transport feasibility, not a implemented Loopflow
handoff or configured model/tool proof. Current Loopflow still starts a private
stdio engine. Driver ownership, event capture, nested lf provenance, restart and
provider-specific non-Codex behavior remain to implement and verify.

### Evidence boundary

Inspected HEAD: `8ccb0bde9ff84d843d4244add77986c62a59e89c`, with **unstaged Cut I
changes** and no staged changes at the initial capture. Source references below
name functions and approximate lines in the inspected working tree, not a claim
about released behavior. A private copy of 443 source/doc/test inputs, hashes,
initial Git status and diffs is under `.lf/tmp/research-execution-records/`.
Before publication HEAD was unchanged and the index still empty. Concurrent edits
changed `engine/agent.rs`, `ops/pr_landing.rs`, `harness/mod.rs`, `store/mod.rs`,
architecture data/HTML and the Cut I ledger. Reinspection of the cited agent and
landing changes found a credential-environment removal/comment cleanup and a test
isolation repair; the retry/repair paths described here remained intact. Cut I
also corrected several stale data-doc paragraphs during this research. These
edits belong to the active implementation, not this contribution.

Read AGENTS/STYLE, architecture overview/reference/execution/data docs, the main
design, Session/current-Run note, dated decisions, Infrastructure memory, H3's
post-rebase review in `cutover/cut-h-one-flow-driver.md`, integration receipt, and
Cut I's ledger including its September 28 continuation. `scratch/concept-review.md`
is absent; its decisions survive in the main design and dated decisions. The H3
review records local acceptance after two authority repairs, not whole-Task
acceptance. The main design's opening status and several architecture paragraphs
lag that review. Cut I remains another Run's work.

Only a small isolated SQLite probe ran here. No Rust build, provider, installed
Home, PM request, migration, or CLI execution proof ran. Historical 0.8–3.5 second
measurements are not a present baseline. Reported H3 tests below are retained
evidence, not tests rerun by this research.

### Current inventory: one screen

| Object/evidence | Actual writer → reader | Responsibility and duplication |
| --- | --- | --- |
| **Session** | `store/sqlite/sessions.rs::create_session`, `replace_session_run`, ready/complete/rename → SQL join in `open_sessions`, then `human_session::surface` | Stable conversation, title, feedback, current Run and history. Independent TUI, Ask and reviews have Sessions; ordinary headless skills usually do not. Native clients/history still enrich the surface. |
| **Run** | `run_record::RunCapture` and Flow reservation/publication → `store/sqlite/runs.rs`, `lf/commands/runs.rs`, usage/activity | One logical execution attempt, nullable Session/Flow/Task/Wave, outcome, provider. Includes mechanical Flow operations. SQL end and `terminal.json` duplicate lifecycle; snapshots still read artifacts. A Run can contain several provider attempts. |
| **FlowInvocation** | `store/sqlite/flows.rs` transactions + common `CliFlowExecutor` → same decoder/driver, Task projection, Session boundary | Expanded captured graph, cursor, iterations, version, claim, current Run, failure and review boundary. Task points at one managed root; other attributed invocations may coexist. Capture and navigation are not conversation identity. |
| **SkillInvocation — proposed** | No current table/type | Closest existing identity is `(invocation, node, iteration tuple)` with ordered Runs. A standalone skill lacks this logical grouping. Whether that becomes an object is Jack's new question. |
| **Exec / command journal** | `bin/lf.rs` runtime wrappers → `journal::with_runtime`, `.lf/journal/runs/<trace>/events.jsonl` and SQLite `run_events` → process/history/doctor readers | Process/command causality and start/end/escalation. `run_events.run_id` is a **TraceId**, not the product Run FK; `process_id` is ExecId. There is no `execs` main table. File and SQL duplicate command events. Exact PID/start receipt serves process authority. |
| **PrLanding** | PR request/supervision and `store/sqlite/pr_landings.rs` → foreground watcher/status/recovery | PR/head/generation/supervisor/outcome across CLI attempts. Command exit, agent success and PR merge are different facts. Jack accepted foreground supervision and manual `lf pr land` resume after interruption. |
| **Run/provider artifacts** | Capture, harness adapters and native launch hooks → inspect/replay/resume, usage reducers, client control | Immutable manifest/context/request, append-only normalized events/raw output, terminal receipt, native thread/account reference, exact client evidence. Provider-native transcript/auth stays provider-owned. These are not all immutable: prepared/native-client/session receipts remain mutable operational artifacts. |

A trace can contain multiple Execs; an Exec can drive multiple Runs; a Flow can
survive multiple Execs; a Session can survive multiple Runs; a landing can survive
multiple commands and repair agents. Those multiplicities justify separate
records. Making them one generic execution row would obscure their lifetimes.

### Data flow and counterexamples

1. **Direct skill or inline prompt.** Dispatch discovers a skill (if any),
   assembles context, routes a provider and calls `begin_run_capture`
   (`lf/commands/run.rs:794`). Headless capture stores a `RunLaunchRequest` with
   prompts, model/account and execution settings. Only independent TUI capture
   creates a Session here. Publishing artifacts and recording SQL are still
   separate steps; `RunCapture::record_row` warns on a failed row write
   (`run_record.rs:1668`), contrary to Jack's accepted fail-closed target. H4
   already owns that gap. A conflict-resolution agent uses
   `ops/mod.rs::launch_skill_agent:2085`, which captures context but does **not**
   save the same replayable launch request. “Every agent has a Run” does not
   imply “every Run can replay.”
2. **Skill inside Flow.** Task and taskless paths share
   `CliFlowExecutor::run_skill/checkpoint` (`lf/commands/flow.rs:645`). Reservation
   writes an unpublished Run at the exact position; publication and settlement
   fence invocation/version/claim/current attempt. Reviews additionally reserve
   a Session. A failed attempt stays in history; retry clears failure/current
   selection and reserves another Run at that position. A loop return changes
   the iteration tuple and is not a retry of the earlier position.
3. **Mechanical command.** Direct `lf rebase` and `lf pr land` enter a journaled
   command Exec (`bin/lf.rs` dispatch). Rebase has Git-operation evidence; landing
   has a PrLanding. A conflict launches a separate `rebase-conflicts` agent Run;
   landing repair launches `ci-fix` (`ops/pr_landing.rs:205–292`). In a Flow,
   `run_op:705` additionally reserves an operation Run, provider `loopflow`,
   skill NULL, and invokes the operation in-process. Do not invent another OS
   Exec for that in-process call or a provider conversation for the operation.
4. **Resume now.** `runs::resume_run` resolves the manifest/native thread and
   calls `util::resume_session_with_env:246` with the **original** Run ID and
   directory. It spawns the native interactive client, preserving account routing
   and exact client locking, but creates no new Run/Session row or capture handle.
   The old terminal outcome is not a new continuation outcome. Native output is
   largely inherited terminal I/O; this path does not establish fresh normalized
   continuation usage. The subprocess explicitly removes the directive-file env,
   not all ambient Flow authority (`util.rs:690`). Old identity must never be
   interpreted as renewed authority. This source finding alone does not prove a
   successful stale mutation: SQL verdict fencing rejects completed/replaced Runs.
5. **Replay now.** `lf/commands/replay.rs` requires the recorded launch request,
   pins the account, and creates a **fresh independent Run**, causally parented
   to the source Run. It reuses captured prompts, not today's skill discovery.
   Files, tools, external services and model behavior are not restored; this is
   request replay, not deterministic or safe replay of effects.
6. **Retry below the Run.** `engine/agent.rs::_launch_with_transient_retries:1190`
   may resume a native thread with a retry prompt or fail over accounts with an
   amended prompt. `RunCapture::fail_and_begin_attempt:1743` records distinct
   provider/account/usage-stream attempts inside one Run. These are neither
   identical-input executions nor new Flow attempts. Preserve this history;
   a new model must explicitly name this inner transport/provider retry level.

H3's review retained a takeover regression: recovery had settled a replacement's
attempt until claim comparison moved inside the transaction. Its stopped-worker
regression also distinguished claim position version from current cursor version.
Those are concrete reasons to reuse the common fenced driver, not create a new
skill executor beside it. Existing source tests include
`a_decision_belongs_to_the_current_attempt_and_recovery_reads_its_outcome` and
`an_interrupted_operation_blocks_for_inspection_instead_of_replaying`
(`store/sqlite/flows.rs:1331,1478`). A missing operation receipt may hide a
successful external effect; ordinary resume cannot silently repeat it.

## Tensions and minimal models

### Product distinction first

A **SkillInvocation** would mean “this requested piece of skill work, with these
captured inputs.” A **Run** is one attempt to perform it. A **Session** means
“this conversation can continue.” A **FlowInvocation** means “this captured
workflow is progressing through its graph.” Completion of any one need not close
the others. An inline prompt also needs retry/continuation despite having no
named Skill; model A must allow an anonymous skill request rather than fabricate
a catalog skill.

| Model | Shape | Cost and what can disappear |
| --- | --- | --- |
| **A. Logical skill execution, optional Flow membership — preferred** | `skill_invocations` owns identity and a pinned input reference; nullable `(flow, node, iterations)` links a Flow skill occurrence. Runs reference it; retry appends Runs. No skill cursor/return counts/worker claim. | Reopens the historical “no step occurrence object” decision deliberately. Adds one meaningful object and FK plus shared attempt reservation work. Can replace scattered position grouping and standalone retry grouping; must delete those competing writers in the same cut. |
| **B. A one-skill FlowInvocation** | Standalone skill becomes a one-node graph using existing Flow persistence/driver; Runs are its attempts. SkillInvocation is an API subtype/view, with no new table. | Least new persistence for standalone retry. Every direct launch changes membership and gains workflow machinery. Inside a multi-step Flow, giving each skill this identity requires nested one-node invocations or a separate projection. Nested wrappers add completion handoffs and conflict with “parent means runtime nesting, not composition.” Do not choose them merely for code reuse. |
| **C. Runs + conversation, no SkillInvocation** | Add explicit attempt/replay/continuation links to Runs; use existing Flow positions when present. Sessions group conversations. | Smallest initial feature for “open this old agent.” Captured-request retry grouping becomes an implicit Run chain, and Session cannot group retries that start fresh native conversations. Honest choice only if Jack does not need to inspect/manage a skill execution independently. |

**A is a child/association, not a subtype:** a multi-step Flow is not a skill,
and a skill need not have a Flow. “Subset” can describe the product relationship
without requiring a second graph, cursor or parent Flow wrapper. Template nodes
remain captured once in the Flow. The skill execution pins the **assembled
request at first execution**, which the current Flow capture alone does not do:
relaunch can otherwise rebuild context from changed files. A content-addressed
input payload can be referenced by retries; per-Run manifests still record the
actual launch, account and deviations. Do not copy the whole graph into each skill.

Minimal authority for A: keep Flow's selected attempt and claim where they are.
SkillInvocation need not store another current-Run pointer or a copied outcome:
its ordered attempt history selects the latest reserved attempt, and its result
is derived from that attempt. Reserve under one transaction, comparing the
expected prior attempt; enforce ordinal uniqueness. For a Flow member, selection
must also match the Flow's exact current position/version/claim. The same shared
attempt operations serve both launch modes; no separate traversal implementation.
Changing inputs starts a new logical execution, not a mutation of the captured
request. Mechanical nodes retain their existing Run identity and Flow position;
there is no requirement to make a mechanical `SkillInvocation`. Runs with a skill execution must agree with its
nullable Flow membership and with existing Task/Wave validators. A Flow node's
captured policy remains Flow-owned; the skill request must not copy claim tokens
or make them durable input. Domain pointers and causal links are typed, not
reconstructed from names.

This is a proposed ownership sketch, **not approved columns or an implementation
plan**. If this simple latest-attempt rule cannot express a selected product
behavior, reconsider the lifetime before adding another current pointer.

### Does every provider conversation need a Session?

Prefer a Session for every **addressable provider conversation**, including
headless work, created when the first conversation Run is reserved. This unifies
native continuation, title, history and current-Run replacement. Session existence
must not mean “waiting for Jack” or “show an open terminal”; otherwise thousands
of headless executions flood the current `open_sessions` view. Listing/filter and
closure semantics are part of that change, not a `kind=interactive` shortcut.
A failed provider start can leave a reserved Session with no native thread yet.
Account failover may create a new native thread; retain each reference on its Run
/provider-attempt evidence rather than enforcing one native thread per Session.

The smaller alternative is to attach a Session atomically on first continuation,
with uniqueness on the original Run preventing racing duplicate Sessions. It
avoids unused Session rows, but preserves two initial ownership paths and delays
title/conversation discovery. The present `create_session` constructor couples
invocation membership to Flow-review kind, so universal Sessions require revising
that validation and its consumers; adding a new kind alone does not implement it. Do not infer shared Session identity from matching
cwd, skill or native transcript text. Both policies are proposals; universal
Sessions have not been accepted by Jack.

## Proposed interactions

All new syntax in this table is **illustrative**, not present CLI functionality.
Existing `lf runs ID --resume`, `lf replay ID`, and `lf flow resume` keep the
current semantics described above until an explicit cutover.

| Intent | Proposed interaction and resulting records |
| --- | --- |
| Start a skill | Existing `lf implement : "change X"` reserves SkillInvocation A and Run 1, plus Session S under the all-conversations policy. No Task required; bare-folder operation remains possible with a writable Home. |
| Retry failed work | **Proposed** `lf skill retry A`: identical pinned request, Run 2 under A, old outcome preserved. Default fresh execution; continuing a partial native thread is a distinct choice because it changes input. Provider/account changes are reported, not called exact replay. |
| Continue finished work with feedback | **Proposed** `lf runs R --continue : "also handle Y"`: new logical skill execution B linked causally to R/A, new Run, same Session/native thread where supported. A remains completed. This is the preferred default, pending Jack's answer below. |
| Open interactively or continue headlessly | **Proposed** `lf runs R --continue --tui` or `--batch : "feedback"`. Both reserve a new Run and capture new input/output/account/usage before attaching the native provider. Opening an already-live client is attachment, not an additional execution. Historical-Run selection names the current Session attempt rather than silently replacing it. |
| Retry the current failed Flow step | Existing Flow/Task resume policy remains the entry point. It authorizes one new attempt at the current position. A historical completed step continued from history runs independently with origin links, never reacquires its former claim or advances today's cursor. Explicit Flow restart/navigation is a different action. |
| Replay an old request | Existing `lf replay R` remains a new execution with saved launch inputs, never a promise to restore world state. Require readable complete input evidence and identify effects that may repeat. |
| Inspect a command | **Proposed** `lf exec show E`: verb/arguments, cwd, start/end/result, child agent Runs, output availability and landing/rebase link. `lf pr land` itself resumes its retained delivery operation; generic “retry command” cannot decide whether a push/merge already happened. |
| Search | **Proposed** `lf runs --task X --outcome failed --limit 50 --after TOKEN`; `lf exec list --command 'pr land'`; `lf search 'constraint failed' --task X`. Search returns typed references, not a new union execution owner. |

Every continuation drops inherited Run/Flow/claim capability, then receives fresh
launch context. Attribution and causal links survive; execution authority does
not. Resume must preserve native account identity and exact live-client checks.
Neither missing output nor an old PID authorizes replacement. Failed/interrupted
operations require their domain reconciliation before any repeat. Session Ready
and Complete remain conversation/review facts, not generic “Run succeeded.”

## Observations: logging and fast reads

### What “everything” can honestly cover

Proposed baseline: every accepted work launch, attempt, command operation,
continuation, navigation outcome and externally observed delivery outcome gets
identity, causal links, timestamps and truthful completion/missingness. Capture
sanitized command metadata and output/transcript availability separately. This
covers work mediated by `lf`, not every shell command on the machine or output
lost before a process could persist it. A killed process can have a start and no
end. Preserve that uncertainty.

Current command events contain argv and error, **not command stdout/stderr**.
Several cheap reads (`runs`, usage, status, roadmap, ps) bypass runtime journaling;
install dispatch intentionally precedes ordinary Home/store authorization.
Adding unconditional logging at `main` would change bootstrap/recovery semantics
and make status polling write traffic. `journal::try_emit` currently appends the
repo file before inserting SQL; a file-write error can skip the SQL insert.
`open_ledger` opens/validates the store on each event. No production caller of
`journal::read_events` was found outside that module's tests; SQL readers and
exact process receipts remain consumed. These are specific duplication and cost
candidates, not a reason to delete Exec identity.

**Proposed normal owner:** an `execs` row for each real CLI process, with stable
trace/parent Exec, nullable launching Run, typed command family, sanitized display
arguments, cwd/optional repo, status/times, and references to domain operations.
`run_events` remains append-only transition evidence keyed by Exec with unique
sequence, not another owner of mutable command state. Rename its misleading
TraceId column during a forward migration. Run→Exec causal association does not
replace either Run caller ancestry or Task claims. Multiple commands can refer
to the same PrLanding; one Exec may create many Runs. Do not store a competing
Task fact on Exec when it can be obtained from a recorded causal Run.

Update an Exec's state and append its terminal event in one SQL transaction.
Retain kernel/process receipts for exact signaling. Capture command output as
bounded append-only payload files per Exec, with stream ordering and truncation
or unavailable markers. Preserve terminal behavior; teeing a TTY/PTY is more
work than capturing a batch pipe and must not be claimed complete from metadata
coverage. Run transcripts remain Run payload; do not copy them into Exec output
and full-text indexes repeatedly.

**No-store boundary needs an explicit policy.** Jack already selected refusing
agent launches without their Run row. That does not authorize breaking `install`,
`--help`, schema diagnosis, or a read-only command on an absent/corrupt store.
Prefer allowing these recovery/read operations, using existing installer receipts
where applicable and reporting logging unavailability without recursive logging.
With every durable destination unavailable, completion plus durable logging is
impossible. If Jack requires an audit of *every* CLI request, including those
reads, a Home-local append-only command receipt outside the mutable DB is needed,
with a rebuildable SQL index and a single declared authority. That has filesystem
writes, retention and recovery costs; it deliberately retains a journal and cannot
also be sold as deleting all sidecars. Do not quietly add that second policy.
Default recommendation: log work, keep observation reads read-only. Never log the
logger's own reads/writes or require a resident to drain events.

### Actual bottlenecks and a small separating experiment

- `SqliteStore::runs:367` has optional-filter OR predicates, correlated label
  subqueries, `(created_at >= since OR ended_at >= since)`, ordering, **no LIMIT**.
  Task/caller history is unbounded. Generic listings cap at 50 in Rust only after
  hydration (`runs.rs:70`). Existing indexes cover Session/Task/Wave/invocation/
  caller history and position attempts, but not global recent/outcome search.
- `run_record::run_snapshots:474` opens each selected manifest, validates context,
  reads terminal evidence and reduces its entire event stream for usage. Missing
  artifact directories silently drop a SQL Run; corruption can fail the whole
  list. Exact/prefix lookup still enumerates Run directories
  (`resolve_manifest:549`). `FLOW_SELECT` deserializes full captures on detailed
  Flow reads. Those costs remain despite having tables.
- Session inventory is already a SQL current-Run join. Its surface still needs
  live-client/history and review policy observations. Do not replace this with
  another manifest inventory or promise zero filesystem work for live actions.

The private probe used SQLite **3.50.4**, a simplified schema with the actual Run
columns/indexes and the exact current list predicate/SELECT: **100,000 Runs,
1,000 Tasks, 100 Runs per Task**, short metadata, no transcripts or concurrent
writers. Twenty warm repetitions followed the first query on a new connection.
Database creation had already warmed OS caches; **no result is disk-cold**.

| Query | Plan | Returned rows | First / warm median ms |
| --- | --- | --- | --- |
| Current Task selector | `SCAN runs`, temporary sort | 100 | 6.64 / 6.53 |
| Resolved Task id, explicit equality, LIMIT 50 | existing `run_task_history` search | 50 | 0.135 / 0.068 |
| Current recent selection | `SCAN runs`, temporary sort | 5,001 | 16.91 / 17.08 |
| Recent-start page with proposed partial published/time index | indexed range | 50 | 0.511 / 0.064 |

This distinguishes query shapes; it is **not an end-to-end speedup measurement**.
The page queries omit label joins and return fewer rows by design. Recent-start
is not equivalent to activity's start-or-end window; activity must retain both.
Scripts/results: `.lf/tmp/research-execution-records/query_probe.py` and
`query-probe.json`. No production schema or payload was used.

### Proposed read/search path

1. Resolve public Task/Wave/project selectors once. Build explicit parameterized
   predicates using IDs. Page in SQL before artifact access, using a stable
   `(created_at,id)` keyset and upper-bound token; never OFFSET through all history.
   Keep activity's end-window query distinct (indexed start/end ranges merged and
   deduplicated), including old Runs ending in the window. Reservations remain
   distinguishable from published launches and count for Started.
2. Return row metadata with explicit missing/corrupt-payload status. Exact IDs use
   the primary key; prefixes use a bounded indexed range with ambiguity reporting.
   Detail/`--events`/`--final` opens only requested payloads. Add a global published
   history index; add outcome/command composites only for demonstrated query plans.
   Task/Wave history indexes can already supply ordered pages. Label enrichment
   joins only the page. No ordinary read repairs/imports missing rows or opens a
   migration writer merely to list history.
3. Materialize usage totals and coverage from normalized events at capture/settle,
   with a retained sequence/offset and idempotent reducer. Preserve absent, partial
   and observed-zero semantics and inner provider-account attempts. This is a
   rebuildable summary of payload, never a second usage receipt. Terminal row and
   summary checkpoint commit together when possible; readers expose lag otherwise.
   Explicit recovery owns reconciliation after file/SQL interruption (H4 overlap).
4. Optional FTS5 indexes **normalized text chunks**: request/feedback/final answer,
   selected diagnostics and transcript excerpts, with owner id, event range and
   payload digest/checkpoint. Filter by indexed owner metadata, then rank/page hits;
   fetch context lazily. Payload remains authoritative and rebuildable indexing
   runs in the foreground on append/settle or an explicit reindex, not a daemon.
   Store capture/index coverage so missing history is visible in search results.
   Token search, substring search and raw-byte grep are different promises.
5. Capture allowlisted metadata, never the environment, credentials, account-home
   files or unsanitized argv. Free text/output can contain secrets too: reuse the
   existing normalization/redaction boundary and exclude raw provider protocol/auth
   material from searchable text. Retain non-secret account IDs for history, not
   credential bytes. No telemetry or new credential access. Specify output
   retention separately from permanent metadata; truncation is never “empty.”

Cold-start proof must measure executable startup, read-only store open, selector
resolution, SQL and rendering separately. A fast in-process SQLite query says
nothing about a CLI that initializes unrelated services or scans transcripts.
Use one connection per command and bounded batched append writes; high-volume
status polling should remain zero-write and should not replay migrations or
normalize provider history. None of these speed improvements requires SkillInvocation.

## Preservation, deletion, and smallest proof

### Migration and deletion order — proposed

1. Ratify lifetimes and logging coverage first. Finish the existing H4 ownership
   obligations through its selected work; this note neither orders nor expands
   Cut I. Snapshot/rehearse any eventual real-Home conversion only through the
   existing approved maintenance/promotion process, never this branch binary.
2. Preserve all Run IDs, terminal outcomes, usage streams, account routing,
   Sessions/current pointers, captured definitions, claims and process identities.
   Backfill SkillInvocations only from provable Flow position tuples or a complete
   standalone request. A missing request gets explicit unavailable retry, not
   rediscovery from today's Skill. Do not guess native conversation equivalence,
   missing invocation membership or historical end times. Old same-ID native
   resumes cannot be retroactively split into fictitious Run attempts; label their
   continuation accounting unresolved. Preserve corrupt/raw inputs in the import
   report without making ordinary readers depend on them.
3. Add linkage and new writes together with one reader conversion. Retry reserves
   its new Run/ordinal/current Session selection under CAS and exact Flow fences
   in one transaction. Publish immutable request/context before provider effects,
   and reconcile exact prepared bytes after interruption. Completion never rewrites
   an earlier attempt. Filesystem/provider effects cannot join a SQLite transaction;
   preserve the uncertain interval rather than assert exactly-once execution.
4. Delete old-ID continuation launch, directory-prefix resolution, eager list
   hydration and duplicate grouping after their replacements pass. Remove the
   repo command-journal writer only after deciding no-store logging and checking
   external/debug consumers; retain `run_events`/exact receipts while control
   reads them. SQL terminal ownership removes lifecycle fallback, not immutable
   launch/terminal evidence. Retire import-only parsers after actual Homes are
   accounted for. No line-count credit for moving code or adding wrappers.

### Smallest observable proof, then boundaries

One isolated real-CLI fixture: run a standalone skill with a stand-in provider,
fail once, change/delete the Skill source and context files, retry with the pinned
request, then continue the successful native thread with explicit feedback. Assert
stable skill execution across retry, new execution across continuation under the
recommended policy, separate immutable Run outcomes and usage, one Session/current
member, exact account/native linkage and fresh authority. Change a Flow's cursor
and claim while opening an old member: the continuation must not submit a verdict,
change the cursor or complete its old review. Reuse H3 takeover/stop regressions.

Add only proofs that distinguish remaining boundaries:

- Task-owned and taskless members, anonymous inline request, repeated loop position,
  and simultaneous retries: one selected new attempt, no duplicate provider effect.
- Direct mechanical operation versus Flow operation: both inspectable, no Session;
  crash after effect/before receipt requires domain inspection, and manual landing
  resume retains its PrLanding. Repair agents have distinct Runs and causal links.
- Completed/interrupted historical records, unavailable native account/history,
  corrupt or missing payload, failed SQL end write and partial import: retain visible
  metadata and missingness; never turn unknown into completion or reexecute blindly.
- Normal folder/no initialized repo, absent/corrupt/unwritable Home, install/help/
  status and repeated cheap reads: enforce the selected logging policy and confirm
  no recursive events, accidental initialization or secret-bearing output capture.
- Density pages and text index: 100k metadata rows plus representative large streams,
  first-process and repeated warm reads, concurrent writer, stable pagination without
  duplicates, index rebuild yielding the same covered hits, missing coverage shown.
  Measure payload opens and writes as well as latency. The small probe here proves
  none of this integrated behavior.

## Open questions for Jack

1. **After “continue” on a successful skill, is it the same skill execution or new
   work in the same conversation?** Recommendation: new SkillInvocation, same
   Session, new Run. Retry preserves the execution and input; continuation preserves
   the conversation and accepts new input. This choice determines the lifetime.
2. **Must a skill inside a multi-step Flow be independently addressable/retriable?**
   If yes, choose A and deliberately reopen “no step occurrence object.” If the
   requirement is only standalone retry, B is substantially cheaper. Historical
   continuation cannot silently rewind the enclosing Flow in either model.
3. **Does “everything logged” include every observation command and its output?**
   Recommendation: complete work/attempt metadata and captured output coverage,
   read-only observation commands, honest no-store recovery gaps. Universal command
   auditing requires the separate append-log policy and its retention cost.
4. **Are headless conversations Sessions from birth?** Recommendation: yes, with
   history/attention/terminal visibility separated. Lazy creation on first
   continuation is smaller but leaves two conversation ownership paths.

Implementation assumptions, not Jack's decisions: native continuation is attempted
only where the provider/account supports it; unavailable history is shown explicitly.
An active client attaches without minting a false attempt. Fresh retries default to
the pinned request; automatic transport retries retain their inner event history.
Search is local to the selected Home with explicit coverage, not cross-Home telemetry.
No new background worker, Work assignment, Task, or delivery requirement follows.

## Review conclusion

The source supports a useful SkillInvocation **only if it names durable work
across attempts**. A new name around the current Run or a nested one-node Flow
would leave the continuation and logging defects untouched. The highest-confidence
reductions are independent of that choice: SQL metadata pages, exact SQL lookup,
new Run identity for continuation, and one clear command-evidence owner. Their
acceptance must preserve the H3 claim fences and truthful missing evidence.
