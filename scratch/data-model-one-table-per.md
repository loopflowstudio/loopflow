# One SQLite owner per product object

LOO-298 · Jack Heart · Accepted contract consolidated 2026-09-28.

The finish line is **code-complete concept review**: the accepted model works end
to end, affected checks pass on integrated bytes, replaced owners are deleted,
and the usage/ownership/deletion review is ready. Tables, renames or a passing
slice alone do not qualify. Configured provider/Desktop and installed migration
proofs stay explicit; no branch binary may touch the installed Home.

Read [current work](parallel-execution.md), [remaining work](remaining-work.md),
[import obligations](import-preservation.md) and [evidence](evidence.md) beside
this contract. [Chapters](chapters.md) owns the accepted Project model.
[Control and archive](parallel-work.md) records publication, ownership and history.
Older Run-based proposals and their intermediate exceptions are superseded here.

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
Taskless Flows use the same driver and record. One started Flow is one FlowSession.
Its captured template composition stays expanded; loop passes are positions in
that graph, identified by node and iteration tuple in AgentSession and Flow
history. Passes and subflows are display lenses, without another Session, claim
or lifecycle. Retry retains the same pass; Iterate advances the cursor and return
counters. Jack Heart selected this on 2026-09-30, superseding the runtime-child
contract. Implementation must fold existing child-pass history into the root by
forward migration before removing child rows and their ownership machinery.

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
| Captured input and launch publication | A captured event in AgentSession history, naming its Exec, plus the immutable payload; the step Exec owns its capture and only it may publish, which prevents two launches |
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
`tasks.started_at` once in that transaction. First bind of an existing agent
conversation also sets Started at assignment time, retaining any existing value;
preserving earlier usage does not postpone Started until another launch. This
retains Jack's first-assignment decision and the current architecture-reference
contract. Merely recording `lf task status`
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

## Completion evidence matrix

### Structured-result implementation boundary · 2026-09-29

Jack selected provider-constrained output, informed by
[PydanticAI](https://ai.pydantic.dev/output/) and
[Jev](https://typesafe.ai/blog/introducing-system-one-models-and-jev).
The captured boundary defines the type before launch: a repeat returns
`{decision: advance|iterate, summary: string}`; an XOR returns
`{path: <one captured path name>}`. Reject additional fields. No confidence score
is inferred. Each occurrence answers its own question; neither current catalog
content nor a later conversation turn can choose its result type.

The implementation uses the existing owners:

1. Derive schema and typed decoding from the saved ConcreteStep. Carry the schema
   through AgentConfig into native provider requests. Codex uses per-turn
   `outputSchema`, Claude print uses `--json-schema`, and OpenCode uses
   `format.type=json_schema`. See their
   [Codex](https://learn.chatgpt.com/docs/app-server#turns),
   [Claude](https://code.claude.com/docs/en/cli-reference) and
   [OpenCode](https://opencode.ai/docs/sdk/#structured-outputs) contracts.
2. Retain the provider's final structured value with its exact native turn in
   Session history, including recovery from native history after driver loss.
   Streaming text fragments and a command's zero exit cannot supply that value.
   Provider failure, absent output and invalid output remain distinguishable.
3. Read/validate the selected successful completion under the existing
   FlowSession version/claim. Consume it with cursor advancement in the same
   transaction. Delete the decision/router CLI and its store authority path;
   keep blocking/Ask feedback separate from navigation.
4. On invalid output, return the validation error to the same AgentSession with
   the same schema. Implementation choice: at most two corrective turns, retained
   durably so recovery cannot reset the bound. Never reinterpret an invalid result
   as provider failure or authorize a different live turn. Exhaustion leaves the
   cursor unchanged with a named validation failure.
5. Saved graphs/policies remain unchanged. The launch-owned output contract
   supersedes obsolete decision-command instructions in saved prompt content;
   no graph recompilation or legacy writer remains. Historical candidates without
   typed output retain their evidence but cannot settle a new boundary.

Proof: invalid→valid in one conversation, bounded exhaustion, late/stale/failed
results rejected, exact-once Task/taskless continuation, and interrupted native
output recovery. Retain shared-engine sibling and failed-thread/successful-idle
cases. The real Codex/local Responses proof now passes the formerly failing legitimate
retry, exhaustion and delayed-command cases. Current proof and limits are kept in
[the execution handoff](parallel-execution.md); configured acceptance remains open.

### Captured-event implementation boundary · 2026-09-29

The reader inventory at `.lf/tmp/captured-event/readers.md` is the starting audit.
Reservation writes a `captured` Session event and selects its sequence in the
same transaction. AgentSession and FlowSession current-capture references use
that sequence; publication, replacement, readiness and completion compare it.
Native start and artifact observations reference the captured event. Capture has
no outcome or resumable lifecycle; native completion remains separate history.

Artifact directory keys are plain strings in captured evidence, not a new ID
type. Keep old directory bytes, prepared→launching rename and byte-identical
publication recovery. New history selectors resolve through SQL; replay must use
that resolution before opening artifacts. Preserve old `run_` selectors and
unknown caller keys without assigning a current conversation to them.

`import_evidence` is the named destination for original historical SQL payloads,
including rows with no established conversation. It is an immutable, migration-only
archive (`source`, original selector, exact payload), never written by reservation
or continuation and never used as a live input catalog. It preserves unknown
membership, foreign-key ancestry constraints and Started evidence. Known
conversations receive captured events; mechanical evidence remains in Flow
history. Do not manufacture an AgentSession merely to attach unknown SQL.

Delete RunId and the live input catalog in this cut. Move Rust/Swift/fixtures
for SessionRecord, Task execution/blockers and Steer authors together. Keep old
serialized import evidence readable; command journal TraceId/run_events stays
separate. Prove interrupted reservation/publication, replacement, old selectors,
unknown SQL, exact original payloads and populated source/materialized upgrades.

### History reader implementation boundary · 2026-09-29

Rebase the released reader hunks onto captured-event identity. History is a read
projection beneath AgentSession, with an optional captured sequence and exact
native start/completion references; no input object or lifecycle returns. Retain
artifact/caller strings only as historical selectors. Native receipts without a
capture or start remain discoverable under their Session with unknown attribution,
including usage coverage. Do not assign the Session's present Task to them.

Use one typed reducer for CLI history, usage, Work activity, landing conclusions
and scorecards. Apply scope and recent limits in SQL before decoding; exact Work
and caller drills remain complete. Preserve every eligible unfinished item and
native completions inside the window. Keep native and recorder outcomes distinct.
Flatten the proposal's completion-only wrapper; retain types only where consumers
need a distinct wire shape. Move Rust, Swift and fixtures together. Proof covers
old selectors, post-bind prospective attribution, orphan native receipts, no
double-counted usage, complete drills and bounded recent reads. Desktop inventory
paging/reconciliation remains the following coordinated cut.

### Whole-design proof

| Requirement | Evidence needed on integrated bytes |
| --- | --- |
| Every lf command visible | Root, direct child and agent child Execs, command result and searchable Task/repo/parent filters; no fake process rows |
| Stable resumable agent conversations | Fresh headless and interactive CLI launches plus connect/handoff/restart, same identity/title/feedback/native history, one current driver |
| Exact authority | Concurrent connect/restart, stale writer/late completion, PID reuse, engine outliving driver, shared-engine sibling preserved |
| Common FlowSession | Task/taskless, multiple attributed Flows with one managed pointer, source-independent resume, XOR, loop positions and return counters, same-pass retries, mechanical-operation interruption; child-pass migration preserves history |
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
