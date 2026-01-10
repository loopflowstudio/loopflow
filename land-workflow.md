# Design: Landing Workflows

Two ways to land a branch, same commit message generation.

## Commands

```bash
lf land              # local merge workflow (uses wt merge under the hood)
lf pr create         # create/update PR with generated title/body
lf pr land           # land via PR (squash merge, uses PR title/body)
```

## Config

```yaml
# .lf/config.yaml
pr: true   # default: lf land warns, suggests lf pr land
pr: false  # default: lf land just works, lf pr land still available
```

## Commit message prompts

Both workflows use customizable prompts for generating messages:

- `.lf/COMMIT_MESSAGE.md` - prompt for generating commit messages
- `.lf/CHECKPOINT_MESSAGE.md` - prompt for generating PR title/body

These are installed by `lf meta init` and can be edited. Falls back to built-in defaults if not present.

**Not** in `.claude/commands/` because these aren't runnable tasks - they're templates for message generation.

## State awareness

| Situation | `lf land` | `lf pr land` |
|-----------|-----------|--------------|
| `pr: false`, no PR exists | ✓ Works | ✓ Works (creates PR first) |
| `pr: false`, PR exists | ⚠ Warn: "PR exists, use `lf pr land` or `--force`" | ✓ Works |
| `pr: true`, no PR exists | ⚠ Warn: "PR workflow enabled, use `lf pr create` first or `--no-pr`" | ✓ Creates PR, then lands |
| `pr: true`, PR exists | ⚠ Warn: "PR exists, use `lf pr land`" | ✓ Works |

## Flags

```bash
lf land --no-pr      # bypass PR check, land locally even if pr: true
lf land --force      # bypass PR-exists check
lf pr land --create  # create PR if it doesn't exist (default behavior)
```

## lf land workflow

```
1. Check config & state (warn if appropriate)
2. Generate commit message from diff (using customizable prompt)
3. Remove design docs (.design/) contents if present
4. Squash commits with generated message
5. wt merge --no-squash (rebase, hooks, merge, push, cleanup)
```

## lf pr land workflow

```
1. Ensure PR exists (create if --create or pr: true)
2. Get PR title/body
3. Remove design docs (.design/) contents if present
4. Squash merge to base branch with PR title/body as message
5. Push
6. Clean up worktree (wt remove)
```

## lf pr create workflow

```
1. Generate PR title/body from diff (using customizable prompt)
2. Create or update PR via gh cli
3. (Optional) Open in browser
```

## Decisions

1. **`lf pr land` message source**: Use existing PR title/body (respect manual edits)

2. **`lf land` when PR exists**: Warn, require `--force` to proceed

3. **Base branch for `lf land`**:
   - Default: Let `wt merge` use repo's default branch
   - Override: `lf land --base develop` passes through to `wt merge develop`

## Implementation order

1. Add customizable prompts (`.lf/COMMIT_MESSAGE.md`, `.lf/CHECKPOINT_MESSAGE.md`)
2. Update `llm_http.py` to check for user prompts first
3. Update `lf meta init` to install prompt templates
4. Add `lf land` command using wt merge
5. Add state awareness (PR exists check, config check)
6. Add flags for overrides (`--force`, `--no-pr`, `--base`)
