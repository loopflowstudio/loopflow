Review the diff on the current branch against `main` and produce a written assessment.

The deliverable is a review document under `.design/`, not fixes. Do not edit any files.

## Process

1. Run `git diff main...HEAD` to see committed changes on this branch
2. Run `git diff` to see uncommitted changes
3. Review against STYLE.md and general code quality
4. Write your assessment to a new markdown file under `.design/` (pick a short descriptive name, create the folder if needed)

## What to look for

**Style guide violations.** Read STYLE.md. Check naming, error handling, documentation patterns, test quality.

**Bugs.** Logic errors, edge cases, off-by-ones, unhandled errors.

**Unnecessary complexity.** Code that could be simpler. Abstractions that don't earn their keep. Features beyond what was asked.

**Missing pieces.** Tests that should exist. README updates for changed behavior.

## What to ignore

Don't flag things unrelated to this branch's changes. Stay focused on what's in the diff.

**Design doc deviations.** If any `.design/*.md` docs exist, treat the implementation as the source of truth. The design docs were scaffolding—deviations are likely intentional refinements discovered during implementation. Evaluate the code at face value for bugs and style issues, not for fidelity to the original plan.

## Output

Write a brief review covering:
- **Summary**: What does this change do? (1-2 sentences)
- **Issues**: Bugs, style violations, or concerns (if any)
- **Suggestions**: Optional improvements (if any)
- **Verdict**: Ready to ship, or needs work?

Keep it concise. If there's nothing to flag, say so.
