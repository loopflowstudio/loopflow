---
layout: default
title: lf Operations
---

# lf Operations

Operations and utilities that do not launch a prompt.

---

## Git Workflow

## lf pr open

Create or update a PR, open in browser.

```bash
lf -m codex pr open
lf pr open -m codex
lf pr open --title "area: short title" --body "## Summary ..."
```

Use `-m/--model` for a one-off agent override when `lf pr open` needs a different harness than your configured default. When omitted, `lf pr open` uses `agent:` from `.lf/config.yaml` or `~/.lf/config.yaml`.

`--title` and `--body` are always required. Use `lf pr` to generate them with agent judgment.

Before opening or updating the PR, Loopflow syncs the default branch in the main repo so the PR is based on current upstream state even when you run `lf pr open` from a sibling worktree.

## lf pr land

Submit PR to merge queue.

```bash
lf pr land
```

Enables auto-merge on your PR. GitHub merges when CI passes and the merge queue clears. Run `lf wt prune` after merge completes to clean up.

## lf queue reconcile

Run one merge-queue reconcile pass over stacked wave runs.

```bash
lf queue reconcile               # every wave with queue state
lf queue reconcile --wave infra  # just one wave
```

Infers stack status from git and GitHub, flips PRs between draft and ready, lazily rebases stack heads, and records queue blocks as attention. Prints one line per wave — `reconciled` or `blocked (<reasons>)`. `lfd` execs the same verb when a PR-merged webhook arrives; running it by hand is always safe.

| Flag | Description |
|------|-------------|
| `-w, --wave NAME` | Only reconcile this wave (default: every wave with queue state) |

## lf cron

Install local launchd jobs that run `lf` commands on a schedule.

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
| `--flow NAME` | Flow or skill to run |
| `--schedule daily` | Daily schedule (default) |

## lf next

Preserve the current worktree and create a fresh branch.

```bash
lf next
```

Commits and pushes current changes, optionally rebases, then creates and pushes a timestamped successor branch in the same worktree. If the current PR is already merged, it first resets to the default branch and syncs from origin before creating the next branch.

## lf advance

Rotate a recurring wave onto a fresh branch.

```bash
lf advance                # wave inferred from the worktree
lf advance --wave shipper
```

Generates the wave's next schema-named branch (de-colliding with a word pair if taken), creates it in the worktree, and pushes it with upstream set. Unlike `lf next`, it doesn't commit or rebase — it's the branch rotation a recurring wave (or its flowloop) runs after landing.

| Flag | Description |
|------|-------------|
| `-w, --wave NAME` | Wave name (default: inferred from the worktree) |

## lf commit

Commit changes.

```bash
lf commit                     # stage all changes, generate a message, commit
lf commit -m "message"       # override the generated message
lf commit -p                  # commit and push
lf commit --no-add            # commit only what is already staged
```

Stages changes by default. If `-m` is omitted, Loopflow generates a commit message from the staged diff.

Options:

| Flag | Description |
|------|-------------|
| `-m, --message` | Commit message override |
| `-p, --push` | Push after committing |
| `--no-add` | Skip `git add -A`; commit only staged changes |

## lf release

Mechanical release subcommands. Use `lf release` or `lf release run` for the full release workflow.

```bash
lf release run patch          # full release workflow
lf release check              # PRs merged since last tag?
lf release notes 1.2.3        # generate narrative RELEASE_NOTES.md from decisions + PRs
lf release bump 1.2.3         # bump manifests
lf release tag 1.2.3          # create + push git tag
lf release status             # workflow + GitHub Release status
```

Keep release-cycle rationale in `release/unreleased/DECISIONS.md` when you want narrative-first notes. `lf release notes` and the full release workflow promote it to `release/v<version>/`, use `DECISIONS.md` as the intent source, use merged PRs/diffs as the shipped-behavior source, and archive the generated root `RELEASE_NOTES.md` to `release/v<version>/NOTES.md`. If the ledger is absent, Loopflow falls back to merged PR history.

Headless release automation does not require a runner-local agent CLI. If the `release-notes` skill cannot start Claude, Codex, or OpenCode, Loopflow writes deterministic notes from the same release context and keeps the archive contract intact.

---

## lf pm

Read and edit a wave's Linear tasks. Each wave is backed by one Linear project;
local projects live under `wave/<wave>/projects/` and tasks attach to them with
labels named `project:<slug>`.

```bash
lf pm status                                                     # show linked waves and task counts
lf pm sync --plan                                                # report Linear/local drift
lf pm show --wave designer                                       # group tasks by local project
lf pm show --wave designer --project ui                          # filter to one local project
lf pm task create --wave designer --project ui --title "Dark mode"
lf pm task update --id 1207... --title "Refine dark mode"
lf pm task done --id 1207... --pr "https://github.com/acme/app/pull/42"
lf pm task move --id 1207... --wave designer --project api
lf pm rename --wave designer --title "Designer"                  # rename the backing Linear project
lf pm init --wave designer                                       # connect/create the Linear project
```

| Command | What it does |
|---------|--------------|
| `status` | Show linked waves, backing Linear project names, and task counts by local project |
| `show` | Print Linear tasks grouped by local project; `--project` filters to one project |
| `task create` | Create a Linear task, optionally attached to a local project |
| `task update` | Edit an existing Linear task |
| `task done` | Close a Linear task and optionally comment with a PR link |
| `task move` | Move a task to another wave's Linear project and attach a local project label |
| `sync --plan` | Report backing Linear project drift, missing labels, stranded Linear projects, and unassigned tasks |
| `rename` | Rename the Linear project backing a wave |
| `init` | Connect or create the wave's Linear project and write `linear_project` into `wave/<name>/GOAL.md` |

| Flag | Description |
|------|-------------|
| `--wave NAME` | Target wave (defaults to the current branch's wave) |
| `--project SLUG` | Local project from `wave/<wave>/projects/<slug>.md` |
| `--id TASK-ID` | Existing Linear issue to update or close |
| `--title` | Task title (required when creating) |
| `--notes` | Task notes/description |
| `--pr URL` | PR link to comment on a closed or updated task |

`lf pm update` remains as a compatibility alias for create/update/done:

```bash
lf pm update --wave designer --project ui --title "Add dark mode"
lf pm update --id 1207... --status done --pr "https://github.com/acme/app/pull/42"
```

Connect Linear first with `lf auth linear`. `lf pm init` pins the project into `GOAL.md`:

```yaml
# wave/designer/GOAL.md frontmatter
pm:
  linear_project: 8c4ba3f9-cf23-4136-87ed-37847aa7dc82
```

---

## lf doctor

Check dependencies.

```bash
lf doctor
```

Verifies that required tools are installed and working.

## lf rebase

Plan or update the current branch against the right base.

```bash
lf rebase        # update the branch
lf rebase --plan # show the strategy without changing git
```

Classifies the branch before mutating git. Disposable branches can reset to
their base, stack children rebase onto their parent, and authored work uses a
normal rebase path. If `scratch/` needs to survive a reset, Loopflow stashes it
under `.lf/scratch-stash/` and restores it afterward.

Use an explicit target when needed:

```bash
lf rebase origin/main
lf rebase --plan parent.branch
```

## lf sync

Update the local default branch to match origin.

```bash
lf sync
```

Fetches `origin/<default-branch>` and updates your local default branch. Safe to run from any worktree.

If the default branch is checked out in another worktree, Loopflow resets that checked-out worktree to `origin/<default-branch>` instead of only moving the ref behind its back. Dirty changes in that worktree are auto-stashed and restored after the reset, including untracked files.

If restoring the stash conflicts, Loopflow keeps the stash so you can recover the work manually.

## lf wt

Worktree helper commands.

### lf wt create

Create or select a worktree from a placement plan.

```bash
lf wt create my-feature              # sibling: root branch from main (the default)
lf wt create thing --child parent    # child: create parent.thing
lf wt create thing --child           # child of the current branch
lf wt create my-feature --plan       # print the plan without creating anything
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
`api.v2`; create ancestry with `--child`.

Worktree branches use the fixed identity shape `<user>/<chain>`. A sibling
`bugs` creates `<user>/bugs` in `../loopflow.bugs`; a child `fix-auth` under
`<user>/bugs` creates `<user>/bugs.fix-auth` in
`../loopflow.bugs.fix-auth`.

### lf wt switch

Switch to a worktree by wave name, chain leaf, or full branch.

```bash
lf wt switch bugs             # the bugs wave worktree
lf wt switch fix-auth         # the …bugs.fix-auth… worktree, by leaf
lf wt switch jack/bugs.fix-auth.20260316_1856  # exact branch
```

### lf wt up / down

Move through the stack — `up` toward main, `down` away from it.

```bash
lf wt up              # to the parent worktree
lf wt down            # to the only child (else lists them)
lf wt down fix-auth   # to a specific child by leaf
```

### lf wt list

Worktrees as a tree: children indent under their parent, workers show their
timestamp, main leads.

```bash
lf wt list
lf wt list --format json
```

```
* main                      active
  bugs                      active
    fix-auth                active
      retry                 active
```

### lf wt ci

Show CI status for the current branch.

```bash
lf wt ci
```

### lf wt prune

Remove worktrees whose branches have been merged.

```bash
lf wt prune           # show what would be pruned
lf wt prune --dry-run # show what would be pruned
lf wt prune --force   # remove prunable worktrees
```

Finds worktrees where the branch is an ancestor of `origin/main` (handles squash merges). Never prunes main/master or worktrees with uncommitted changes (scratch/ files are excluded).

| Flag | Description |
|------|-------------|
| `--dry-run` | Show what would be pruned without removing |
| `--force` | Skip confirmation prompt |

## lf pr abandon

Abandon a branch: close PR, remove worktree, delete branch.

```bash
lf pr abandon feature-branch
lf pr abandon feature-branch --force   # skip confirmation
```

| Flag | Description |
|------|-------------|
| `-f, --force` | Skip confirmation and force abandon with uncommitted changes |

## lf shell

Shell integration setup.

```bash
lf shell init       # print shell integration code
lf shell install    # install to shell config file
```

Installs a wrapper that sources shell directives after `lf` commands (auto-cd into new worktrees).

## Typical Workflow

```bash
lf wt create my-feature       # create or select a placed worktree
lf wt switch my-feature       # switch to worktree from another
# ... work on feature ...
lf commit                     # commit with generated message
lf pr open                         # open PR (CI runs automatically)
# ... address review feedback ...
lf commit -p                  # commit and push
lf wt ci                      # check CI status
lf pr land                       # submit to merge queue
# ... wait for CI to pass and merge ...
lf wt prune                   # cleanup merged worktrees
```

## See Also

[`lf` reference](lf.md) · [Get Started](getting-started.md) · [Configuration](config.md)
