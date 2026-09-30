---
requires: branch diff, current design, and runnable focused proof
produces: simpler code related to the diff and executed proof
action_style: procedural
---
Simplify anything related to the branch diff. Make the code easier to understand
and change while preserving required behavior. The output is a diff.

1. Read the full diff and current design, then follow the affected code into
   its surrounding implementation, callers, tests, configuration, and docs.
   Scope follows the relationship to the change, not just modified lines.
   Take every worthwhile simplification you find in that scope; do not stop
   after one small win or wander into unrelated cleanup.

2. Make the deepest cuts first. Read or update the plan's **Delete — do not
   maintain** list of concrete files/symbols and their exclusive tests/fixtures.
   Remove obsolete concepts, authorities, and paths before polishing surviving
   code. Never repair, refactor, or extend code the plan removes, including its
   exclusive tests/fixtures. Preserve required behavior, data, and proof on the
   surviving path; include any minimum consumer cutover or migration in the same
   cut. Temporary breakage within that cut calls for finishing it, not repairing
   the predecessor. Keep remaining deletion targets current for the next pass.
   Additive work needs no invented deletion.

3. Then look at every scale in what remains. Clearer names,
   simpler control flow, better function boundaries, a more direct algorithm or
   data structure, less repeated work, and fewer concepts or dependencies all
   count. So do simpler tests, configuration, and documentation. These are
   starting points, not an exhaustive checklist.

4. Choose the shape that makes the actual problem simplest. Extract or inline,
   combine or split, share a real common idea or separate cases that were forced
   together. Reuse existing capabilities when they fit. Remove unnecessary
   indirection, state, and speculative flexibility; retain abstractions that
   help readers. Clarity matters more than brevity or line count.

5. Make coherent edits and verify the affected behavior with focused proof.
   If proof fails, repair the reduction; if required proof cannot run, stop with
   its exact blocker. Reuse applicable passing evidence; gate and CI own broader
   suites.

Leave consequential rationale beside the changed code or in the existing plan.
Summarize the simplification, applicable proof, and unresolved gaps briefly so
the next reader can use them. No separate pass record is required. If nothing
needs reducing, say so without manufacturing edits. Realign reconciles the plan
with the work.
