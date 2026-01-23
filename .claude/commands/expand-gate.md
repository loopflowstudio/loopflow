---
requires: diff vs main
produces: .design/<branch>-expansion.md or code changes
---
Is there anything missing to fulfill the MVP on this change?

## Goal

Look at what this branch does and ask: is it complete? Not gold-plated, not perfect—just complete enough to ship. If something obvious is missing, either add it or note it.

This is a quick check, not a feature planning session. If the answer is "yes, it's complete," say so and stop.

## Workflow

1. `git diff main...HEAD` to see what changed
2. Read the implementation and any design docs
3. Ask: what would a user expect that isn't here?
4. If something small is missing, add it
5. If something larger is missing, write `.design/<branch>-expansion.md`

## What counts as missing

- Error handling for realistic failure modes
- Edge cases that users will hit
- CLI flags that the implementation supports but doesn't expose
- Tests for the main behavior (not exhaustive, just proof it works)
- A README update if user-facing behavior changed

## What doesn't count

- Nice-to-haves that weren't in scope
- Performance optimizations
- Refactoring opportunities
- Future features that could build on this

## Output

If complete:
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
2. ...

## Recommendation
<Add it now / Note for follow-up / Out of scope>
```

Write to `.design/<branch>-expansion.md`. If the gap is small, fix it directly and note what you added.
