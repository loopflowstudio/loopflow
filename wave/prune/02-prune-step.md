# 02: Prune Step

**Finish line:** `.lf/steps/prune.md` exists and successfully cleans up done worktrees when run with `lf prune`.

## What to build

An agent step that runs after deterministic `wt prune`. The backstop — catches what Rust doesn't handle yet.

### Workflow the step follows

1. `lf ops wt list --format json` — structured worktree state
2. `lf ops wt prune` — dry run to see what deterministic code catches
3. `lf ops wt prune --force` — remove those
4. For remaining non-main worktrees, investigate each:
   - `gh pr list --head <branch>` — PR state?
   - `git log --oneline -5 <branch>` — recent activity?
   - `git log origin/main --grep="<branch>"` — branch name in main commit messages?
   - Check if work appears in main under different SHAs (squash merges)
5. Remove worktrees that are clearly done (PR merged/closed, work in main)
6. Flag `landed-dirty` worktrees with clear instructions for the human
7. Report summary: what was removed, what needs attention

### Tone

Aggressive. Warns at the top that it will delete worktrees. Cleanup tool, not safety tool.

### Step format

Follows PROMPT_STYLE. Frontmatter with `requires:` and `produces:`. Headless-compatible — the step should work in both interactive and headless surfaces.

## What's available

Sprint 01 shipped. `wt list --format json` now includes `dirty` (bool) and `remote_gone` (bool) on every `WorktreeState`. `wt prune --force` removes remote-gone clean worktrees alongside merged ones. `wt list` shows `landed-dirty` (red, `merged && dirty`) and `remote-gone` (yellow, `remote_gone && !merged`) as first-class states. The step can rely on all of this.

## Done when

1. `.lf/steps/prune.md` exists with proper frontmatter
2. Step runs successfully from any worktree
3. Step never removes dirty worktrees or main
4. Step reports what it did and what needs manual attention
