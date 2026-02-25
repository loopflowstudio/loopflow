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
lf ops pr
```

Generates a PR description based on the branch diff, creates the PR (or updates if one exists), and opens it in your browser.

Idempotent: run it to create, or again to update after more commits.

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

Generate commit message and commit.

```bash
lf ops commit
```

Stages changes, generates a commit message based on the diff, and commits.

Options:

| Flag | Description |
|------|-------------|
| `-p, --push` | Push after committing |

## lf ops lint

Run the configured lint command.

```bash
lf ops lint
```

Reads `.lf/config.yaml` and runs `lint:` from repo root. Use this to match your gate checks locally.

## lf ops test

Run the configured test command.

```bash
lf ops test
```

Reads `.lf/config.yaml` and runs `test:` from repo root.

## lf ops release

Publish a release: generate notes, land PR, tag, push.

```bash
lf ops release              # bump patch, publish
lf ops release minor        # bump minor
lf ops release 1.0.0        # explicit version
lf ops release -n           # dry run — show what would happen
```

Creates a worktree from `origin/main`, generates release notes via agent, commits, creates and lands a PR, then tags and pushes. CI picks up the tag and builds the release.

| Flag | Description |
|------|-------------|
| `version` | Bump type (`patch`, `minor`, `major`) or explicit version (default: `patch`) |
| `-n, --dry-run` | Preview the release without making changes |

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

Fetches main and rebases. If conflicts occur, launches an assistant to help resolve them.

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
lf ops wt create my-feature            # creates ../my-feature
lf ops wt create my-feature -b develop # from develop instead of main
lf ops wt create feature-B --stack     # stack on current branch
```

| Flag | Description |
|------|-------------|
| `-b, --base` | Base branch (default: main) |
| `-s, --stack` | Stack on current branch (branch from it, PR targets it) |

When using `--stack`, the new worktree branches from the current branch instead of main.

### lf ops wt switch

Switch to a worktree by its short directory name.

```bash
lf ops wt switch my-feature       # switches to ../my-feature
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

Finds worktrees where the branch is an ancestor of `origin/main` (handles squash merges). Never prunes main/master or dirty worktrees.

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
