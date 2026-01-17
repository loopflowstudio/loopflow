# Commands

Adds `lfops rebase` command and improves task listing.

## Review

**Verdict:** Ready to ship

## Changes

### `lfops rebase`

New command that:
1. Fetches origin/main
2. Attempts `git rebase origin/main`
3. If successful, pushes with `--force-with-lease`
4. If conflicts occur, aborts the rebase and launches Claude with the rebase prompt

The `.claude/commands/rebase.md` prompt guides conflict resolution. The builtin fallback in `templates/commands/rebase.md` provides the same prompt if no repo-specific version exists.

### Improved `lf` task listing

Running `lf` without arguments now shows richer task metadata:
- Source location: `.claude`, `.lf`, or `builtin`
- Frontmatter fields: `interactive`, `requires`, `produces`
- Pipelines show task flow: `ship: implement → rebase → polish → draft_commit`

### Filtering improvements

Non-task files in `.lf/` are now excluded from the task list:
- Known config files: `config.yaml`, `COMMIT_MESSAGE.md`, `CHECKPOINT_MESSAGE.md`
- Uppercase-stem files (likely docs like `STYLE.md`, `PROMPTS.md`)

### Template cleanup

- Simplified `_scaffold_repo` - no longer copies commit message templates
- Streamlined `config.yaml` template with better defaults
