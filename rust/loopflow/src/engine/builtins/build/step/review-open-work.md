---
interactive: true
requires: none
produces: scratch/open-work.md, possibly dispatched ship runs and branch prunes
---
Clear outstanding branches, PRs, worktrees, and waves until the repo has an obvious next move.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- If a `wave/<name>/` directory matches this work, skim its roadmap and items.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Goal

Get to inbox zero on your own in-flight work.

First clear the decks: ship work that is close, abandon cruft, and prune stale remote branches. Then audit waves from the cleanest current picture and decide what deserves continued attention.

## Workflow

### 1. Scan

Run the scan headlessly before discussion. Gather:

- Local worktrees: `lf op wt list --format json`
- Open PRs authored by the user: `gh pr list --author @me --state open --json number,title,headRefName,url,isDraft,mergeStateStatus,statusCheckRollup,updatedAt`
- Remote branches authored by the user: `lf op branches list --user @me`
- Stale candidates: `lf op branches list --user @me --stale 60d`
- Wave entries under `wave/`: each README, roadmap item counts by priority bucket, recent commits touching the wave area, associated open PRs
- Merge status for each branch: ahead/behind main, CI status when a PR exists, whether a branch is merged or squash-merged
- Wave attribution for each worktree and branch:
  - Worktrees: use the engine's worktree resolver output and sibling worktree convention. Do not invent another naming scheme.
  - Branch-only items: use `lf op branches list`, which resolves branch names through the configured branch schema. `-` means waveless.

Treat missing `gh` data as unknown, not green.

### 2. Write initial triage

Write `scratch/open-work.md` before asking anything:

```markdown
# Open Work — <date>

## Pass 1: Clear the decks

| Item | Kind | Age | Status | Recommendation | Why |
|---|---|---|---|---|---|
| <branch> | worktree+PR | 21d | CI green, 1 nit | **ship** | Reviewed, only polish left |
| <branch> | branch only | 35d | diverged 40 commits | **abandon** | Superseded by wave/foo item |
| <branch> | remote only | 90d | no PR | **prune** | No worktree, no PR |

## Pass 2: Wave audit

| Wave | Vision progress | Recent activity | Recommendation |
|---|---|---|---|
| redesign | 2 of 4 pillars shipped, third in flight | 3 PRs in 2wk, all roadmap-linked | **continue** |
| chatgui | Core UX still not usable | 14 commits, mostly refactors | **busy without progress — reduce scope or abandon** |
```

Recommendations:

- Waveless branch: **ship** or **abandon** only.
- Waved branch: **ship**, **ship-partial**, or **abandon**.
- Remote-only stale branch: **prune** when no open PR exists.
- Ambiguous branch: **discuss** with a concrete question.

Judge waves by progress toward README Goals/Vision, not activity counts. Commit counts and PR counts are signals; delivered value is the answer. Call out **busy without progress** and **lack of action** explicitly.

### 3. Discuss Pass 1 row by row

Walk each row with the user.

- **ship**: dispatch `lf ship` in that worktree as a background job. Do not wait. Log to `scratch/ship-logs/<branch>.log` in the main repo.
- **ship-partial**: dispatch `lf ship` the same way; the ship flow defaults toward landing and deferring leftovers into the wave roadmap.
- **abandon**: after confirmation, use the existing abandon path for worktrees or delete local/remote branches directly when no worktree exists.
- **prune**: batch remote-only branch deletes for one `lf op branches prune ...` command at the end.

Dispatch mechanics:

```bash
mkdir -p scratch/ship-logs
(cd <worktree> && nohup lf ship > <main>/scratch/ship-logs/<branch>.log 2>&1 &)
```

Fire and forget. Multiple ships can run in parallel across sibling worktrees. Move to the next row immediately.

### 4. Discuss Pass 2 wave by wave

After Pass 1 actions are dispatched or the user says to move on, audit each wave:

- **continue** — goals still matter and recent work advances them
- **split** — too broad for one wave
- **reduce scope** — goal still matters, but current work is churn
- **archive** — goal reached, obsolete, or not worth carrying

Do not mutate wave files automatically. Record decisions inline in `scratch/open-work.md`.

### 5. End with a checkpoint

Update `scratch/open-work.md`:

- Strike through completed rows
- Leave remaining rows as a punch list
- Add dispatched ship commands and log paths
- Add branch prune command(s) run or queued
- Add final wave decisions

## Archival conventions

Infer archive conventions from the repo instead of hardcoding them. Look for `wave/old/`, archive wording in READMEs, empty roadmaps, deprecated frontmatter, or long periods with no commits touching the area. Mark inferred archives as low-priority and say why. If ambiguous, ask during discussion.

## Guardrails

- Only scan the user's own branches and PRs.
- Do not evaluate teammate PRs.
- Do not create new waves for waveless branches.
- Do not wait on dispatched `lf ship` jobs.
- Do not tail or summarize ship logs at the end; report where they are.
- Do not prune branches with open PRs unless the user explicitly overrides.
