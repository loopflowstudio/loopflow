# newux

UX improvements for Maestro and standardization on `.md` task file extensions.

## Review

**Verdict:** Ready to ship

Clean, well-organized work across two concerns: (1) consolidating UX research into a 3-task pipeline and implementing fixes, (2) standardizing on `.md` extensions for task files. No bugs found. Code follows STYLE.md.

## What was built

### UX Pipeline (ux-research → ux-gaps → ux-fix)

Consolidated `ux-review` into `ux-research`, creating a 3-task pipeline for UX work:
- `ux-research`: Capture screenshots, audit visual issues, simulate user profiles
- `ux-gaps`: Compare Maestro against Figma/Cursor/Notion, identify gaps
- `ux-fix`: Implement priority fixes from research

### Maestro UX Fixes

**Setup improvements:**
- Progress stepper (3-step indicator showing install progress)
- Skip option for manual installation
- Better error messages with recovery commands

**Discoverability:**
- Task dropdown shows descriptions extracted from task files
- Mode toggle has tooltip explaining Auto vs Interactive
- Token count has icon and tooltip
- Empty states redesigned (worktrees, voices)
- Prompt input placeholder shows examples

**Power user features:**
- Model selector (claude, claude:opus, codex, etc.)
- Command preview showing the `lf` command that will execute
- Copy-to-clipboard for command preview

### Extension Standardization

- Task files now only recognized with `.md` extension
- `.lf` extension support removed from `gather_task()` and `list_user_tasks()`
- Existing `.lf/*.lf` files renamed to `.claude/commands/*.md`
- Documentation updated throughout

## Design notes

**Job to be done reframing**: The UX gaps analysis reframes Maestro's purpose from "manage worktrees and launch LLM coding sessions" to "make progress on my codebase while I do something else." This is captured in `.design/ux-gaps.md` and `.design/questions.md`.

**Remaining high-impact gaps** (from ux-gaps.md, not addressed in this branch):
1. Output happens in external terminal (requires architectural change)
2. No onboarding walkthrough for new users
3. Configuration before expression (users must configure before typing)

**Open questions** captured in `.design/questions.md`:
- Should Maestro work without lf CLI installed?
- Should worktrees be hidden from beginners?
- Should output move into Maestro?
- What's the iOS/iPad story?

These are strategic questions for future work, not blockers for this branch.
