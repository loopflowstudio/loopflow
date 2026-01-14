# newrepos

Fix `lf ops init` to work and improve first-run experience for new repositories.

## Summary

`lf ops init` now works. Templates are bundled under `src/loopflow/templates/` and copied to the right places. Error messages detect uninitialized repos and suggest running `lf ops init`.

## What changed

**Templates directory:** All templates now live in `src/loopflow/templates/`:
- `config.yaml` - default loopflow config
- `STYLE.md` - generic style guide (no loopflow-specific references)
- `PROMPTS.md` - explains the prompt system for new repos
- `commands/*.md` - the prompt files themselves

**Starter prompts (6):** design, implement, review, debug, polish, iterate. Run `lf ops init --all` for additional prompts.

**Better error messages:** When a task is not found in an uninitialized repo, suggests `lf ops init` instead of showing confusing hints about creating `.claude/commands/`.

**`lf ops commit` command:** Generate commit message from diff and commit. Supports `-p` flag to push after commit.

**`lf ops doctor` improvements:** Now checks for task files and reports repo initialization status.
