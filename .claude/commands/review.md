Review the diff on the current branch against `main` and produce a written assessment.

The deliverable is a consolidated design document under `.design/`. Do not edit code files.

## Process

1. Run `git diff main...HEAD` to see committed changes on this branch
2. Run `git diff` to see uncommitted changes
3. Review against STYLE.md and general code quality
4. Read all existing `.design/*.md` files to understand what's already documented
5. Write a **single consolidated document** to `.design/` that includes:
   - Your review assessment
   - Any still-relevant information from previous design docs
   - Delete all other `.design/*.md` files after consolidating

## What to look for in the review

**Style guide violations.** Read STYLE.md. Check naming, error handling, documentation patterns, test quality.

**Bugs.** Logic errors, edge cases, off-by-ones, unhandled errors.

**Unnecessary complexity.** Code that could be simpler. Abstractions that don't earn their keep. Features beyond what was asked.

**Missing pieces.** Tests that should exist. README updates for changed behavior.

## Consolidating design docs

After writing your review, merge it with anything worth keeping from existing `.design/` docs. Be aggressive about culling:

**Keep:**
- Decisions and rationale that explain non-obvious choices
- Architecture diagrams or data structure definitions still accurate to the implementation
- User quotes that capture intent
- "What's not implemented" lists if still relevant

**Delete:**
- Old reviews (superseded by your new one)
- Design details that match the obvious implementation
- "What's done" checklists (the code is the source of truth)
- Outdated plans or explorations
- Anything the README or code comments already explain

The goal is one lean document that helps the next person understand what was built and why, without duplicating what's obvious from reading the code.

## What to ignore in review

Don't flag things unrelated to this branch's changes. Stay focused on what's in the diff.

**Design doc deviations.** Treat the implementation as the source of truth. Design docs were scaffolding—deviations are intentional refinements. Evaluate code at face value, not for fidelity to the original plan.

## Output format

Write a single `.design/<branch-name>.md` file structured as:

```markdown
# <Branch Name>

<1-2 sentence summary of what this branch does>

## Review

**Verdict:** Ready to ship | Needs work

<Issues, if any. Skip section if none.>

## Design

<Consolidated design notes worth preserving. Skip section if nothing non-obvious to document.>
```

Delete all other `.design/*.md` files after writing the consolidated doc.

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.

