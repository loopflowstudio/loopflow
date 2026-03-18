---
requires: wave/<chord>/
produces: scratch/tend-scan.md
---
Read the territory. Understand what each member wave has done, is doing, and is stuck on.

## Scope

The chord-wave's area lists member wave directories. Each directory contains a
README (vision, strategy, goals, risks, metrics) and numbered work items
(the roadmap). This step reads all of it, plus the living state around it.

Member wave names come from those directory names. If the chord-wave area
contains `wave/chord-model/` and `wave/signals/`, the wave names are
`chord-model` and `signals`.

## Workflow

1. **Read wave configs.** For each member wave directory in the area:
   - README.md — vision, strategy, goals, risks, metrics
   - All numbered item files — the roadmap
   - The wave YAML — flow, mode, direction, triggers

2. **Read runtime state.** For each member wave:
   - `lfq show <wave-name> --json` — live wave state from lfd
   - Capture wave status, iteration, open_pr_count, stack_count
   - If `active_run` exists, capture status, step_index, branch, PR state,
     draft state, queue_role, and queue_block_reason
   - If `lfq show` says the wave does not exist, note it explicitly as
     "defined on disk but not registered in lfd"

3. **Read recent activity.** For each member wave:
   - `git log main --since="1 week ago"` filtered to the wave's area paths
   - Open PRs (`gh pr list`) from the wave's worktrees
   - CI status on open PRs
   - Any `scratch/` artifacts from in-progress work

4. **Read blocks.** Look for signals that a wave is stuck:
   - PRs with failing CI that haven't been fixed
   - Items with no recent commits
   - Merge conflicts
   - Open questions in `scratch/questions.md`

5. **Read cross-wave state.** Look for interactions between waves:
   - PRs that touch files in another wave's area
   - Items in different waves that reference the same code
   - Dependency ordering (does wave A's work block wave B?)

## Output

Write `scratch/tend-scan.md`:

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
