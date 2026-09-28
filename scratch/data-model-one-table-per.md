# One SQLite owner per product object

LOO-298 · Jack Heart · Current implementation plan · 2026-09-28

Jack requested direct ownership of plan clarification and validation. This is the
current plan, reconciled against `71496d5da4f729ffc34ca1f9881e9598a5e67606`.
Accepted product decisions, implementation choices and observed proof are
separated below. This document supersedes the old Run-based H4–H6 plan.
The previous full design and dated implementation ledger remain in
[the checkpoint](https://github.com/loopflowstudio/loopflow/blob/71496d5da4f729ffc34ca1f9881e9598a5e67606/scratch/data-model-one-table-per.md).
The commit exists locally; remote availability was not checked.

## Finish line and present state

The requested stop is a **code-complete concept review**: the whole accepted
model implemented, affected checks passing on the integrated code, and a
reviewable account of usage, owners, deletion and remaining deployment evidence.
A plan, new tables, renamed types, one passing slice or a contributor's conclusion
alone does not reach that stop. Publication, installation and real-Home conversion
are separate actions; no branch binary may touch the installed Home.
Jack authorized publishing the next verified checkpoint on 2026-09-28 so
dependent Tasks could use it. That checkpoint is published as `e13f29909` on
PR #1296, based on `c512813b5`; the remote branch, GitHub head and Task record
were independently checked. This supersedes the earlier local-only hold for
that publication, without claiming code completion or authorizing landing,
installation or live-Home conversion. The saved 13-step
feature invocation still contains implement → compress → review-slice →
concept-review → loop-decide, then a human demo boundary. Preserve its captured
order; a partial implementation's concept review is not the requested finish.

H1–H3 are implemented and reviewed: Task selects its managed invocation; all
Task-attributed Flows retain attribution; Task and taskless Flows share a row
owner and executor. Preserve that work. Cut I is reviewed at `2d597800e`:
listeners, residents and lfd are deleted. The canonical fleet proof preserved
all 1,020 receipts in 4.540 seconds; architecture, formatting and all-target
Clippy passed. The draft build was interrupted after 376.234 seconds, with
repeated expected-schema reconstruction observed on database opens. Repair that
development performance path without weakening schema validation. Retain the
inherited unwritable-store counterexample for the new writer contract. This is
local Cut I acceptance, not a green full gate or installed acceptance.

The new execution model replaces H4–H6. H7 remains in this Task, governed by
[Chapters](chapters.md), which Jack accepted on 2026-09-27. The old proposal
for a chapters table, packet, config line and chapter history API is superseded.

## Accepted product model

| Object | Meaning | Lifetime and owner |
| --- | --- | --- |
| Exec | One actual `lf` process started and exited | One durable `execs` row per process; many commands/steps may execute inside it |
| AgentSession | One agent conversation that can continue | Durable conversation row; interactive or headless, with stable name, feedback and native conversation identity |
| FlowSession | One started Flow that can continue | Evolve the existing invocation row: captured graph, cursor, return counts, current boundary, claim and completion |
| Agent process | Actual provider engine/process | Exact native identity and OS evidence; can outlive a client and may serve more than one conversation |

The physical product owners are `execs`, `agent_sessions` and `flow_sessions`.
Evolve/rename the existing Session and invocation tables instead of maintaining
old and new runtime owners together. History is subordinate to its Session;
history tables or journal entries do not become another public lifecycle.

No separate Request, Execution, SkillInvocation, AgentExec, SkillExec or replacement
Run product object. Jack confirmed on 2026-09-28 that agent outcomes belong in
AgentSession history and preferred fewer objects. History entries have stable
references; they do not acquire an independent resumable lifecycle.
Flow and Skill remain reusable definitions. AgentSession covers skills, inline
prompts, reviews, Asks and helper agents. `interactive` is independent of purpose;
Ask/review completion semantics remain distinct. Default conversation views show
interactive sessions; explicit filters expose headless and completed sessions.
`--all` retains its existing all-repositories meaning.

An AgentSession has at most one authoritative driver at a time. Its creator is
historical; its current driver may change. A saved conversation may have no live
driver or engine. A dead driver does not establish a dead provider. Viewing is
passive. Connecting does not complete a review or advance a Flow.

A Task selects one managed FlowSession and may have other attributed FlowSessions.
Taskless Flows use the same driver and record. A FlowSession's captured template
composition stays expanded; only actual runtime nesting creates child sessions.

## Usage to implement

```sh
lf -b implement                          # records a headless AgentSession
lf session list --interactive false --task LOO-298 --json
lf session connect SESSION               # use live connection, otherwise resume
lf session connect SESSION --restart     # explicit replacement, same conversation
lf session rename SESSION 'Parser review'
lf session bind SESSION --task LOO-298    # write once; same target is a no-op
lf flow example                          # creates a FlowSession, with or without Task
lf flow resume FLOW_SESSION              # captured progress, shared driver
```

New flag spelling above is an implementation choice, not a claim of current CLI
support. Existing `open`/desktop surfaces must converge on the same connect
operation; do not retain two lifecycle implementations. CLI/API naming and
consumer migration happen together. Default UI visibility never decides whether
a conversation is recorded or resumable.

Normal path: start a headless agent, find its Session, connect from Desktop or
terminal, rename and bind it, then reconnect after its lf process exits. Keep the
conversation and terminal continuity where the same surface is reused. Replacing
the engine preserves persisted conversation; do not claim preservation of
unsubmitted editor text without a separate UI proof.

Recovery: `connect` uses the existing engine when it can. `--restart` interrupts
and releases the exact conversation owner, then resumes its native identity.
Graceful stop precedes force termination of an exclusively owned process. Never
kill a shared engine to restart one conversation. Ambiguous owner evidence stays
unresolved; do not start a competing writer merely because an observer timed out.

## Exec ancestry is a causal lf tree

```text
Exec A
├── Exec B   via_agent=false        direct lf → lf
└── Exec C   via_agent=true, S      agent in AgentSession S invoked lf
    └── Exec D   via_agent=true, T  agent in AgentSession T invoked lf
```

Each child stores `parent_exec_id` and the incoming `via_agent` bit; agent-issued
commands also retain their calling AgentSession. The bit describes the caller,
not whether the child launches an agent. Provider and shell processes do not
become synthetic Execs. Root provenance may be unknown for imported history;
unknown is not silently encoded as false.

Direct children take the invoking Exec. Agent-issued children carry stable
Session identity and provider ownership generation, then resolve the matching
current driver when dispatching. A replaced provider's delayed command must not
be attributed to the replacement merely because it names the same Session.
Frozen parent
IDs in a provider's launch environment cannot correctly describe a later driver
handoff. Resolve and record the accepted parent once at child admission; never
rewrite an existing child's parent after a handoff. If no current driver exists,
retain the proven historical owner/caller or explicit missingness, not a guessed
live parent. Causal ancestry grants no signal or Flow-settlement authority.

### Exec completion is general

Every Exec records its command's terminal outcome and end time, with exit code
or signal when observed, whether it drove an agent, a Flow or a plain command.
The lifecycle covers succeeded, failed and interrupted commands. Terminal
fields remain absent where no trustworthy completion was observed; process
disappearance alone cannot manufacture an exit code, signal or completion time.
The existing journal already emits command completion/error events; evolve that
writer into the indexed Exec owner rather than inventing a second result path.
An absent terminal receipt remains unknown and is not proof that the process is
running. Exact process observation and command outcome answer different questions.

AgentSession history references the driving Exec. Several Execs can continue
one conversation; a Flow-driving Exec may also drive several conversations, so
do not assume one nullable Session FK on Exec represents every launch path.
Reuse Exec's outcome when displaying the command result; do not copy it into a
competing result field. Provider completion is a different fact even in a
one-call Exec: the provider may finish and a later command operation fail.
AgentSession history retains that provider completion and references its driving
Exec. For several calls or steps in one Exec, correlate their individual
completion entries in Session/Flow history. A successful
Exec that parks a Flow at review does not mean the Flow is complete; a resumed
conversation does not reopen or change a completed Exec.

## Facts that must survive deleting Run

This is the concrete ownership plan, not a Run-to-Exec rename.

| Existing Run fact | Destination and invariant |
| --- | --- |
| Command process identity, exit, parent | Exec row; one row represents one real lf process |
| Conversation identity, title, native thread, request/feedback | AgentSession; never copied on reconnect/restart |
| Current conversational driver | Nullable Exec reference plus a generation fence on AgentSession; old driver loses write authority atomically |
| Provider PID/start and endpoint | Existing native process/connection evidence linked to AgentSession; engine and terminal client remain distinguishable |
| Captured input and launch publication | AgentSession launch metadata plus immutable payload; reservation/publication comparison prevents two launches |
| Flow graph/cursor/claim and pending review | Existing invocation owner evolved to FlowSession; one shared driver |
| Agent work start, completion, failure, retries, duration and usage | AgentSession history, correlated to provider turn/receipt and driving Exec; idempotent receipt acceptance, missing distinct from zero |
| Agent-backed Flow step completion | FlowSession references the exact successful AgentSession history entry, validated against its selected boundary and generation |
| Mechanical Flow step start and outcome | FlowSession history, correlated by node, iteration tuple and generation; no invented AgentSession or child Exec |
| Task/Wave and provenance | Typed Session/FlowSession ancestry plus immutable event attribution; Exec command context remains distinguishable from work performed |
| Prior Run IDs and historical outcomes | One-time import mapping/evidence; preserve exact known relationships and explicitly unknown ones |

Reuse the existing event journal and event infrastructure. Do not add a generic
attempt table, object hierarchy or public lifecycle merely to recreate Run.
An event sequence/correlation key is necessary evidence, not a new resumable object.
A Flow boundary can run in-process with no child Exec and no AgentSession.

One Exec can complete step A then fail in step B. Its process exit cannot be the
outcome of both steps. Persist each boundary's start before external work and its
outcome afterward; settlement compares current FlowSession/version/generation and
selected Session where present. Old successes, late provider output and helper
Sessions cannot settle the current boundary. Retries retain earlier outcomes.
An interrupted mechanical operation without a completion receipt remains uncertain
until its effect is inspected or an explicit retry is requested. Preserve the
current operation recovery behavior; do not manufacture exactly-once effects.

AgentSession lifetime is not one model turn. Provider turns and routing retries
within it retain separate usage/outcome events. Flow retry and ordinary conversation
continuation must not silently become each other's completion authority.

Concrete recovery proof: a Flow step starts AgentSession S under Exec A; the
agent fails, recording failure in S's history. Resume under Exec B retains S
and appends a successful completion. The FlowSession references that exact
completion when advancing once. Both outcomes and their usage remain visible.
Later continuation of S does not rewrite the completion already consumed by the
Flow; an earlier success at another boundary cannot satisfy the current step.
Record the completion and its Flow consumption atomically when they share the
store, retaining existing version/claim comparisons. Provider process receipts
remain exact operational evidence, without becoming another product object.

Handoff must distinguish the Flow driver from the conversation driver. A Flow
can keep its orchestration claim while its selected AgentSession changes driver.
The old conversation driver may no longer start turns or change Session state;
the Flow consumes only the selected, recorded completion under its own fence.
An observer may display events but does not acquire either claim. If the driver
dies and the engine finishes a turn, recovery must retain that provider outcome
once, rather than replace it with the driver's exit result or silently lose it.

## Attribution, Started and historical import

Task implies Wave; supplied ancestors must agree. Flow-owned AgentSessions share
the FlowSession's nullable Task. Bind is null-to-Task, permits done/landed Tasks,
retains a present Wave and is idempotent for the same Task. CLI states the target;
Desktop confirms it. Bind cannot change Flow membership to satisfy a mismatch.

Deleting Run changes the implementation of Started, not its purpose. Reserve the
first actual Task work (agent conversation or mechanical Flow boundary) and set
`tasks.started_at` once in that transaction. Merely recording `lf task status`
or another observational Exec never starts a Task. Preserve existing timestamps
and recorded historical Started evidence. Import reports inferred timestamps as
inferred; it does not fabricate successful work.

Historical attribution is not bulk rewritten just because the Session's current
driver changes. Preserve the existing historical-attempt boundary when converting
Runs. **Supervisor implementation assumption, not a new Jack decision:** a bind
affects subsequent work; earlier usage retains its recorded attribution. Jack
was asked about whole-conversation versus prospective binding and no answer has
been recorded. Proceed with this conservative boundary, recording the assignment
time so mid-turn cumulative usage cannot silently move the whole earlier turn.
Attribute a turn/receipt according to its recorded start/assignment evidence;
unknown allocation remains explicit rather than inventing a token split.
Prove pre-bind, post-bind and active-turn cases in the import/bind fixture. The
earlier current-Run-only assumption does not authorize rewriting every old event.
A taskless FlowSession and its member Sessions cannot be bound piecemeal to
incompatible Tasks. Whole-Flow binding remains outside this selected operation.

Import uses real-shaped four-origin evidence: interactive, completed keyed Ask,
Task review and taskless pending review, plus headless Runs and command journal
history. Preserve names, feedback, native identity, captures, unknown membership,
failed attempts and controller-only evidence. Never invent an Exec for a Run
without evidence that a distinct lf process existed. One old Exec with several
Runs imports as one Exec with several correlated work events/Sessions.

Reserve publication and immutable artifacts remain distinct boundaries. A crash
between them must reconcile exact saved input without launching twice. Provider
launch refuses before side effects if required rows cannot be written. Ordinary
reads never import, scan manifests for identity, or restore mutable sidecars.

## Validation from current source and provider evidence

Inspected at `71496d5da` on 2026-09-28; these observations constrain the plan:

| Observation | Consequence |
| --- | --- |
| `session.rs::Run` owns attribution, location, publication and end; Session points at current Run | Removing the struct alone would lose several distinct facts; the destination map above is required |
| `lf/commands/flow.rs::run_op` executes in the driver process; `store/sqlite/flows.rs::settle_attempt_in` distinguishes missing operation receipts | Exec cannot substitute for each boundary; keep start/outcome correlation and interrupted-operation protection |
| `journal/mod.rs::ensure_run_context` already records parent Exec within a trace | Evolve this path into the indexed owner; preserve direct-child and historical-parent behavior |
| `harness/mod.rs::configure_agent_env` removes inherited execution identity | Introduce deliberate Session caller propagation; removing all scrubbing would reintroduce stale authority |
| `harness/codex.rs::start_inner` creates a dedicated stdio app-server with kill-on-drop | Live connect requires a reconnectable transport/owner; changing resume argv alone cannot implement it |
| `lf/commands/runs.rs::collect_runs_at` reads row-selected artifacts before final output limits | Query summary/filter/pagination in SQL first; list must not open every transcript |
| `store/sqlite/runs.rs::bind_session_runs_in` currently updates all Session Runs | Existing code exceeds the earlier current-Run-only assumption; retain this as a counterexample to resolve, not accepted history policy |

Actual installed Codex 0.157.1 was tested with an isolated provider fixture:
a second app-server and native `codex resume --no-daemon` refuse an actively
owned conversation; the TUI displays a lock screen. Interrupting a turn does
not release ownership; stopping the owning server permits resume. This does not
prove multi-client attachment, default daemon mode, real model/tool continuity,
or Claude behavior. [Research](research-execution-records-and-resume.md) retains
code observations; the local probe is `/tmp/lf-codex-resume.Trpyup/`.

The implementation must prove a reconnectable route before building the rest of
connect around it. Prefer the existing provider's supported endpoint. Any
Loopflow-owned bridge must be limited to this conversation/engine lifetime; it
must not restore Wave listeners, residents or a general daemon. A transport proof
must include driver handoff and nested lf calls, not only transcript viewing.

**Transport feasibility verified on 2026-09-28:** the actual Codex 0.157.1
app-server accepts two WebSocket clients over a private Unix socket. Client B
resumed Client A's active thread; the same engine and active turn survived A's
disconnect. Native `codex resume --remote unix://PATH THREAD` displayed that
active conversation without a lock screen. Closing that TUI left the turn
active. B then interrupted only that thread while another thread on the same
engine stayed active. Upstream responses were synthetic and credential-free.
Raw JSON, PTY text and reproduction are in
`/tmp/lf-connect-plan.nrPdX2/native-ui-probe/` and its sibling script.
The first raw-JSON Unix-socket probe and a proxy probe timed out. Unix transport
uses a WebSocket handshake; the raw-JSON assumption was wrong. The proxy timeout
was not diagnosed. None of these probes ran Loopflow's connect implementation,
transferred its database claim, changed LF ancestry or proved model/tool recovery.

Use this native endpoint as the first implementation route. It establishes that
a new general Loopflow service is unnecessary for client connectivity. Lifetime,
capture ownership, per-thread command provenance and exact driver handoff remain
implementation obligations; successful native attachment does not prove them.

## Implementation order and observable exits

1. **Finish Cut I review.** Independently inspect the actual diff and retained
   proofs, resolve the interrupted contention result, and move the ignored
   research snapshot out of live architecture discovery without weakening the
   checker or deleting evidence. Keep the launch-contract counterexample for
   the new writer cut. Local review acceptance is not installed acceptance.
2. **Prove connect and Exec ownership first.** Minimal actual-provider-engine
   fixture: start a headless conversation, connect another client without a
   second engine or interrupted turn, transfer the driver, issue a nested lf
   command and verify parent plus `via_agent`. Explicit restart preserves native
   thread and fences late output. Include shared-engine sibling survival.
   Failed feasibility changes the transport design before schema-wide work.
3. **Convert one complete lifecycle.** First align the core architecture,
   CLI/Session and planning documentation and STYLE guide with this accepted
   contract, marking unimplemented behavior. Then convert Exec/AgentSession/FlowSession writers,
   indexed readers, events and import together for headless → discover → connect
   → restart, with Task and taskless Flow paths. Carry H4 admission/publication,
   H5 opaque identity/history and H6 typed ancestry into this change. Delete
   replaced Run ownership and readers; preserve capture/receipt evidence.
   Intermediate commits may retain work in progress but are not a cutover.
4. **Complete consumers and preservation.** Ask/keyed retry, human review,
   rebase-conflict/landing-repair helpers, replay, nested loops, usage, activity,
   Session actions, Rust/Swift DTOs and retained desktop panes. Core docs and
   skills change with their callers. No stale subjects, title sidecars,
   WorkCatalog reconstruction or alternate taskless driver remains.
5. **H7.** Implement the accepted status-based Linear Project chapter rotation
   and `flow:` default. Preserve active Task identity/worktree/PR/FlowSession,
   cancel only proven untouched backlog, retry interrupted mutations, and show
   the same result after a second private Home sync. No chapter table or packet.
6. **Integrated proof, deletion and concept review.** Run affected suites once
   on final bytes, materialize drafts in a disposable copy, verify populated
   import and compare production additions/deletions including moves. Inspect
   the complete user interaction and ownership graph. Present code-complete
   review here; record deployment gaps separately, without a passing claim.

The supervisor owns the plan, proof interpretation and next instruction. Jack
authorized maximum useful parallelism without reducing quality on 2026-09-28.
Bounded contributors now have disjoint file ownership recorded in
[the parallel work allocation](parallel-work.md); the existing managed worker
owns execution-model integration and shared files. Contributors leave changes
uncommitted and do not repair each other's in-progress files. Build/test access
is coordinated, and final integrated proof stays serial. Use Codex only and
`lf` for git/delegation. No automatic restart or delivery beyond the requested
concept-review boundary.

## Completion evidence matrix

| Requirement | Evidence needed on integrated bytes |
| --- | --- |
| Every lf command visible | Root, direct child and agent child Execs, command result and searchable Task/repo/parent filters; no fake process rows |
| Stable resumable agent conversations | Fresh headless and interactive CLI launches plus connect/handoff/restart, same identity/title/feedback/native history, one current driver |
| Exact authority | Concurrent connect/restart, stale writer/late completion, PID reuse, engine outliving driver, shared-engine sibling preserved |
| Common FlowSession | Task/taskless, multiple attributed Flows with one managed pointer, source-independent resume, XOR, runtime loop children, retries, mechanical-operation interruption |
| Attribution and Started | Typed nullable ancestry matrix, bind race/same-target/done-Task, read-only command leaves Started unchanged, timestamp monotonicity and import provenance |
| History and usage | Four-origin plus command/headless import, old IDs and unknowns, no fabricated process, interrupted/idempotent import, separate failed/successful turn usage with missingness |
| Fast searchable reads | SQL filters and limits precede payload IO; indexed Task/Session/parent and text search plan; measure cold/warm list and detail on representative dense fixture, report scale and actual latency |
| Desktop and wire | Rust/Swift fixtures together; bound-but-off-roadmap stays bound; interactive filter; pane/surface/draft retention for bind/rename and Session identity after restart; projection changes only with inputs |
| Chapters/default Flow | `chapters.md` Done when, partial rotation/retry, conflicting current Projects explicit, two Home convergence, retained active Task and existing Linear Projects |
| Removal and checks | No replaced runtime owners; fmt, all-target Clippy, affected suites, migration and architecture checks; docs/skills/generated HTML match final behavior |
| Existing incident reports | Explicitly investigate/dispose of PR publication missing its Task row, stacking an already-created Task and cancellation/refused-start cleanup; preserve outcomes and scope |

Search means indexed identity/ancestry/command/skill/title discovery. Transcript
content is available on detail; no transcript search platform is introduced.
Measure actual timings rather than declaring a new latency budget from an index.

Configured provider/Desktop acceptance, backed-up real-Home conversion and release
activation remain full Task obligations. The current code-complete review must
name their status and prepared procedure; it cannot claim them from fixtures or
perform forbidden installed-Home changes to close a checklist.

## Open decisions and next action

### This slice

Exec admission, native-engine provenance and driver handoff are in progress.
[The working ledger](cutover/exec-ownership.md) records actual failures/passes,
retained executable edits and the next dependent proof. This is not completion
of the connect/ownership exit or acceptance of an owner-only cutover.

The accepted object names and Exec-tree requirements are settled. Transport
feasibility is an engineering proof, not a request for Jack to choose a protocol.
Prospective bind attribution is the explicit implementation assumption above;
Jack may revise it. Never silently use the current Session Task to rewrite all
old usage. This assumption does not block the remaining independent work.

Next action: continue the saved implementation step with the connect/Exec ownership proof,
using the exact acceptance rows above. Update this plan when evidence changes
an implementation choice; do not append another competing model.
