---
layout: default
title: lf op Commands
---

# lf op Commands

Operations and utilities. `lf op` handles everything that's not launching a prompt.

## lf op cp

Copy context to clipboard for use with web clients.

```bash
lf op cp                          # copy default context (agent doc, LOOPFLOW.md, scratch/, wave/)
lf op cp src tests                # copy specific paths
lf op cp -e "*.pyc"               # exclude patterns
lf op cp --docs README.md,swift/  # prefetch additional docs
```

Options:

| Flag | Description |
|------|-------------|
| `-e, --exclude` | Exclude patterns |
| `--docs PATH[,PATH...]` | Prefetch additional docs—files, globs, or dirs (default: none) |

---

## Git Workflow

## lf op pr

Create or update a PR, open in browser.

```bash
lf -m codex op pr
lf op pr -m codex
lf op pr --title "area: short title" --body "## Summary ..."
```

Use `-m/--model` for a one-off agent override when `lf op pr` needs a different harness than your configured default. When omitted, `lf op pr` uses `agent:` from `.lf/config.yaml` or `~/.lf/config.yaml`.

`--title` and `--body` are always required. Use `lf pr` to generate them with agent judgment.

Before opening or updating the PR, Loopflow syncs the default branch in the main repo so the PR is based on current upstream state even when you run `lf op pr` from a sibling worktree.

## lf op land

Submit PR to merge queue.

```bash
lf op land
```

Enables auto-merge on your PR. GitHub merges when CI passes and the merge queue clears. Run `lf op wt prune` after merge completes to clean up.

## lf op queue reconcile

Run one merge-queue reconcile pass over stacked wave runs.

```bash
lf op queue reconcile               # every wave with queue state
lf op queue reconcile --wave infra  # just one wave
```

Infers stack status from git and GitHub, flips PRs between draft and ready, lazily rebases stack heads, and records queue blocks as attention. Prints one line per wave — `reconciled` or `blocked (<reasons>)`. `lfd` execs the same verb when a PR-merged webhook arrives; running it by hand is always safe.

| Flag | Description |
|------|-------------|
| `-w, --wave NAME` | Only reconcile this wave (default: every wave with queue state) |

## lf cron

Install local launchd jobs that run `lf` commands on a schedule. (Top-level, not
under `lf op` — the cron is a local scheduler, not a git operation.)

```bash
lf cron add --wave memory --flow export-memory --schedule daily
lf cron list
lf cron remove --wave memory --flow export-memory
```

`add` writes `~/Library/LaunchAgents/loopflow.cron.<wave>.<flow>.plist` and loads it with launchd. The job runs from the current repo with `ProgramArguments` set to `lf <flow> --wave <wave>`.

| Command | What it does |
|---------|--------------|
| `add` | Install or replace a launchd cron job |
| `list` | Show installed loopflow cron jobs |
| `remove` | Unload and delete one cron job |

| Flag | Description |
|------|-------------|
| `-w, --wave NAME` | Wave name passed to the scheduled command |
| `--flow NAME` | Flow or step to run |
| `--schedule daily` | Daily schedule (default) |

## lf op next

Preserve the current worktree and create a fresh branch.

```bash
lf op next
```

Commits and pushes current changes, optionally rebases, then creates and pushes a timestamped successor branch in the same worktree. If the current PR is already merged, it first resets to the default branch and syncs from origin before creating the next branch.

## lf op advance

Rotate a recurring wave onto a fresh branch.

```bash
lf op advance                # wave inferred from the worktree
lf op advance --wave shipper
```

Generates the wave's next schema-named branch (de-colliding with a word pair if taken), creates it in the worktree, and pushes it with upstream set. Unlike `lf op next`, it doesn't commit or rebase — it's the branch rotation a recurring wave (or its mind) runs after landing.

| Flag | Description |
|------|-------------|
| `-w, --wave NAME` | Wave name (default: inferred from the worktree) |

## lf op commit

Commit changes.

```bash
lf op commit                     # stage all changes, generate a message, commit
lf op commit -m "message"       # override the generated message
lf op commit -p                  # commit and push
lf op commit --no-add            # commit only what is already staged
```

Stages changes by default. If `-m` is omitted, Loopflow generates a commit message from the staged diff.

Options:

| Flag | Description |
|------|-------------|
| `-m, --message` | Commit message override |
| `-p, --push` | Push after committing |
| `--no-add` | Skip `git add -A`; commit only staged changes |

## lf op release

Mechanical release subcommands. Use `lf release` or `lf op release run` for the full release workflow.

```bash
lf op release run patch          # full release workflow
lf op release check              # PRs merged since last tag?
lf op release notes 1.2.3        # generate narrative RELEASE_NOTES.md from decisions + PRs
lf op release bump 1.2.3         # bump manifests
lf op release tag 1.2.3          # create + push git tag
lf op release status             # workflow + GitHub Release status
```

Keep release-cycle rationale in `release/unreleased/DECISIONS.md` when you want narrative-first notes. `lf op release notes` and the full release workflow promote it to `release/v<version>/`, use `DECISIONS.md` as the intent source, use merged PRs/diffs as the shipped-behavior source, and archive the generated root `RELEASE_NOTES.md` to `release/v<version>/NOTES.md`. If the ledger is absent, Loopflow falls back to merged PR history.

Headless release automation does not require a runner-local agent CLI. If the `release-notes` step cannot start Claude, Codex, or OpenCode, Loopflow writes deterministic notes from the same release context and keeps the archive contract intact.

---

## lf op pm

Read and edit a wave's roadmap. The roadmap lives in Linear — `lf op pm` talks to it directly, so there is no local mirror and nothing to sync.

```bash
lf op pm show --wave designer                              # print the wave's live Linear roadmap
lf op pm update --wave designer --title "Add dark mode"    # create a task
lf op pm update --wave designer --id 1207... --title "..." # update a task
lf op pm update --wave designer --id 1207... --status done # close a task
lf op pm init --wave designer                              # connect/create the Linear project, write linear_project into GOAL.md
lf op pm status                                            # show linked waves
```

| Command | What it does |
|---------|--------------|
| `show` | Print the wave's live roadmap from Linear |
| `update` | Create a task (no `--id`), or update/close one (`--id`; `--status done` closes it) |
| `init` | Connect or create the wave's Linear project and write `linear_project` into `wave/<name>/GOAL.md` |
| `status` | Show which waves are linked to a Linear project |

| Flag | Description |
|------|-------------|
| `--wave NAME` | Target wave (defaults to the current branch's wave) |
| `--id TASK-ID` | Existing Linear issue to update or close |
| `--title` | Task title (required when creating) |
| `--notes` | Task notes/description |
| `--status done` | Close the task |

Connect Linear first with `lf op auth linear`. `lf op pm init` pins the project into `GOAL.md`:

```yaml
# wave/designer/GOAL.md frontmatter
pm:
  linear_project: 8c4ba3f9-cf23-4136-87ed-37847aa7dc82
```

---

## lf op doctor

Check dependencies.

```bash
lf op doctor
```

Verifies that required tools are installed and working.

## lf op rebase

Plan or update the current branch against the right base.

```bash
lf op rebase        # update the branch
lf op rebase --plan # show the strategy without changing git
```

Classifies the branch before mutating git. Disposable branches can reset to
their base, stack children rebase onto their parent, and authored work uses a
normal rebase path. If `scratch/` needs to survive a reset, Loopflow stashes it
under `.lf/scratch-stash/` and restores it afterward.

Use an explicit target when needed:

```bash
lf op rebase origin/main
lf op rebase --plan parent.branch
```

## lf op sync

Update the local default branch to match origin.

```bash
lf op sync
```

Fetches `origin/<default-branch>` and updates your local default branch. Safe to run from any worktree.

If the default branch is checked out in another worktree, Loopflow resets that checked-out worktree to `origin/<default-branch>` instead of only moving the ref behind its back. Dirty changes in that worktree are auto-stashed and restored after the reset, including untracked files.

If restoring the stash conflicts, Loopflow keeps the stash so you can recover the work manually.

## lf op wt

Worktree helper commands.

### lf op wt create

Create or select a worktree from a placement plan.

```bash
lf op wt create my-feature              # sibling: root branch from main (the default)
lf op wt create thing --child parent    # child: create parent.thing
lf op wt create thing --child           # child of the current branch
lf op wt create my-feature --plan       # print the plan without creating anything
lf op wt create jack-heart.mobile.20260225_1122  # checks out origin branch in ../loopflow.mobile
```

Two relative-to-here verbs. **Sibling** (the default) roots an independent
branch from main. **Child** stacks under its parent. Ad-hoc worktrees never
nest unless you ask with `--child`.

| Flag | Description |
|------|-------------|
| `-c, --child [PARENT]` | Stack under `PARENT`, or under the current branch when omitted |
| `-s, --sibling` | Root an independent branch from the default branch (already the default) |
| `--plan` | Print the placement plan without mutating git |

Dots are reserved for stack ancestry. Use `api-v2` as a worktree segment, not
`api.v2`; create ancestry with `--stack`.

If the input matches an existing `origin/<branch>` name, `lf` checks out that branch instead of creating a new one. Root branch names follow the configured branch schema. Stacked children append the new segment to the parent branch with a dot.

### lf op wt switch

Switch to a worktree by wave name, chain leaf, or full branch.

```bash
lf op wt switch bugs             # the bugs wave worktree
lf op wt switch fix-auth         # the …bugs.fix-auth… worktree, by leaf
lf op wt switch jack/bugs.fix-auth.20260316_1856  # exact branch
```

### lf op wt up / down

Move through the stack — `up` toward main, `down` away from it.

```bash
lf op wt up              # to the parent worktree
lf op wt down            # to the only child (else lists them)
lf op wt down fix-auth   # to a specific child by leaf
```

### lf op wt list

Worktrees as a tree: children indent under their parent, workers show their
timestamp, main leads.

```bash
lf op wt list
lf op wt list --format json
```

```
* main                      active
  bugs                      active
    fix-auth                active
      retry                 active
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
lf op wt create my-feature       # create or select a placed worktree
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
