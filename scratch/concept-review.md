# LOO-298 concept review

2026-09-26 · Reviewed `54abb81ff5deb165bc4e39473766efa53220d65b`

**Judgment: keep the amended concepts; the branch is not ready to publish.**
Session identity surviving execution replacement makes the product easier to
use. Flow / Invocation naming and taskless execution need no further redesign.
The implemented column reduction and Task invocation retention do not yet
deliver that experience. Binding has two product consequences that need explicit
visibility before acceptance. No further concept change is selected.

This autonomous review follows the supplied concept-review skill. Governing
intent is the [amended design](data-model-one-table-per.md),
[review feedback](data-model-review-feedback.md), and
[Session ownership decision](session-runs-and-current-run.md). Jack's original
direction commissions one SQLite owner per product object. The later participant
(name unresolved) approved taskless Invocations and Session owning Runs plus a
current Run. The older Task title and copied handoff do not override that review.

## Usage first

Proposed end-state walkthrough, not an executed demonstration. The affected
[Session documentation](../docs/lf.md#sessions) already teaches the intended
commands; keep its examples and present-tense specification. The following
clarifications are review copy for implementation and acceptance, not a claim
that the current CLI supports bind, rename, or the Task filter.

```sh
lf --interactive : "Review the parser"
lf session list --json
lf session rename <session-id> "Parser review"
lf session bind <session-id> --task INF-123 --json
lf session list --task INF-123 --json
lf runs --task INF-123 --json
lf session open <session-id>
```

Jack starts this example outside a registered Task checkout, so the conversation
starts without a Task. Rename changes its title. Bind assigns its current Run to
INF-123, even if that Task's PR has landed. The same Session stays selected and
its live terminal keeps its draft. INF-123 is an illustrative identifier.

Recovery uses the same Session and `open` action. Resume the current Run when
its native history is recoverable. If a replacement is required and exact
execution evidence permits it, append a Run to this Session and retain the old
Run in history. Keep the name and saved feedback. Stable Session identity does
not promise recovery of a dead provider's unsaved native state.

For a Flow review, Ready saves feedback; Complete returns it once to the waiting
boundary. Reopening, closing a pane, or provider exit never completes the review.
If another invocation is broken, this valid Session remains reachable; the
broken invocation retains an identified recovery problem.

Two additions to the eventual usage copy expose the current assumptions:

- Bind changes the current Run and the attribution inherited by its replacement.
  Earlier Runs keep their own Task and usage attribution. Session history can
  therefore span Tasks.
- Under the retained parent equality, a Run in a taskless Invocation cannot be
  assigned a Task individually. A rejected bind changes nothing. Taskless and
  independent are different: a taskless Run can still execute a Flow node.

The [architecture reference](../docs/architecture-reference.md#core-models-and-apis)
owns validators and transactions. `docs/waves.md` already owns Chapter history;
no new planning vocabulary is needed. The builtin
`rust/loopflow/src/engine/builtins/surfaces/headless.md:7` correctly distinguishes
Ready from completion, and `task/skill/review-design.md:114` requires recording
open assumptions. Keep those skill instructions unchanged. Storage internals
and replacement bookkeeping do not belong in each review skill. No canonical
usage or skill rewrite is needed during this bounded review.

## Core model in one screen

| Jack's action | Concept and identity | State owner / API responsibility |
| --- | --- | --- |
| Choose a workflow | Flow template | Authored definition; Project selects the default, explicit selection can override |
| Start, interrupt, resume or restart its execution | Invocation ID | Captured graph, cursor, returns, claim and nullable Task; restart replaces execution identity |
| Revisit one loop body on a pass | Child Invocation ID | Runtime nesting only; template composition introduces no invocation parent |
| Inspect an execution attempt | Run ID | Nullable invocation, Task, Wave and Session; outcome, usage and exact process evidence |
| Open, name and complete a conversation | Session ID | Title, feedback, completion and current Run; history queries Run's Session FK |
| Bind a conversation to work | Current Run's Task/Wave | Validated write; does not reopen Task or change execution membership |
| Inspect an earlier plan | Chapter ID and Project at (Wave, Chapter) | Repository-wide clock; preserved historical evidence |

A node and launch-time iteration tuple locate a Run within execution; they do
not need another product object. A loop return, a child invocation, a Run retry,
and a Session reopen remain distinct transitions. The design already explains
their different lifetimes; collapsing them would lose required recovery facts.

## Findings and consequences

### 1. Conversation identity belongs above execution replacement

Observed source: `ops/human_session.rs:289` resolves interactive identity via a
Run manifest, then dispatches to Ask and Task review owners. Task review lookup
at `:1166` enumerates current invocations. Ordinary Flow boundaries have another
path, backed by `ops/flow_run.rs:108` and `:122` reading/writing `position.json`.
`swift/Loopflow/Models/SessionRecord.swift:19` exposes the current projection;
it has no current-Run/history contract.

Normal path today: CLI Open → target dispatch → kind-specific surface → native
resume. Failure path: `open_boundary` (`human_session.rs:748`) attempts native
resume, then clears the Ask's old Run binding and feedback at `:780`, or the
Task position's binding and feedback at `:799`, before relaunch. The relaunch
writers at `:548` and `:594` also reset feedback. This is source evidence of
the existing model, not a regression introduced by either storage cut.

Accepted replacement: Open addresses one Session row, follows its current Run,
and switches that pointer together with the exact pending review binding when
replacement is warranted. Preserve prior Runs, name and feedback. This removes
the need for callers to understand the inventory's storage kinds or recover a
conversation by finding a new Run ID. Ask and review kinds still determine where
completion returns its answer; those are useful semantics, not duplicate owners.

The change is behavioral: existing reopen proofs cannot establish retained
feedback merely by continuing to pass. Prove two replacements, stale old-Run
Ready/Complete rejection, and one successful completion consuming the retained
answer. Keep published native identity and exact-client authority separate from
Session identity. Do not infer a right to relaunch from a missing receipt.

### 2. Binding the Session does not currently mean moving its whole history

This is a design consequence, not an observed runtime defect. The
[current-Run-only assumption](data-model-one-table-per.md#validation-and-denormalization-audit)
is internally consistent, but a single-Run demo hides its visible consequence:

| Proposed interaction | Session's displayed Task | Earlier Run A | Current Run B |
| --- | --- | --- | --- |
| Run A starts on Task X | X | X | Not created |
| Recovery appends B | X | X | X |
| Bind Session to Task Y | Y | X | Y |

After the last row, `session list --task Y` finds this Session while
`runs --task X` still finds A. Usage for A stays on X; usage for B follows Y.
The readers agree about each Run, but Session history is not identical to the
Task's Run history. The broad phrase “usage attribution follows the field” must
not imply a bulk rewrite.

Keep the simpler current-Run assumption for implementation. The exact product
choice for later review is whether assigning a conversation should also reassign
its previous attempts. If Session-wide history movement is required, return
that ownership change for design and behavioral review: it can conflict with
invocation-owned Runs and cannot be implemented by silently rewriting all rows.
Stable Session identity was approved; this historical-binding consequence was not.

### 3. Taskless Flow support exposes the limit of universal bind

Observed: `ops/flow_run.rs:33` already permits optional Task/Wave selectors.
Target rule: `Run.invocation.task == Run.task`, including null, in the design's
validation section. Therefore binding a taskless Flow review's current Run to
Task X violates the rule, even though the Session appears among orphans.
`docs/lf.md:580` describes changing/clearing an invocation Run's Task but does
not state this null-to-Task case explicitly.

The exact unresolved choice is whether universal bind means one common operation
with this invariant rejection, or whether assigning an individual taskless Flow
Run to a Task must succeed. The latter changes the agreed ancestry contract;
assigning the whole Invocation would also affect other Runs, children and current
root ownership. Neither is a harmless UI change or selected alternative.

Retain nullable equality and atomic rejection pending that choice. Do not add a
synthetic Task, silently change membership, or teach starting another conversation
as an equivalent successful bind. This choice does not block writing this review
or implementing the already-approved owners.

### 4. Session discovery must not depend on unrelated execution health

Carried from [review-slice](data-model-slice-review.md), confirmed by source:
`store/sqlite/durable.rs:392` decodes every Task position before selecting human
steps; `decode_flow_position:1247` parses capture, cursor, failure and claim.
Errors propagate through Session listing (`human_session.rs:269`) and exact Task
review lookup (`:1166`). A malformed autonomous capture can prevent reaching a
valid unrelated review. This remains unexecuted source-traced behavior.

The simpler interaction is to open the selected conversation without repairing
another Task first. Follow it through to direct Session/current-Run reads, then
validate the selected invocation when its execution is needed. Preserve broken
invocation bytes and expose the specific failure through its recovery surface.
Restoring a stored human flag or silently dropping corruption would not deliver
the accepted ownership model.

The four removed columns have no remaining current SQL consumer in the inspected
diff. Retain root index/iteration until historical conversion: the decoder at
`durable.rs:1273` still reconstructs old flat progress. The smaller SQL shape is
useful, but it cannot count as the Session availability or ownership result.

### 5. Invocation history is preserved; conversation history still needs its owner

New source evidence since the previous concept review: the
`retain_flow_invocations` draft replaces `task_flow_positions` with an
invocation-keyed table. Its partial unique index selects one current invocation
per Task. Task completion closes that row (`store/sqlite/children.rs:275`,
`durable.rs:1177`); restart marks it replaced (`children.rs:355`). Capture,
cursor, pending Run, feedback and final claim remain stored. The JSON identity
constraint and the unclaimed update's invocation-ID predicate
(`durable.rs:1030`) preserve the distinction between execution identity and
reused numeric versions. The populated preservation fixture at
`store/migrations.rs:4778` also checks rejection of duplicate invocation IDs
and dangling Task parents; it has not been executed.

This supports a simpler future interaction: finish or restart work and still
inspect the previous execution. It introduces no extra recovery action for Jack.
Closed claims are history, not permission to control a process; chapter evidence
includes past execution while active-claim reads select only the current row
(`store/sqlite/chapters.rs:98–101`). Keep those distinctions through the cutover.

Retention alone does not make completed conversations accessible. Session
discovery still selects only current Task invocations, and Run replacement still
overwrites their pending binding. There is no Session row or Run-history query
behind that retained feedback. Do not describe invocation history as completed
Session history, or build another Session projection over closed invocation rows.
Complete the approved Session/Run owner and use the retained execution as its
preservation input. The nullable SQL Task column likewise does not establish
shared taskless execution: `FlowRun` still has its separate file owner.

No new lifecycle or API wrapper earns its place here. Keep the public distinction
between resuming a conversation and restarting an invocation; deleting that
distinction would confuse native recovery, new execution and historical evidence.
The existing static receipts are unaffected by this review. Retained-feedback,
Session availability and history claims need their own behavior proofs; neither
compiled migration fixtures nor existing reopen tests establish them.

## Evidence, next action and handoff

The entry tree contained the preceding step's uncommitted
`scratch/data-model-slice-review.md` update. It was read and preserved unchanged;
this review edits only `scratch/concept-review.md`. HEAD matches that review's
inspected HEAD. Inspected the invocation-retention diff, migration, settlement,
chapter evidence, CLI/Swift Session shape and normal/recovery paths above.
No product tests, Home inventory, migration or live demo ran in this review.

Reuse the current review-slice's evidence limits: its resource preflight and safe
recovery failed on the active `main-view-task` build at **15.3 GiB / 12 GiB**,
with **98.8 GiB** free. This review did not resample that condition. Behavioral
tests are still owed; compilation is not their execution. That review records
formatting, migration validation and HTML consistency passing. Architecture
coverage is now **30/31**, with only `wave_chapters` missing; the retired Task
table gap is resolved. The branch-range whitespace failure remains (earlier
draft header and copied patch context). A clean check of this note clears
neither that failure nor the remaining architecture gap.

Smallest next proof, after TESTING.md resource preflight permits execution:

```sh
cargo test -p loopflow --lib dropping_task_step_projection_preserves_execution_and_review_evidence
cargo test -p loopflow --lib retaining_invocations_preserves_populated_execution_and_review_bytes
cargo test -p loopflow --lib store::sqlite::durable::durable_store_tests
cargo test -p loopflow --lib stale_human_decisions_cannot_target_a_replacement_invocation
```

Use TESTING.md's isolated environment; also prove populated preservation after
draft materialization in a disposable source copy. Then implement the coherent
Invocation/Run/Session owner cutover with all readers and writers. Its first
Session proof should combine retained feedback across repeated replacement with
an unrelated malformed invocation, and verify that stale results cannot settle
the current boundary. Add the two-Run bind example and taskless null-to-Task
rejection to make the assumptions observable. Prove taskless review recovery
after template removal through the same invocation owner and settlement fences.
Resource-blocked execution does not block ordinary implementation of the
approved owners; no new Ask is needed to proceed within that scope.

Repository-wide Chapter conversion remains required: current
`store/sqlite/chapters.rs:39` activates per Wave. DTO/desktop integration,
populated Home import and rehearsal, configured CLI/app acceptance, measurements,
and the commissioned deletion research remain required too. Keep the full
design's preservation obligations; this review selects no narrower finish line.

No new concept or speculative redesign is selected. No navigation edge, PR
mutation, Task completion, execution restart or next Run is requested by this
review. The following Flow step owns navigation using this evidence.
