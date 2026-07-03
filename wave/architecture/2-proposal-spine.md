---
priority: high
---

# Proposal spine

**Finish line:** One real architectural simplification travels the full arc —
throwaway-worktree prototype → `proposed` → human design gate → `approved` — with
the prototype's outcome (works / cost) recorded on the proposal. Reduce has
proven it can carry a design decision through the gate, not just study.

## Context

Study bootstrap is done: the four analysis maps and `docs/architecture.md` are on
main, each analysis carries a HEAD freshness marker, and one assess pass can name
its next move from that state. The conversation subsystem — the false pressure
behind the session model — was pulled as reduce's first reduction, leaving a smaller live
split: `Run`, `Session`, `ExecutionProcess`, and agent/output events.

The live proposal is `proposals/session-record-spine.md` (status: `draft`). It
argues for one user-facing **Session Record** read model over the live layers,
so clients stop stitching `Session` + `Run` + `ExecutionProcess` by hand across
CLI, lfd, lfq, and Concerto. This item is the vehicle that takes *that* proposal
— or whichever proposal earns it — through the arc.

This is milestone 2 of the reduce arc. It is the first proof that the meta-wave
profile works end-to-end, so favor carrying one proposal all the way over
drafting several.

## The move, concretely

1. **Deepen the study that gates the design.** Before prototyping, close the
   continuity audit around session lifecycle (see `analysis/session-model-comparison.md`
   → "Next evidence to gather"):
   - Trace `lfq sessions`, `lfq attach`, and Concerto session rendering against
     the current DTOs.
   - Decide whether `AgentStarted`, `AgentEnded`, and `OutputLine` are legacy
     vocabulary or still load-bearing.
   - Decide whether Concerto opens ordinary terminals as standalone workspace
     panes or only as panes inside a main agent tmux session.

2. **Prototype in a throwaway worktree.** Build the read-model aggregator before
   touching storage:
   - Assemble an internal `SessionRecord` view from existing store tables.
   - Populate it for one active run: control session + process + latest events.
   - Render it in a narrow CLI/debug endpoint or script.
   - Compare against current `lfq sessions` and the Concerto session view.

   Prototype success = the aggregate makes existing behavior clearer without
   forcing a risky database migration. Record the outcome (works / cost /
   surprises) on the proposal; discard the prototype code.

3. **Surface at the gate and park.** Move the proposal `draft → prototyped →
   proposed`. Present it for human design agreement. Reduce cannot self-approve a
   change that shifts public vocabulary or DTO/API shape — so it parks the
   proposal at `proposed` and stays productive elsewhere rather than idling.

## Open questions to resolve at the gate

- Should `run_id` stay required for worker/wave sessions and optional only for
  palette sessions?
- Should Concerto support unmanaged terminal panes beside lf sessions, or should
  every terminal live inside a wave/agent tmux topology? If panes exist, what is
  the adoption path that turns one into a loopflow-managed session?
- Do `AgentStarted`, `AgentEnded`, and `OutputLine` survive as compatibility
  events or become session-event variants?
- Is resume/fork a Session Record operation in loopflow, or delegated to the
  underlying harness until all harnesses support it?

## Done when

- One proposal reaches `approved` in `proposals/` with a recorded prototype
  result (works / cost).
- Its prototype was a throwaway spike, not merged implementation.
- The design decisions it embodies are written down clearly enough that
  execution can proceed unbounded in size once approved.
