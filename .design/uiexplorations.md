# UI Explorations

Bundle LOOPFLOW.md in the package, add `lf ops commit` command, improve `lf ops pr land`, and enhance Maestro UI with hover actions.

## Review

**Verdict:** Ready to ship

## Design notes

### Bundled LOOPFLOW.md

Replaces repo-level `PROMPTS.md` with a package-bundled `LOOPFLOW.md`. The file is now included in prompts via `include_loopflow_doc` config option (default: true). This ensures agents always have context about loopflow's workflow conventions regardless of repo configuration.

### lf ops commit

New command with LLM-generated commit messages. Options:
- `-p/--push`: Push after commit (sets upstream if needed)
- `-m/--message`: Override generated message
- `-a/-A/--add/--no-add`: Stage all changes (default: yes)

### lf ops pr land simplification

Changed from manual squash-merge to `gh pr merge --squash --delete-branch`. Benefits:
- PR shows as "merged" not "closed"
- GitHub handles the merge remotely
- Cleaner local state after merge

### Maestro hover actions

Worktree rows now show terminal/IDE quick-action buttons on hover, replacing the status badge. Context menu labels use configured app names (Warp, iTerm, etc.) instead of generic "Terminal".

### Open question: Prompt frontmatter

See `questions.md` for details on proposed task file frontmatter support.
