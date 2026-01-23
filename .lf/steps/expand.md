---
requires: diff vs main
produces: code changes or scratch/<branch>-expansion.md
---
Is there anything missing to fulfill the MVP? If complete, explore what's next.

## Goal

First check: is this branch complete enough to ship? Not gold-plated, not perfect—just complete. If something obvious is missing, add it or note it.

If complete, optionally sketch the natural next step that would multiply the value.

## Workflow

1. `git diff main...HEAD` to see what changed
2. Read the implementation and any design docs
3. Ask: what would a user expect that isn't here?
4. If something small is missing, add it
5. If complete, consider: what's the natural "part 2"?
6. Write findings to `scratch/<branch>-expansion.md`

## What counts as missing

- Error handling for realistic failure modes
- Edge cases that users will hit
- CLI flags that the implementation supports but doesn't expose
- Tests for the main behavior (not exhaustive, just proof it works)
- A README update if user-facing behavior changed

## What doesn't count as missing

- Nice-to-haves that weren't in scope
- Performance optimizations
- Refactoring opportunities
- Future features that could build on this

## What makes a good expansion

If the MVP is complete, consider extensions that:

**Extend the branch's intent.** If this branch adds worktree support, expansion might add parallel execution. The expansion should feel like "part 2."

**Multiply value.** Changes that make everything else better, not just add one more thing.

**Stay tractable.** Achievable in one session. If it needs more than 500 words to describe, it's too big.

## Output

If complete with no expansion ideas:
```markdown
# Expansion check: <branch>

**Verdict: COMPLETE**

The implementation covers the expected functionality. No gaps identified.
```

If something's missing:
```markdown
# Expansion check: <branch>

**Verdict: GAPS**

## Missing
1. <What's missing and why it matters>

## Recommendation
<Add it now / Note for follow-up>
```

If complete with expansion opportunity:
```markdown
# Expansion check: <branch>

**Verdict: COMPLETE**

## Expansion opportunity
<What it is and why it's worth doing>
```

Write to `scratch/<branch>-expansion.md`. If the gap is small, fix it directly and note what you added.
