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

Read and edit a wave's Linear planning state. Each wave is backed by one Linear
Initiative, projects are Linear Projects, and tasks are Issues. `sync` refreshes
the local SQLite read model used by every other read surface.

```bash
lf pm status                                                     # show linked waves and task counts
lf pm sync --wave designer                                       # refresh SQLite from Linear
lf pm show --wave designer                                       # read; refresh when stale
lf pm show --wave designer --no-sync                             # cache-only agent/app read
lf pm show --wave designer --sync                                # force a refresh first
lf pm show --wave designer --project ui                          # filter to one project
lf pm project update --wave designer --project ui --definition "..." --kr "..."
lf pm project archive --wave designer --project retired-bet
lf pm task create --wave designer --project ui --title "Dark mode"
lf pm task update --id 1207... --title "Refine dark mode"
lf pm task done --id 1207... --pr "https://github.com/acme/app/pull/42"
lf pm task move --id 1207... --wave designer --project api
lf pm rename --wave designer --title "Designer"                  # rename the backing Linear Initiative
lf pm init --wave designer                                       # connect the Initiative
```

| Command | What it does |
|---------|--------------|
| `status` | Show linked waves, backing Linear Initiative names, and task counts by Project |
| `show` | Export the SQLite Project/task snapshot; refresh stale data when possible |
| `project create/update/archive` | Write or retire Linear Projects, then refresh SQLite |
| `task create` | Create a Linear task attached to a Project |
| `task update` | Edit an existing Linear task |
| `task done` | Close a Linear task and optionally comment with a PR link |
| `task move` | Move a task into a wave's Linear Project |
| `sync` | Fetch Linear Initiatives, Projects, KRs, and Issues into SQLite |
| `sync --plan` | Report Initiative/Project drift without writing SQLite |
| `rename` | Rename the Linear Initiative backing a wave |
| `init` | Create or connect the wave's Linear Initiative and write its stable id into `GOAL.md` |

| Flag | Description |
|------|-------------|
| `--wave NAME` | Target wave (defaults to the current branch's wave) |
| `--project SLUG` | Linear Project slug from the synced snapshot |
| `--sync` | Refresh from Linear before reading |
| `--no-sync` | Read SQLite only; never contact Linear |
| `--id TASK-ID` | Existing Linear issue to update or close |
| `--title` | Task title (required when creating) |
| `--notes` | Task notes/description |
| `--pr URL` | PR link to comment on a closed or updated task |

Connect Linear first with `lf auth linear`. `lf pm init` pins the Initiative into `GOAL.md`:

```yaml
# wave/designer/GOAL.md frontmatter
pm:
  provider: linear
  linear_initiative: 8c4ba3f9-cf23-4136-87ed-37847aa7dc82
```

When the id is absent, init derives the Initiative title from the wave name.
It links one exact match, creates one when none exists, and fails when the title
is ambiguous. Later commands use the persisted id, never the mutable title.

By default, `show` serves snapshots younger than one hour without a network
request. Older snapshots get one five-second refresh attempt; failures fall
back to cache until the snapshot is a week old. Use `--no-sync` in agents and
UI paths so rendering never waits on Linear.

---

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
