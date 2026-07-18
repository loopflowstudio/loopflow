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
- Read wave/PM context only when the seed names the exact wave, task, project,
  or a concrete coordination question; never infer it or repair access as a
  prerequisite.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Scope

The chord-wave's area lists member wave directories. Each directory contains a
`GOAL.md` and `MEMORY.md`; Project definitions, KRs, and tasks come from its PM
snapshot. This step reads all of it, plus the living state around it.

Member wave names come from those directory names. If the chord-wave area
contains `wave/chord-model/` and `wave/signals/`, the wave names are
`chord-model` and `signals`.

## Workflow

1. **Read wave configs.** For each member wave directory in the area:
   - `GOAL.md` — objective, measures, cadence, policy, and the Linear handle
   - `MEMORY.md` — what the wave has learned and decided
   - `lf pm show --wave <wave> --json` — measured bets, KRs, and tasks from SQLite

   Linear is the source of truth; there are no local Project or Task lists.

2. **Read runtime state.** For each member wave:
   - `lf status <wave-name> --json` — Wave presence, resident state, Project
     Sessions, Tasks, next owners, worktrees, PRs, and attention
   - `lf task status <issue-id> --json` or
     `lf project status <project-id> --json` only when the Wave snapshot needs
     deeper inspection

   Do not infer product state from tmux names or branch naming.

3. **Read recent activity.** For each member wave:
   - `git log main --since="1 week ago"` filtered to the wave's area paths
   - Open PRs owned by the Wave's Tasks
   - CI status on open PRs
   - Any `scratch/` artifacts from in-progress work

4. **Read unlanded branches.** Look for work that was pushed but never landed:
   - Start from each Task's persisted branch and worktree
   - For each Task branch ahead of main, show `git log main..<branch> --oneline`
     and `git diff --stat main..<branch>`
   - Check whether a PR exists for the branch (`gh pr list --head <branch>`)
   - Note branches with significant unlanded commits — these represent
     work the wave already did that the chord can't see from main alone
   - Check Task worktrees too (`lf wt list`) — a worktree with
     uncommitted changes or unpushed commits is the same signal. Low-level
     worktrees not attached to a Task are diagnostic state, not roadmap work.

5. **Read blocks.** Look for signals that a wave is stuck:
   - PRs with failing CI that haven't been fixed
   - Tasks or Sessions with no recent activity
   - Merge conflicts
   - Open questions in `scratch/questions.md`

6. **Read cross-wave state.** Look for interactions between waves:
   - PRs that touch files in another wave's area
   - Tasks in different waves that reference the same code
   - Dependency ordering (does wave A's work block wave B?)

## Task references

When the scan names more than one Task, read `lf roadmap --wave <wave> --json`
for plan-wide rows and `lf status <wave> --json` for live execution. Render
every Task with the shared reference:

```markdown
[identifier](provider URL) — readable active PR/workspace slug — status/next owner
```

Fill the link from `task.identifier` and `reference.issue_url`. Use
`active_pr.slug` from roadmap; for status, match `active_pr` to `prs[].id` and
use that PR's `slug`. Fall back to `reference.workspace.slug`. Take status from
`runtime.status`, or from the roadmap `section` when runtime is absent;
`next_move.owner` supplies next owner. Never reconstruct a provider URL, branch,
or slug from an identifier, title, worktree, or naming convention. Omit only a
link or slug whose snapshot evidence is explicitly absent; keep the Task and
its available status/next owner.

## Output

Write `scratch/garden-scan.md`:

```markdown
# Tend Scan — <date>

## Wave: <name>
### Config
<objective, cadence, policy, PM binding>

### Runtime
<Wave presence and resident state, active Project/Tasks, attention>

### Progress
<what shipped recently, what's in flight>

### Projects
<Project KRs, Project state, next owner>

### Tasks
<[identifier](provider URL) — readable active PR/workspace slug — status/next owner,
followed by any relevant title or evidence>

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

**Partial reads.** Read every Project and Task in each Wave's PM/status snapshot.
Skipping filed or running work means the assessment will miss things.
