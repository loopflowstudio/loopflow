---
layout: default
title: lf Operations
---

# lf Operations

Operations and utilities that do not launch a prompt.

---

## Git Workflow

## lf pr publish

Push and create or refresh a PR, then print its state and URL. Opens no browser
— this is the headless publication command agents use.

```bash
lf -m codex pr publish
lf pr publish -m codex
lf pr publish --title "area: short title" --body "## Summary ..."
```

Use `-m/--model` for a one-off agent override when copy generation needs a
different harness than your configured default. When omitted, `lf pr publish`
uses `agent:` from `.lf/config.yaml` or `~/.lf/config.yaml`. Use `lf pr` to
generate `--title`/`--body` with agent judgment.

Before publishing, Loopflow syncs the default branch in the main repo so the PR
is based on current upstream state even when you run it from a sibling worktree.
Push or GitHub failure returns an error and presents nothing.

## lf pr open

Publish the PR (same as `lf pr publish`), then open it for review — the GitHub
page in the browser. This is the explicit, human-initiated review action; agents
use `publish`, `submit`, or `land` instead.

```bash
lf pr open
lf pr open --title "area: short title" --body "## Summary ..."
```

If launching the review surface fails, only `lf pr open` fails — the PR is
already published and its URL is printed. An absent presentation preference uses
the GitHub browser default.

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
lf pm init --wave designer --team-key DSG                        # connect or rebind Initiative + team
lf pm reteam --wave designer --apply                             # move Projects, then open Issues without a writing body
```

| Command | What it does |
|---------|--------------|
| `status` | Show linked waves, backing Linear Initiative names, and task counts by Project |
| `show` | Export the SQLite Project/task snapshot; refresh stale data when possible |
| `project create/update/archive` | Write or retire Wave-prefixed Linear Projects, then refresh SQLite |
| `task create` | Create a Linear task attached to a Project |
| `task update` | Edit an existing Linear task |
| `task done` | Close a Linear task and optionally comment with a PR link |
| `task move` | Move a task into a wave's Linear Project |
| `sync` | Fetch Linear Initiatives, Projects, KRs, and Issues into SQLite |
| `sync --plan` | Report Initiative/Project drift without writing SQLite |
| `rename` | Rename the Linear Initiative backing a wave |
| `init` | Create or connect the Wave's Linear Initiative and team; an explicit team key rebinds an existing Wave |

| Flag | Description |
|------|-------------|
| `--wave NAME` | Target wave (defaults to the current branch's wave) |
| `--project SLUG` | Canonical Loopflow Project slug; the Wave prefix shown in Linear is excluded |
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
is ambiguous. It also binds a Wave-owned team whose key prefixes new Issues.
Passing an explicit `--team-key` replaces an existing team binding after
adopting or creating that exact team. Later commands use persisted ids, never
mutable titles.

Linear's Projects view is flat, so provider titles use `<Wave> — <Project>`.
Loopflow removes that display prefix while reading and keeps the canonical name
and slug. Creating `Loopflow API` in `product` therefore shows `Product —
Loopflow API` in Linear and remains `project:loopflow-api` everywhere in
Loopflow.

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
their base, while authored work uses a normal rebase path. If `scratch/` needs
to survive a reset, Loopflow stashes it under `.lf/scratch-stash/` and restores
it afterward.

Use an explicit target when needed:

```bash
lf rebase origin/main
lf rebase --plan parent.branch
```

Keep conflict resolution in the current process when the branch is too large
or sensitive to hand to another agent:

```bash
lf rebase --manual
# edit the conflict paths printed by lf
lf rebase --continue

lf rebase --abort # restore the pre-rebase branch instead
```

Manual recovery stays local and never pushes. Each `--continue` stages only
the current conflict paths; repeat edit/continue until the rebase completes.

## lf wt

Inspect, switch, and clean worktrees. Normal roadmap work starts with
`lf task run <issue-id>`; `lf wt` remains a low-level Git primitive.

Place dependent roadmap work through the Task API, not `lf wt`:

```bash
lf task run CHILD --stack-on PARENT
```

This creates CHILD's readable sibling worktree from PARENT's open PR branch.
`lf pr open`, `lf rebase`, and `lf pr land` carry the recorded placement forward.

### lf wt switch

Switch to a worktree by directory name, identity leaf, or full branch.

```bash
lf wt switch bugs             # the bugs task worktree
lf wt switch fix-auth         # the …bugs.fix-auth… worktree, by leaf
lf wt switch jack/bugs.fix-auth.20260316_1856  # exact branch
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

## Typical Task Workflow

```bash
lf task run ENG-123
lf task status ENG-123
lf task steer ENG-123 "also cover the migration"
lf task wait ENG-123 --until terminal
```

## See Also

[`lf` reference](lf.md) · [Get Started](getting-started.md) · [Configuration](config.md)
