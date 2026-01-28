# Dynamic budget allocation

Budgets are fixed per section (area: 50k, docs: 30k, diff: 20k). When an area is small, unused budget goes to waste while docs get truncated.

**Problem**: A step targeting a 10k-token area still reserves 50k, leaving less room for reference docs that might be crucial.

**Solution**: Shift unused budget between sections. If area uses 10k of its 50k budget, redistribute 40k to docs and diff proportionally.

**Effort**: Medium (modify `_limit_to_budget` to track actual usage, add reallocation pass)
