---
requires: diff vs main
produces: .design/<branch>-review.md with verdict
---
Does this work? Ship or iterate?

Quality gate for inner loops. Fast. Decisive. Leave a useful review, make the verdict crystal clear.

## Workflow

1. `git diff main...HEAD` and `git diff` to see what changed
2. Check against STYLE.md
3. Look for bugs, not style nitpicks
4. Write review to `.design/<branch>-review.md`

## What matters

**Bugs.** Logic errors, unhandled edge cases, things that will break in production.

**Missing pieces.** Tests for new behavior. Obvious gaps in the implementation.

**Style violations that indicate confusion.** Inline imports, `_v2` suffixes, tests that assert on mock calls—these suggest the author didn't understand the codebase patterns.

## What doesn't matter

Style nitpicks. Documentation gaps. "Wouldn't it be nice" improvements. Code that works is code that ships.

## Output

Write `.design/<branch>-review.md`:

```markdown
# Review: <branch>

**Verdict: SHIP** or **Verdict: ITERATE**

## Summary
<2-3 sentences on what this change does>

## Issues
<Numbered list if ITERATE, or "None" if SHIP>

## Notes
<Anything worth mentioning but not blocking>
```

The verdict line must be unambiguous. SHIP or ITERATE, nothing in between.
