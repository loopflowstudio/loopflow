---
layout: default
title: lf ops Commands
---

# lf ops Commands

Operations and utilities. `lf ops` handles everything that's not launching a prompt.

## lf ops cp

Copy context to clipboard for use with web clients.

```bash
lf ops cp                     # copy repo docs
lf ops cp src tests           # copy specific paths
lf ops cp -e "*.pyc"          # exclude patterns
lf ops cp --no-lfdocs         # skip repo docs
```

Options:

| Flag | Description |
|------|-------------|
| `-e, --exclude` | Exclude patterns |
| `--lfdocs / --no-lfdocs` | Include wave/, scratch/, and root .md files |

---

## Git Workflow

## lf ops pr

Create or update a PR, open in browser.

```bash
lf ops pr --title "area: short title" --body "## Summary ..."
```

`--title` and `--body` are always required. Use `lf pr` to generate them with agent judgment.

## lf ops land

Submit PR to merge queue.

```bash
lf ops land
```

Enables auto-merge on your PR. GitHub merges when CI passes and the merge queue clears. Run `lf ops wt prune` after merge completes to clean up.

## lf ops next

Preserve the current worktree and create a fresh branch.

```bash
lf ops next
```

Commits and pushes current changes, optionally rebases, then creates and pushes a timestamped successor branch in the same worktree. If the current PR is already merged, it first resets to the default branch and syncs from origin before creating the next branch.

## lf ops commit

Commit staged changes.

```bash
lf ops commit -m "message"
```

Stages changes and commits with the explicit message. If you want generated messaging, run the `lf commit` step.

Options:

| Flag | Description |
|------|-------------|
| `-p, --push` | Push after committing |

## lf ops release

Mechanical release subcommands. Use `lf release` for the full agent-orchestrated workflow.

```bash
lf ops release check              # PRs merged since last tag?
lf ops release notes 1.2.3        # generate RELEASE_NOTES.md
lf ops release bump 1.2.3         # bump manifests
lf ops release tag 1.2.3          # create + push git tag
lf ops release status              # workflow + GitHub Release status
```

---

## lf ops doctor

Check dependencies.

```bash
lf ops doctor
```

Verifies that required tools are installed and working.

## lf ops rebase

Rebase current branch onto main.

```bash
lf ops rebase
```

Fetches main and rebases. On conflicts, exits with conflict details for manual resolution.

## lf ops sync

Update local main to match origin.

```bash
lf ops sync
```

Fetches `origin/main` and updates your local main branch. Safe to run from any worktree.

## lf ops wt

Worktree helper commands.

### lf ops wt create

Create worktree with schema-based branch name.

```bash
lf ops wt create my-feature            # creates ../loopflow.my-feature
lf ops wt create my-feature -b develop # from develop instead of main
lf ops wt create feature-B --stack     # stack on current branch
lf ops wt create jack-heart.mobile.20260225_1122  # checks out origin branch in ../loopflow.mobile
```

| Flag | Description |
|------|-------------|
| `-b, --base` | Base branch (default: main) |
| `-s, --stack` | Stack on current branch (branch from it, PR targets it) |

When using `--stack`, the new worktree branches from the current branch instead of main.

If the input matches an existing `origin/<branch>` name, `lf` checks out that branch instead of creating a new one. Worktree directory names always use the wave component (`{name}`), not the full branch metadata.

### lf ops wt switch

Switch to a worktree by its short directory name.

```bash
lf ops wt switch my-feature       # switches to ../loopflow.my-feature
```

### lf ops wt list

List worktrees with prunable metadata.

```bash
lf ops wt list
lf ops wt list --format json
```

### lf ops wt ci

Show CI status for the current branch.

```bash
lf ops wt ci
```

### lf ops wt prune

Remove worktrees whose branches have been merged.

```bash
lf ops wt prune           # show what would be pruned
lf ops wt prune --dry-run # show what would be pruned
lf ops wt prune --force   # remove prunable worktrees
```

Finds worktrees where the branch is an ancestor of `origin/main` (handles squash merges). Never prunes main/master or worktrees with uncommitted changes (scratch/ files are excluded).

| Flag | Description |
|------|-------------|
| `--dry-run` | Show what would be pruned without removing |
| `--force` | Skip confirmation prompt |

## lf ops abandon

Abandon a branch: close PR, remove worktree, delete branch.

```bash
lf ops abandon feature-branch
lf ops abandon feature-branch --force   # skip confirmation
```

| Flag | Description |
|------|-------------|
| `-f, --force` | Skip confirmation and force abandon with uncommitted changes |

## lf ops shell

Shell integration setup.

```bash
lf ops shell init       # print shell integration code
lf ops shell install    # install to shell config file
```

Installs a wrapper that sources shell directives after `lf` commands (auto-cd into new worktrees).

## Typical Workflow

```bash
lf ops wt create my-feature       # create worktree (../my-feature)
lf ops wt switch my-feature       # switch to worktree from another
# ... work on feature ...
lf ops commit                     # commit with generated message
lf ops pr                         # open PR (CI runs automatically)
# ... address review feedback ...
lf ops commit -p                  # commit and push
lf ops wt ci                      # check CI status
lf ops land                       # submit to merge queue
# ... wait for CI to pass and merge ...
lf ops wt prune                   # cleanup merged worktrees
```

## See Also

[`lf` reference](lf.md) · [Get Started](getting-started.md) · [Configuration](config.md)
