---
requires: diff vs main
produces: scratch/<branch>.md with verdict
---
Does this work? Ship or iterate?

Fast. Decisive. Leave a useful review, make the verdict crystal clear.

## Workflow

1. `git diff main...HEAD` and `git diff` to see committed and uncommitted changes
2. `git log main..HEAD --oneline` to understand commit history
3. Read STYLE.md and compare the diff against it
4. Read existing `scratch/*.md` files
5. Write consolidated review to `scratch/<branch-name>.md`
6. Delete other `scratch/*.md` files after consolidating

## What matters

**Bugs.** Logic errors, unhandled edge cases, things that will break in production.

**Missing pieces.** Tests for new behavior. Obvious gaps in the implementation.

**Style violations that indicate confusion.** Inline imports, `_v2` suffixes, tests that assert on mock calls—these suggest the author didn't understand the codebase patterns.

**Unnecessary complexity.** Abstractions that don't earn their keep. Features beyond what was asked.

## What doesn't matter

**Style nitpicks.** Code that works is code that ships.

**Design doc deviations.** The implementation is the source of truth.

**Unrelated code.** Only review what's in the diff.

**Generic best practices.** Don't flag things that work fine just because a linter would complain.

## Consolidating scratch docs

Merge anything worth keeping from existing `scratch/` docs. Be aggressive about culling:

**Keep:** Decisions that explain non-obvious choices. "Not implemented" notes if still relevant.

**Delete:** Old reviews. Details obvious from the code. Done checklists. Outdated plans.

## Output

Write `scratch/<branch-name>.md`:

```markdown
# <Branch Name>

<1-2 sentence summary>

**Verdict: SHIP** or **Verdict: ITERATE**

## Issues
<Numbered list if ITERATE, or "None" if SHIP>

## Notes
<Anything worth mentioning but not blocking. Skip if nothing.>

## Design notes
<Consolidated notes worth preserving. Skip if nothing non-obvious.>
```

The verdict line must be unambiguous. SHIP or ITERATE, nothing in between.

Delete other `scratch/*.md` files after writing.
