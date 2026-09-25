# Flow and Session compression review

2026-09-25, starting HEAD `31e68051d`. No runtime reduction selected. Only this
note and stale human-boundary wording in docs/architecture-reference.md changed.

Finish line: remove a competing representation or ownership boundary while
preserving captured execution, exact completion and feedback delivery. Fewer
lines alone, renamed wrappers, or deleting recovery evidence do not qualify.

## Effective model, before and after

An expanded Flow captures every Skill and XOR alternative. ExecutionCursor owns
the selected path, position, pending route/verdict, direction and traversal counts;
its finish method delegates edge calculation to finish_step. Task transactions
and ordinary Flow files settle that same model under their own authority.
An active ordinary boundary records an attempt and its receipts. SessionRecord
projects a human boundary or native conversation. Complete returns feedback;
the deciding occurrence chooses Advance/Iterate. Keyed Ask retains its answer
for the blocked caller and recovery.

The runtime model and public contract remain unchanged. The architecture owner
table no longer advertises deleted decide_flow or a human navigation decision.
Related paragraphs now describe explicit review completion and separate edges.

## Model path and mirrors inspected

- engine/flow.rs, execution.rs and transitions.rs: captured occurrence policy,
  selected child traversal, pending decisions and the pure edge reducer.
- durable.rs, store/sqlite/durable.rs and controller/task/mod.rs: Task cursor,
  stored projections, old progress decoding, worker claims and review settlement.
- ops/flow_run.rs, flow_session.rs and human_session.rs: ordinary position,
  attempt receipts, Session identity, captured launch input and Ask retention.
- lf/commands/flow.rs, session.rs, run.rs and lf/mod.rs: launch, resume,
  decision, readiness and completion surfaces.
- Swift SessionRecord, RegistryQuery and SessionsView; Session DTO fixtures,
  controller/engine/Session regression sources, pursue.yaml, architecture/data.md,
  architecture-reference.md and the curated task-continuation direction.

## Suspected reductions and disposition

| Candidate | Why it stays in this pass |
| --- | --- |
| Ordinary Boundary policy and selected XOR body copies | Already removed. Both derive from the captured definition; no remaining second policy/body to delete. |
| FlowTransition versus cursor navigation | A pure edge result consumed by one traversal owner, not another interpreter. Replacing its variants with index comparisons would encode the same distinctions implicitly without removing ownership. |
| Task and ordinary persistence adapters | Task claim/version checks and atomic domain events differ from ordinary file/driver locks. Joining them would introduce a new persistence abstraction, not remove a duplicate object. |
| SQL root index/iteration versus serialized cursor | Real storage redundancy, but the columns also recover older flat progress. A coherent deletion needs a forward migration that first materializes those facts. Deleting checks or rewriting the applied draft would lose the recovery contract. Leave the whole slice for that migration. |
| Task human token's captured Skill | Transport snapshot consumed by synchronous prompt preparation; not an independently mutable definition. Removing it requires moving preparation to authoritative store lookup and proving launch/restart behavior. Removing only the bytes would restore source loading or strand the current launcher. |
| Boundary completion, Flow finished receipt and cursor position | Distinguish successful boundary execution, settled navigation and terminal invocation recording. A recorded candidate alone is not a successful Run; collapsing receipts into pending decisions would change recovery. |
| Session states/kinds and completion API | Rust/Swift fields match. State controls readiness/liveness and Complete availability; kind selects completion semantics. Swift already has one Complete action and no navigation action. There is no second client decision owner to remove. |
| Legacy verdict decoding and retained Ask completion | Preserve populated execution facts and retry answers. They are required recovery paths, not unsupported internal aliases. |
| Wave Playhead and shared types under its module | The remaining structural deletion is real but separately scoped in wave-playhead-removal.md. It requires journal, scheduling, resident and client migration; a rename or import move alone would not remove the interpreter. |

Review conclusion: no runtime API, field, type or DTO removed. Prior compression
already removed the coherent local duplicates. This pass corrects the remaining
architecture wording without widening into a migration or replacing explicit
types with implicit conventions.

## Verification

Source and mirror review only; no executable behavior changed, so no behavioral
suite was rerun. Historical scratch passes are not new validation.
`uv run python scripts/check_architecture.py` passed all eight ownership
categories; `git diff --check` passed. The new note was also checked for trailing
whitespace and balanced fences.
No provider, production Session, installation, orchestration, commit or publication.
