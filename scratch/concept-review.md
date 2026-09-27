# LOO-298 concept review

2026-09-26 · Reviewed `246f0006002f43bdef906db052509c04a0b8120c`
against `4cd64be3d0432aa04dd9256293eef7088db97ccc`, concentrating on the
selected-attempt repair at `830e9af7b` and the latest
[slice review](data-model-slice-review.md). Entry tree was clean. This pass
changes only this concept review; the implementation and slice review are preserved.

**Judgment: keep the amended concepts; the branch is not ready to publish.**
Session is the conversation Jack keeps; Run is an attempt within it. The previous
lookup-to-action P1 is repaired at source level. The Task-start reader's reference
to the deleted table is also repaired. Neither repair has executed behavioral
proof. No additional product noun, recovery command or lifecycle is needed.
The complete owner/caller conversion and all eight Done When obligations remain.

The [amended design](data-model-one-table-per.md), [review feedback](data-model-review-feedback.md)
and [Session decision](session-runs-and-current-run.md) govern. Jack commissioned
one SQLite owner per main product object. The later participant (name unresolved)
approved taskless Invocations, Session owning Runs plus a current Run, and Flow /
Invocation naming. The older Task title and Wave memory do not override those
corrections. This review supplies evidence and chooses no Flow navigation.

## Usage first

Proposed end-state walkthrough, not an executed demonstration:

```sh
lf --interactive : "Review the parser"
lf session list --json
lf session rename <session-id> "Parser review"
lf session bind <session-id> --task INF-123 --json
lf session list --task INF-123 --json
lf runs --task INF-123 --json
lf session open <session-id>
```

From an unregistered checkout, Jack starts an unbound conversation, names it,
and binds it to INF-123 even after its PR lands. Every view agrees about the
current Run's Task. The same selected Session and live terminal retain the
draft. INF-123 is illustrative; bind and the filtered Session list remain target
behavior. A registered Task checkout supplies ancestry at launch instead.

Recovery keeps the same Open action. Resume native history when recoverable;
when exact execution evidence permits replacement, append a Run and select it
without losing the Session's title, feedback or earlier attempts. Missing launch
receipts cannot justify another provider launch. Retaining Session identity does
not recover a dead provider's unsaved native state.

For a review, Ready saves feedback; Complete returns it once to the waiting
boundary. Pane closure and provider exit never complete the review. A broken
neighboring invocation should leave this conversation reachable.

Draft clarification for `docs/lf.md` Sessions, after all-kind implementation and
proof; the latest Task-review source supports this interaction:

> Use the Session ID to name the conversation. A Run ID or Run prefix selects
> that attempt: if it has been replaced, the command reports its Session and
> current Run without acting on the replacement. Open and Complete select a
> current attempt and keep that selection through the operation. A concurrent
> change rejects the operation instead of redirecting it. Complete saves that
> attempt's feedback and tears down the same attempt. Renaming by Session ID
> names the conversation even if its current attempt changes.

This clarifies the existing stale-attempt guarantee; it is not a new approval.
It avoids making Jack choose among different recovery commands or understand
locks. Keep the already-clear examples in `docs/lf.md:576` unchanged here.
Before shipping, reconcile the preceding prepared-Run paragraph (`:562`) with
the final reservation/publication contract: it still describes the integrated
file-backed path and launch-time context filling. The architecture reference
owns transactions and validators; `docs/waves.md` owns Chapter history.
The builtin `LOOPFLOW.md:75` already distinguishes readiness from completion,
and `task/skill/review-design.md:112` requires explicit assumptions. No skill
rewrite is needed to teach storage internals.

## Core model in one screen

| Action | Identity and owner | State change |
| --- | --- | --- |
| Choose a workflow | Flow template; Project default | Task may explicitly select another Flow |
| Execute or restart it | Invocation; captured graph, cursor, returns, claim | Resume retains capture; restart retains history under a new execution identity |
| Enter a nested loop body | Child Invocation | Runtime nesting only; template composition is expanded before execution |
| Inspect an attempt | Run; nullable Invocation, Task, Wave and Session | Run owns outcome, usage and exact process evidence |
| Name or reopen a conversation | Session; title, feedback, completion, current Run | Replacement appends a member Run and compares the current pointer |
| Complete a review | Selected Run and exact waiting boundary | Settle saved feedback once, then tear down the same attempt |
| Assign the conversation to work | Current Run's Task/Wave | Validated bind; membership and earlier Run attribution remain intact |
| Inspect past plans | Repository Chapter; Project at (Wave, Chapter) | Rotation preserves active identity and freezes predecessor evidence |

A node and launch-time iteration tuple locate a Run. No occurrence object is
needed. Loop return, Run retry, Session reopen and Invocation restart preserve
different facts. An operation's selected-attempt expectation is an input, not
another durable record. Session identity grants neither process control nor
permission to settle an invocation.

## Findings and consequences

Source paths below are relative to `rust/loopflow/src/`. These are source
observations, not runtime reproductions.

### 1. The selected attempt now survives the action boundary

The former experience could redirect a request for Run A to replacement B,
while teardown still addressed A. The current path keeps the original choice:

- `ops/human_session.rs:724` holds launch exclusion through Complete's
  settlement, continuation request and teardown. `controller/task/mod.rs:764`
  accepts the original `FlowPosition`; it no longer reloads B as the expected
  position. `store/sqlite/children.rs:235,271` compares that original snapshot
  in the transaction, and `sessions.rs:362` compares current Run and feedback.
  Continuation remains requested before stopping the selected review provider.
- Open (`human_session.rs:1035`) re-resolves the original exact/prefix selector
  under the launch lock and rejects a changed position. Native-history lookup
  precedes client transfer (`:1632`); transfer and client receipt publication
  retain exclusion. `lf/commands/util.rs:616` releases the lock before waiting
  for provider exit. Ready and Complete can then proceed during the conversation.
  Replacement reserves against the opening snapshot and returns its own Run
  (`human_session.rs:894`), rather than reading a later current pointer on exit.
- Rename (`human_session.rs:1505`) retains the supplied selector across the lock
  wait. Exact/prefix requests also pass the selected Run into the SQL transaction
  (`store/sqlite/sessions.rs:214`). A stable Session-ID rename deliberately
  remains a conversation operation.

The public CLI dispatch preserves the supplied ID (`lf/commands/session.rs`).
Its separate completion-worktree lookup supplies journal scope and does not
rewrite the selector. JSON Open prepares an action; it is not native resume.

The deterministic action matrix (`controller/task/mod.rs:2338`) pauses after
lookup, replaces A with B retaining feedback, and resumes Complete/Open/rename
for exact, prefix and Session selectors. It asserts unchanged B/cursor/feedback
and simulated clients on rejection; Session-ID rename is the intended exception.
Subsequent Open and Complete of B preserve A's simulated client, stop B and save
one completion event. The native handoff fixture (`lf/commands/util.rs:904`)
separately observes actual receipt publication through the startup lock and stops
its owned shell stand-in. These complementary fixtures have compiled, not run.

**Consequence:** retain this API distinction and its fences. There is no need
for a second current-attempt store or a generic action abstraction. The original
P1 is no longer an outstanding source defect; acceptance still needs execution.
Ask and taskless native-resume callers share the lock handoff, but their separate
owners and action contracts are not thereby converted.

### 2. Task-start repair preserves history without claiming provider success

The latest slice found `store/sqlite/chapters.rs:97` still reading the dropped
`task_flow_positions` table through Task detail (`lf/commands/waves.rs:1090`).
It now reads retained `flow_invocations` and published Runs, alongside historical
events. Current Rust outside migration history has no remaining reference to
that table. This supersedes earlier negative-inventory claims that missed it.

The fixture at `store/sqlite/durable.rs:1613` uses an ephemeral migrated store:
reservation leaves Started false; publication makes it true; completing the
review and removing its current invocation retains Started. It is unexecuted.
Publication here is durable launch progress, not proof of a provider start,
liveness, successful outcome or completed work. Keep those distinctions when
general Run lifecycle fields replace the remaining event-based read.

**Consequence:** no new Task lifecycle is needed. Displayed Started and safe
backlog retirement ask different questions. The target can derive the display
from Runs while preserving historical execution/authored/PR evidence for
retirement. Binding away the last current attribution must not make old work
eligible for automatic abandonment.

### 3. One conversation owner remains the unfinished product improvement

| Experience still split | Current source | Required continuation |
| --- | --- | --- |
| List conversations | `human_session.rs:575` combines four sources | Query Session/current Run rows for every kind |
| Keep Ask feedback/name on replacement | `human_session.rs:1204,1208` resets feedback and copies names | Convert Ask to stable Session ownership; remove the copy |
| Execute a taskless Flow | `ops/flow_run.rs:108,136` reads/writes `position.json` | Shared invocation owner and driver; preserve captured recovery |
| Find a Task's Runs | `lf/commands/runs.rs:77,91` scans manifests and matches Work selectors | Indexed typed Run parents, also used by usage and Sessions |
| Bind and inspect history in the desktop | `session.rs` contains partial Run facts; Swift `SessionRecord.swift:134` retains the old projection | Complete general Run facts/validators and common DTO/history/pane identity |
| Rotate the repository plan | `store/sqlite/chapters.rs:44` clears current by Wave | One repository Chapter activation and frozen predecessor plans |

The earlier publication/lookup improvements remain: retained SQL membership
precedes interactive dispatch for exact/prefix Run selectors; reserved artifacts
reconcile before SQL publication and recorder construction (`run_record.rs:1581,2205`);
unknown published launches cannot be replaced merely from missing receipts;
invalid selected captures render on their own unavailable Session
(`human_session.rs:1743`). Those authored proofs remain owed too.

The simplifying result is one conversation list, one Open action, and stable
pane identity across replacement. Complete the common owners and all callers,
then delete the alternate paths. They still support live behavior, so deleting
them now would lose capability. Table-only readers over an unimported Home are
also forbidden. No intermediate dual-owner publication is selected.

## Unresolved product consequences

Current-Run-only bind remains an implementation assumption. If A belonged to X,
replacement B initially belongs to X; binding the Session to Y changes B while
A's attribution and usage remain on X. Approval of stable conversation identity
did not approve a bulk historical rewrite. Any requirement to move all history
needs an explicit ownership decision before expanding that behavior.

A taskless Run can belong to an Invocation. Nullable Task equality then rejects
binding that individual Run to a Task atomically. Orphan does not imply
independent execution. Allowing that bind requires changing the accepted parent
constraint or deciding how whole-invocation attribution moves. Neither is
silently selected here. No new participant decision is required to continue
within the recorded assumptions, and this bounded review opens no Ask.

## Smallest next action and proof

When TESTING.md preflight permits, execute the selected-attempt matrix, native
handoff and Task-start reader fixtures first. Also retain the Ask reopening and
taskless Flow regression commands from the latest slice review. A passing matrix
would prove these simulated interleavings, not configured provider recovery or
actual CLI dispatch. Add the isolated CLI status/roadmap read against a registered
Task to exercise the repaired reader's public consumer.

Keep every owed publication, historical-selector, stale-provider/Ready,
corrupt-neighbor, repeated-replacement, schema, Task-controller and durable-store
proof in the design ledger. Repeat populated preservation after canonical
materialization in a disposable source copy. Compilation proves none of their
runtime results.

Continue the approved shared taskless driver and runtime nesting, general Run
facts, all Session kinds, launch/read/bind callers, Rust/Swift DTOs and stable
pane identity/caches, repository Chapter operation, populated offline import,
exact-writer real-Home maintenance and configured CLI/app acceptance. Comparable
measurements, deletion research followed by deletion, and final documentation
consistency remain required. All eight Done When obligations still govern.
The supplied PR #1296 publication-record and existing-Task stacking reports remain
unreproduced implementation scope; this review changes no delivery state.

## Evidence boundary

Inspected the branch inventory and latest implementation diff, normal Complete
and rename, Open recovery, reservation/publication, native lock handoff, exact
settlement, deterministic fixtures, Task-start consumers, current alternate
owners, DTOs and affected usage/skill guidance. No new blocking source defect
was identified in the selected-attempt repair. No repository-wide history
survey, implementation change, execution restart or product test was needed for
this concept judgment.

Reused the latest slice review's resource receipt: active `main-view-task`
**15.3 GiB / 12 GiB**, **97.2 GiB** free. This documentation-only pass did not
resample resources or run safe recovery. That review records passing formatting,
all-target Clippy, migration validation and generated HTML consistency; tests
remain unexecuted under TESTING.md's resource rule. Prior receipts keep their
scope. SQLite owner coverage still has the recorded **32/33** `wave_chapters`
gap; historical branch-range whitespace findings remain.

Only this note changed. Its whitespace and local-link checks pass; those checks
clear neither the architectural gap nor historical whitespace. No Home, provider,
PR, Task disposition or Flow navigation changed. No executed proof was invalidated
by this note, and no full-design acceptance claim is promoted by it.
