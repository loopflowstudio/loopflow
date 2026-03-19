---
requires: scratch/tend-chord.md
produces: scratch/tend-review.md
interactive: true
---
Review what the chord already played.

## Goal

This is a retrospective human checkpoint, not an approval gate.

Show the human what changed, why it changed, and what it means for the member
waves. If they want to amend or revert something, do that deliberately and
record it.

## Opening

Orient the human quickly:

1. What assessment drove this chord
2. Which waves changed
3. The main tradeoff or risk introduced by the mutations

## Walkthrough

For each applied or skipped mutation:
- show the concrete before / after
- explain the rationale plainly
- note any risks or follow-up
- ask whether to **keep**, **amend**, or **revert** it

If the human wants an amendment or revert, make the smallest clean change that
matches their intent and update the chord record if needed.

## Cross-cutting questions

After the per-mutation walkthrough, zoom out:
- Did the chord address the real pressure?
- Did anything surprising happen during play?
- Is there a mutation that should be undone or added next cycle?
- Are the silent waves still correctly silent?

## Output

Write `scratch/tend-review.md`:

```markdown
# Chord Review — <date>

## Summary
<overall human reaction>

## Decisions
### <mutation title>
**Verdict**: keep | amend | revert
**Notes**: <human reasoning>
**Follow-up**: <if any>

## Session Notes
<trajectory observations, calibration notes, context to remember>
```

If the human asks for an amendment or revert, apply it before finishing and note
exactly what changed.

## What to avoid

**Pretending this is pre-approval.** The chord already landed. Review what
actually happened.

**Defensiveness.** If the human dislikes a mutation, treat that as signal.

**Vague summaries.** Tie each decision back to a concrete change.
