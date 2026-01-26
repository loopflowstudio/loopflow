---
requires: code on branch
produces: scratch/<branch>.md with verdict
---
Review the diff on this branch and produce a written assessment.

The goal is to put something in the human's hands quickly. A draft they can shape—add issues you missed, remove nitpicks they disagree with, adjust the verdict. You can always be re-invoked. Don't aim for comprehensive; aim for useful and fast.

The deliverable is a consolidated design document under `scratch/`. Do not edit code files.

## Workflow

1. Run `git diff main...HEAD` to see all committed changes
2. Run `git diff` to see uncommitted changes
3. Run `git log main..HEAD --oneline` to understand commit history
4. Read any style guides in the repo
5. Check `roadmap/` (root and area-specific) for architecture context and prior decisions
6. Read existing `scratch/*.md` files
7. Write a single consolidated document to `scratch/<branch-name>.md`
8. Delete other `scratch/*.md` files after consolidating

## What to look for

Focus only on code in the diff. Don't flag unrelated issues.

**Bugs.** Logic errors, edge cases, off-by-ones, unhandled errors in the new code.

**Unnecessary complexity.** Abstractions that don't earn their keep. Features beyond what was asked. Over-engineering.

**Missing pieces.** Tests for new behavior. README updates for user-facing changes.

When noting issues, be specific and actionable. "This is complex" is useless. "4 nested conditionals; early returns would simplify" is actionable—whether a human or iterate picks it up.

## What to ignore

**Design doc deviations.** The implementation is the source of truth. Design docs were scaffolding—deviations are refinements, not bugs.

**Unrelated code.** Only review what's in the diff. This isn't a codebase audit.

**Generic best practices.** Don't flag things that work fine just because a linter would complain. Focus on actual problems.

## Consolidating design docs

Merge anything worth keeping from existing `scratch/` docs. Be aggressive about culling:

**Keep:** Decisions that explain non-obvious choices. User quotes that capture intent. "Not implemented" notes if still relevant.

**Delete:** Old reviews. Details obvious from the code. Done checklists. Outdated plans.

## Output format

Write `scratch/<branch-name>.md`:

```markdown
# <Branch Name>

<1-2 sentence summary>

## Review

**Verdict:** Ready to ship | Needs work

<Specific issues if any. Skip section if none.>

## Design notes

<Consolidated notes worth preserving. Skip if nothing non-obvious.>
```

Delete other `scratch/*.md` files after writing.
