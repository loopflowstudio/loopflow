---
requires: scratch/qa-findings.md
produces: scratch/triage.md
action_style: procedural
---
Assess QA findings. Separate blocking issues from polish items.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- If a `wave/<name>/` directory matches this work, skim its `GOAL.md`, `MEMORY.md`, `projects/`, and live tasks (`lf op pm show --wave <name>`).
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

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
