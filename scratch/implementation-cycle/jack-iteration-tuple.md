# Jack's decision: Flow iteration is a tuple

2026-09-25, recorded by the Claude control conversation (run_1357ccbd…) from
Jack's direct messages: "Feature has TWO loops", "the iteration counter should
just be a tuple", "iteration is not a single number now". It supersedes the
parent's counter correction in `parent-flow-counter-review.md`, which labels
every loop span from the shared scalar `ExecutionCursor.iteration`.

Required change to the cycle-4 Flow surface, before demo:

- Rust `FlowReturn` / `PinnedTaskFlow`: expose the iteration as the ordered
  per-edge return counts at each nesting level (authored order), derived from the
  existing `FlowProgress.repeats`. Drop the shared-scalar `FlowReturn.iteration`
  field rather than keeping both.
- Swift `TaskFlowView`: each loop region shows its own count, and the Flow header
  shows the tuple, e.g. "Iteration (2, 1)". No Swift reconstruction or Feature
  hardcoding.
- Session membership iteration uses the same tuple for its Flow step. Check
  whether `ExecutionCursor.iteration` can then be removed. It is part of
  `boundary_key` and exact human-boundary identity, so the proof must show that
  saved positions, stale-decision rejection and Session occurrence keep working
  before deleting it.
- Fixtures move together in Rust and Swift; no defaults.

Ownership: the compress child (PID 34419) was mid-edit when this was recorded.
The control conversation will make this change after that child exits, unless
the parent has already taken it. Check `git status` and this file's
"Status" line first.

Status: implemented and focused proof passes; see parent-iteration-tuple.md. Independent review and configured demo remain.
