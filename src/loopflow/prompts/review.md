Review the diff on the current branch against `main` and fix any issues found.

## Deliverables

1. **Fixes** - Correct any issues in the code directly
2. **Review guide** - Create `<branch>.md` at repo root for human reviewers (see below)

## Process

1. Read STYLE.md if present
2. Examine the diff between main and current branch
3. Fix issues directly: bugs, style violations, missing tests
4. Run tests to verify fixes
5. Update the design doc

## What to look for

- Bugs and correctness issues
- Code clarity and maintainability
- Adherence to STYLE.md conventions
- Test coverage for new functionality

Fix issues as you find them. Don't just report problems—solve them.

## Review guide

Create (or update) a review guide at `<branch>.md` in the repo root.

To find the branch name:
```bash
git branch --show-current
```

For example, if the branch is `post-winter`, write to `post-winter.md`.

Write it for a human reviewer who hasn't seen this code yet. Help them understand:

- What this branch does and why
- Notable decisions or tradeoffs worth knowing about
- Anything risky, uncertain, or worth extra scrutiny
- What's unfinished (if anything)

If a design doc already exists, transform it—strip implementation details that are now in code, surface what's still relevant for review.

Keep it short. A few paragraphs, not a formal document.

`lf pr land` removes the review guide automatically.
