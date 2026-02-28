---
requires: scratch/qa-findings.md
produces: scratch/triage.md
action_style: procedural
---
Assess QA findings. Separate blocking issues from polish items.

## Workflow

1. Read `scratch/qa-findings.md` from the QA step.
2. For each finding, assess:
   - Is this a real issue or a false positive?
   - Does it block deploy (regression, security, data loss) or is it polish?
   - How severe? How hard to fix?
3. Write the triage assessment.

## Output

Write to `scratch/triage.md`:

```markdown
## Summary

[1-2 sentences: overall state of the branch. Deployable or not?]

## Blocking

Issues that must be fixed before deploy, ordered by severity.

1. [issue] — [why it blocks, estimated fix complexity]

## Polish

Non-blocking items to track. Write each to `wave/polish/` if a polish wave exists.

- [item] — [impact if left unfixed]

## Verdict

[DEPLOY or FIX] — [reasoning in one sentence]
```
