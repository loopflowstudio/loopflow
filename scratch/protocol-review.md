# Loopflow decision protocol — review surface

This is LOO-295 / Advance, not the state-recovery work in LOO-296. The current
branch is ready for a focused concept/code review, not release acceptance.
The complete direction remains `loopflow.md`; this note does not reduce it.

## The core model to review

A Flow owns its authored steps and backward edges. A loopflow is the same Flow
with at least one edge. A pass traverses the repeated section; a slice is work
inside that pass. `loop-decide` judges the evidence after review-slice and
concept-review. Work and review agents do not write navigation files.

- `RepeatPolicy { from, max_iterations }`: the authored edge and pass budget.
  The builtin design review returns to kickoff; delivery returns to implement.
- `FlowDecision::{Advance, Iterate}`: one navigation enum, used for autonomous
  judgments and explicit human decisions through different authority endpoints.
- `FlowVerdict { decision, summary }`: the pending candidate and its evidence or
  next direction. `FlowProgress` retains per-edge counts, direction, and candidate.
- `ExecutionCursor`: ordinary Flow position, including a selected XOR child.
  Task's existing FlowPosition retains its Task claim and transaction authority.
  Both use `finish_step` for the meaning of edges.
- `Boundary`: one current attempt's exact identity and provider receipt. Provider
  success is necessary for an autonomous candidate; human approval is separate.
- Blocked invokes `ask_once` with a key derived from the exact Flow occurrence
  and pass. It joins the existing Ask and retains the completion summary.

The visible protocol is `lf flow decide advance|iterate "evidence/direction"`.
`lf flow blocked "reason and question"` opens Ask running unblock, waits for
human Complete, and returns the answer for reassessment. The existing lf
Codex/Claude Code launch path remains in use. No Jev/Pydantic SDK was added.

Jev's bounded judgment interface and Pydantic's validation/correction semantics
informed this contract; sources and qualified inferences are in
`decision-protocol-research.md`. A schema-valid decision is not proof of a good
judgment. Correction feedback is not another implementation pass. Calibrated
probabilities are not invented for generative agents.

## Concrete simplifications in this slice

The decision belongs to one dedicated skill. Review skills produce evidence.
The human and autonomous APIs use one navigation enum. The old on-disk temporary
Skill replacement has been removed; ordinary Flow launches use captured Skill
content. Human boundaries use existing Flow Sessions instead of a terminal-only
yes/no confirmation. Their approval is persisted before continuation is requested
and the departing provider is stopped. An operation interrupted without a receipt
stays inspectable; it is never silently replayed as if it had not happened.

## Findings repaired

- A saved candidate followed by failure/interruption must not advance.
- Human provider completion must not be interpreted as approval.
- Checkpointing Waiting could overwrite a concurrently saved human approval.
  Same-position checkpointing now preserves the decision; a regression covers it.
- Generic human revision initially guessed the preceding reviewer's target.
  Builtins now author the targets. Historical gates without an edge retain the
  same preceding-autonomous-step behavior in Task and ordinary execution.
- Detached successor launches must clear the departing Flow/Session authority.
- An invalid human Iterate must leave the review available for a valid decision.

## Remaining acceptance gaps to keep visible

The broader anywhere-Flow contract is not fully proven. The Wave playhead still
has its own forward/wrap interpreter and hidden singleton flow-step execution.
Task's existing loader still excludes XOR. Ordinary XOR recovery pins a selected
child, but not every unselected child/router at invocation creation. These are
explicit unresolved integration findings, not accepted exclusions or grounds to
ship the branch. `loopflow-entrypoints.md` contains the full evidence matrix.

Live Ghostty/provider approval → implementation → Ask → reassessment has not
been demonstrated by the fixtures. Opening the requested concept-review Session
is a review handoff, not that runtime acceptance demo. Full CI and hosted app
interaction remain separate checks. No global installation, production Home
migration, push, merge, or Task completion belongs to this review.

Review the model first: does one authored Flow plus one saved position make the
interaction natural, and can the remaining Wave/Task adapters be reduced to the
same executor without retaining competing semantics? Preserve the accepted
anywhere-Flow requirement while making that reduction.

## Validation record

The first integrated selection passed 25 reducer/engine/Task/store tests. Keyed
Ask's 12 tests passed. Ordinary persistence's initial three tests passed; a
later selection passed its four tests including the checkpoint race, the real
CLI executor's durable human wait/recovery test, authored human-edge parsing,
and the revised Task delivery-loop fixture. That selection also caught one
unrelated build-flow source assertion changed too broadly during test repair;
it was restored. The final focused selection verifies that repair and Session
integration. `cargo clippy --all-targets -j4 -- -D warnings` passed after all
runtime edits. Architecture ownership and whitespace checks passed.

The personal concept-review/loop-decide/unblock Skills were refreshed from
canonical sources. The generic Codex skill validator rejects Loopflow's generated
`loopflow`/`loopflow-skill` metadata; this is a validator-format mismatch, not a
reported validation pass. Builtin discovery/contract tests cover the canonical
Loopflow format. No provider judgment quality or live transport is inferred.

Final integrated selection: **22 passed, 0 failed**, covering ordinary durable
wait/recovery, pending-decision and human-approval fencing, nested human Session
recovery, build-flow contract, exact/stale Task human decisions, concurrent
settlement, and durable-child context scrubbing. Final formatting, whitespace,
and architecture checks passed. The preceding all-target Clippy pass applies to
these same runtime bytes; only evidence notes changed afterward.

The installed CLI catalog does not yet expose concept-review. The requested
Ghostty review therefore launches through installed lf's existing inline/TUI
path with the canonical concept-review instructions, without replacing the CLI
or migrating a production Home to test this branch.
