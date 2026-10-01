---
requires: branch diff and current design
produces: simpler code related to the diff
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
   exclusive tests/fixtures. Preserve required behavior, data, and tests on the
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

5. Sanity-check the reduction: build changed code and run its focused test
   when behavior changed. Reuse applicable results; no edits means no rerun.
   Fix actual failures. If a check cannot run headless, use a headless equivalent
   or leave it to gate/CI; do not stop or block the Flow for it. Human judgment
   belongs to demo/review. Gate owns affected suites and acceptance checks once.

Keep rationale beside the code or in the plan, and one command/result line
with any deferred owner. No pass ledger. If nothing needs reducing, say so.
Realign reconciles the plan.

## Keep authored context within budget

Use the assembled `lf:context-budget` snapshot, or run `lf context --skill compress`
to read effective limits, their configuration sources, and current usage. Before
updating scratch or Wave memory, read complete sources named by excerpt pointers.
Bring over-budget material under both token and byte limits as part of this step.
Merge duplicates, summarize long evidence, remove obsolete notes inherited from a
stacked parent, and keep historical detail in git rather than ambient context.
Preserve live decisions, attribution, unresolved work, and contrary evidence;
keep a precise git reference when older detail still matters. Preserve uncommitted
evidence before removing it. Edit existing notes instead of accumulating reports.
Re-run the query after writing. Do not raise limits to conceal overflow. If the
live decisions alone cannot fit, record the concrete conflict and remaining overage.
