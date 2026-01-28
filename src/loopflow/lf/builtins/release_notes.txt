Generate release notes for a version bump.

You're writing for someone scanning PyPI or GitHub releases to decide if they should upgrade.

Do not ask questions. If anything is unclear, make the best assumption and proceed.

## Philosophy

Translate implementation into experience. A series of small commits might add up to "the daemon is now reliable" or "Concerto feels snappier." Find that thread.

Ask: What can users do now that they couldn't before? What's easier? What's fixed? What feels different?

## For minor/major releases

When summarizing many commits, the commit messages alone won't tell the full story. Use `git diff` and read key files to understand actual impact. Look for:
- New commands or CLI flags users can run
- Changed behavior or defaults they'll notice
- Removed friction or fixed pain points
- New workflows that are now possible

## Output format

Return a structured response with:
- **summary**: 2-3 sentences capturing the feeling of this release. What's the headline? For minor releases, paint the bigger picture of how the tool has evolved.
- **changes**: bullet list of notable changes (5-10 items for minor releases, 3-8 for patches)

## Style

Lead with outcomes, not mechanisms.

Good:
- "Agents now persist their worktrees across iterations—no more lost context between runs"
- "New `lfops next` command: land a PR and immediately continue on a stacked branch"
- "The daemon auto-recovers from database schema changes instead of failing silently"

Bad:
- "Add worktree persistence to agent runner"
- "Refactored publish.py to use new pattern"
- "Updated schema migration logic"

Skip internal refactors unless they affect what users experience.
