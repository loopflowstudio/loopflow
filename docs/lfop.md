---
layout: default
title: lf op Commands
---

# lf op Commands

Operations and utilities. `lf op` handles everything that's not launching a prompt.

## lf op cp

Copy context to clipboard for use with web clients.

```bash
lf op cp                     # copy repo docs
lf op cp src tests           # copy specific paths
lf op cp -e "*.pyc"          # exclude patterns
lf op cp --no-lfdocs         # skip repo docs
```

Options:

| Flag | Description |
|------|-------------|
| `-e, --exclude` | Exclude patterns |
| `--lfdocs / --no-lfdocs` | Include wave/, scratch/, and root .md files |

---

## Git Workflow

## lf op pr

Create or update a PR, open in browser.

```bash
lf op pr --title "area: short title" --body "## Summary ..."
```

`--title` and `--body` are always required. Use `lf pr` to generate them with agent judgment.

## lf op land

Submit PR to merge queue.

```bash
lf op land
```

Enables auto-merge on your PR. GitHub merges when CI passes and the merge queue clears. Run `lf op wt prune` after merge completes to clean up.

## lf op next

Preserve the current worktree and create a fresh branch.

```bash
lf op next
```

Commits and pushes current changes, optionally rebases, then creates and pushes a timestamped successor branch in the same worktree. If the current PR is already merged, it first resets to the default branch and syncs from origin before creating the next branch.

## lf op commit

Commit staged changes.

```bash
lf op commit -m "message"
```

Stages changes and commits with the explicit message. If you want generated messaging, run the `lf commit` step.

Options:

| Flag | Description |
|------|-------------|
| `-p, --push` | Push after committing |

## lf op release

Mechanical release subcommands. Use `lf release` for the full agent-orchestrated workflow.

```bash
lf op release check              # PRs merged since last tag?
lf op release notes 1.2.3        # generate RELEASE_NOTES.md
lf op release bump 1.2.3         # bump manifests
lf op release tag 1.2.3          # create + push git tag
lf op release status              # workflow + GitHub Release status
```

---

## lf op doctor

Check dependencies.

```bash
lf op doctor
```

Verifies that required tools are installed and working.

## lf op rebase

Rebase current branch onto main.

```bash
lf op rebase
```

Fetches main and rebases. On conflicts, exits with conflict details for manual resolution.

## lf op sync

Update local main to match origin.

```bash
lf op sync
```

Fetches `origin/main` and updates your local main branch. Safe to run from any worktree.

## lf op wt

Worktree helper commands.

### lf op wt create

Create worktree with schema-based branch name.

```bash
lf op wt create my-feature            # creates ../loopflow.my-feature
lf op wt create my-feature -b develop # from develop instead of main
lf op wt create feature-B --stack     # stack on current branch
lf op wt create jack-heart.mobile.20260225_1122  # checks out origin branch in ../loopflow.mobile
```

| Flag | Description |
|------|-------------|
| `-b, --base` | Base branch (default: main) |
| `-s, --stack` | Stack on current branch (branch from it, PR targets it) |

When using `--stack`, the new worktree branches from the current branch instead of main.

If the input matches an existing `origin/<branch>` name, `lf` checks out that branch instead of creating a new one. Worktree directory names always use the wave component (`{name}`), not the full branch metadata.

### lf op wt switch

Switch to a worktree by its short directory name or full branch name.

```bash
lf op wt switch my-feature       # switches to ../loopflow.my-feature
lf op wt switch jack.my-feature.20260316_1856  # resolves that exact branch's worktree
```

### lf op wt list

List worktrees with prunable metadata.

```bash
lf op wt list
lf op wt list --format json
```

### lf op wt ci

Show CI status for the current branch.

```bash
lf op wt ci
```

### lf op wt prune

Remove worktrees whose branches have been merged.

```bash
lf op wt prune           # show what would be pruned
lf op wt prune --dry-run # show what would be pruned
lf op wt prune --force   # remove prunable worktrees
```

Finds worktrees where the branch is an ancestor of `origin/main` (handles squash merges). Never prunes main/master or worktrees with uncommitted changes (scratch/ files are excluded).

| Flag | Description |
|------|-------------|
| `--dry-run` | Show what would be pruned without removing |
| `--force` | Skip confirmation prompt |

## lf op abandon

Abandon a branch: close PR, remove worktree, delete branch.

```bash
lf op abandon feature-branch
lf op abandon feature-branch --force   # skip confirmation
```

| Flag | Description |
|------|-------------|
| `-f, --force` | Skip confirmation and force abandon with uncommitted changes |

## lf op shell

Shell integration setup.

```bash
lf op shell init       # print shell integration code
lf op shell install    # install to shell config file
```

Installs a wrapper that sources shell directives after `lf` commands (auto-cd into new worktrees).

## Typical Workflow

```bash
lf op wt create my-feature       # create worktree (../my-feature)
lf op wt switch my-feature       # switch to worktree from another
# ... work on feature ...
lf op commit                     # commit with generated message
lf op pr                         # open PR (CI runs automatically)
# ... address review feedback ...
lf op commit -p                  # commit and push
lf op wt ci                      # check CI status
lf op land                       # submit to merge queue
# ... wait for CI to pass and merge ...
lf op wt prune                   # cleanup merged worktrees
```

## See Also

[`lf` reference](lf.md) · [Get Started](getting-started.md) · [Configuration](config.md)
