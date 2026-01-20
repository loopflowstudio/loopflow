# Questions

## Rebase blocked: step-flow-loop branch

**Date:** 2026-01-20

**Issue:** Cannot rebase `step-flow-loop` onto main due to fundamental README.md conflicts.

**What happened:**
- This branch has 7 commits explaining the "step, flow, loop" conceptual structure in README.md
- Main has since been restructured with a different approach: demo GIFs, "Quick fix", "Feature workflow", "Background agents" sections
- The two documentation approaches are incompatible—they organize and explain the same features in completely different ways

**Decision needed:**
1. Abandon this branch (main's approach supersedes it)
2. Manually integrate the step/flow/loop conceptual explanations into main's new structure
3. Keep this branch's approach and replace main's README

The conflict is not about code correctness—both versions are valid documentation. This is a product decision about how to present Loopflow.
