---
requires: wave/<chord>/
produces: scratch/garden-scan.md
---
Read the territory. Understand what each member wave has done, is doing, and is stuck on.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- If a `wave/<name>/` directory matches this work, skim its roadmap and items.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Scope

The chord-wave's area lists member wave directories. Each directory contains a
README (vision, strategy, goals, risks, metrics) and roadmap work items
(the roadmap). This step reads all of it, plus the living state around it.

Member wave names come from those directory names. If the chord-wave area
contains `wave/chord-model/` and `wave/signals/`, the wave names are
`chord-model` and `signals`.

## Workflow

1. **Read wave configs.** For each member wave directory in the area:
   - `GOAL.md` — intent, measures, process judgment, and the Linear handle
   - `MEMORY.md` — what the wave has learned and decided
   - The live roadmap — `lf op pm show --wave <wave-name>` (Linear is the source of truth; there are no local roadmap files)

2. **Read runtime state.** For each member wave:
   - `wave/<wave-name>/.wave-endpoint` — a live wave server publishes its
     endpoint here; absent means the wave is not running
   - `tmux ls` — the wave's server and any dispatched worker sessions
   - Open PRs on the wave's branches (`gh pr list`) and their queue state

3. **Read recent activity.** For each member wave:
   - `git log main --since="1 week ago"` filtered to the wave's area paths
   - Open PRs (`gh pr list`) from the wave's worktrees
   - CI status on open PRs
   - Any `scratch/` artifacts from in-progress work

4. **Read unlanded branches.** Look for work that was pushed but never landed:
   - `git branch -r` filtered to branches matching the wave name
   - For each branch ahead of main, show `git log main..<branch> --oneline`
     and `git diff --stat main..<branch>`
   - Check whether a PR exists for the branch (`gh pr list --head <branch>`)
   - Note branches with significant unlanded commits — these represent
     work the wave already did that the chord can't see from main alone
   - Check local worktrees too (`git worktree list`) — a worktree with
     uncommitted changes or unpushed commits is the same signal

5. **Read blocks.** Look for signals that a wave is stuck:
   - PRs with failing CI that haven't been fixed
   - Items with no recent commits
   - Merge conflicts
   - Open questions in `scratch/questions.md`

5. **Read cross-wave state.** Look for interactions between waves:
   - PRs that touch files in another wave's area
   - Items in different waves that reference the same code
   - Dependency ordering (does wave A's work block wave B?)

## Output

Write `scratch/garden-scan.md`:

```markdown
# Tend Scan — <date>

## Wave: <name>
### Config
<flow, mode, direction, area>

### Runtime
<registered/not registered, status, iteration, active run, PR/queue state>

### Progress
<what shipped recently, what's in flight>

### Items
| # | Title | Status |
|---|-------|--------|
| 01 | ... | shipped / in-flight / blocked / queued |

### Blocks
<anything preventing progress — CI failures, conflicts, stalls, missing decisions>

### Open PRs
<PR number, title, CI status, age>

### Unlanded Branches
<branch name, commits ahead of main, diff stats, PR status (none/open/closed)>

(repeat for each member wave)

## Cross-Wave
<interactions, dependencies, conflicts between waves>

## Raw Signals
<anything notable that doesn't fit above — patterns, surprises, anomalies>
```

## What to avoid

**Interpretation.** This step observes. It doesn't judge priorities, suggest changes, or
evaluate quality. That's assess's job.

**Staleness.** Run the commands. Don't rely on memory or cached state. The scan must
reflect the repo as it is right now.

**Partial reads.** Read every item file in every member wave. Skipping items means
the assessment will miss things.
