# LOO-298 concept review

2026-09-26 · Reviewed `d55ad174d7214b2cf586880713a598b30bf1041f`

**Judgment: keep the amended concepts; the branch is not ready to publish.**
Session identity surviving execution replacement makes the product easier to
use. Flow / Invocation naming and taskless execution need no further redesign.
The implemented column reduction does not yet deliver that experience. Binding
has two product consequences that need explicit visibility before acceptance.

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
at `:1162` enumerates positions. Ordinary Flow boundaries have another path,
backed by `ops/flow_run.rs:107` and `:122` reading/writing `position.json`.
`swift/Loopflow/Models/SessionRecord.swift:19` exposes the current projection;
it has no current-Run/history contract.

Normal path today: CLI Open → target dispatch → kind-specific surface → native
resume. Failure path: `open_boundary` (`human_session.rs:748`) attempts native
resume, then clears the Ask's old Run binding and feedback at `:779`, or the
Task position's binding and feedback at `:798`, before relaunch. The relaunch
writers at `:547` and `:593` also reset feedback. This is source evidence of
the existing model, not a regression introduced by the four-column removal.

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
steps; `decode_flow_position:1244` parses capture, cursor, failure and claim.
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
`durable.rs:1270` still reconstructs old flat progress. The smaller SQL shape is
useful, but it cannot count as the Session availability or ownership result.

## Evidence, next action and handoff

The working tree was clean at entry. Relative to review-slice's inspected HEAD,
only two scratch notes changed; executable and canonical documentation bytes
are unchanged. This review inspected the branch diff and the normal/recovery
paths above. It ran no product tests, Home inventory, migration or live demo.

Reuse review-slice's evidence limits: the last resource preflight and safe
recovery failed on the active `main-view-task` build at 14.5 GiB / 12 GiB.
This review did not resample that condition. Behavioral tests are still owed;
compilation is not their execution. Its architecture inventory gaps
(`task_flow_positions`, `wave_chapters`) and branch-range whitespace failure
remain unresolved. A clean check of this new note does not clear them.

Smallest next proof, after TESTING.md resource preflight permits execution:

```sh
cargo test -p loopflow --lib dropping_task_step_projection_preserves_execution_and_review_evidence
cargo test -p loopflow --lib store::sqlite::durable::durable_store_tests
```

Use TESTING.md's isolated environment; also prove populated preservation after
draft materialization in a disposable source copy. Then implement the coherent
Invocation/Run/Session owner cutover with all readers and writers. Its first
Session proof should combine retained feedback across repeated replacement with
an unrelated malformed invocation, and verify that stale results cannot settle
the current boundary. Add the two-Run bind example and taskless null-to-Task
rejection to make the assumptions observable.

Repository-wide Chapter conversion remains required: current
`store/sqlite/chapters.rs:39` activates per Wave. DTO/desktop integration,
populated Home import and rehearsal, configured CLI/app acceptance, measurements,
and the commissioned deletion research remain required too. Keep the full
design's preservation obligations; this review selects no narrower finish line.

No new concept or speculative redesign is selected. No navigation edge, PR
mutation, Task completion, execution restart or next Run is requested by this
review. The following Flow step owns navigation using this evidence.
