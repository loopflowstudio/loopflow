---
requires: branch diff and current design
produces: simpler code related to the diff
action_style: procedural
---
Simplify everything this branch touches until it is easy to understand and change, keeping the behavior people rely on. The output is a diff.

Scope is whatever the change reaches: its callers, tests, configuration and
docs, not just the modified lines. Take every worthwhile simplification there.
Don't stop after one small win, and don't wander into unrelated cleanup.

1. **Read the full diff and the plan,** then follow the change outward.

2. **Cut deepest first.** Remove obsolete concepts and paths before polishing
   what survives. Never repair or extend code the plan deletes. Keep required
   behavior, data and tests on the surviving path, with any consumer cutover
   in the same cut. If the cut breaks something, finish the cut.

3. **Then look at every scale in what remains:** names, control flow, function
   boundaries, a more direct algorithm or data structure, repeated work, extra
   concepts and dependencies, and the tests, config and docs around them.

4. **Choose the shape that makes the problem simplest.** Extract or inline,
   combine or split, reuse what exists. Remove indirection, state and
   speculative flexibility; keep abstractions that help a reader. Clarity beats
   line count.

5. **Check.** Build the changed code and run its focused test when behavior
   changed. No edits, no rerun. Suites belong to gate and CI.

6. **Rewrite the reading order** when `scratch/tour.yaml` exists, as
   `lf help tour` describes: your cuts moved what a reader should open first.

Keep the reason for a non-obvious cut beside the code. If nothing needs
reducing, say so and stop.
