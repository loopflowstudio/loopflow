# Agent Loops: Overview

Agent loops run a Goal continuously against a repo, producing PRs to a personal-main branch.

## Core Concepts

| Concept | Purpose | Location |
|---------|---------|----------|
| **Goal** | What to achieve, how to select work | `.lf/goals/` |
| **Flow** | Sequence of tasks | TBD |
| **Area** | Where to work (pathset) | CLI or goal frontmatter |
| **Voice** | Style/persona (optional) | `.lf/voices/` |

**Agent = Goal × Flow × Area [× Voice]**

## Four Loop Types

| Type | Trigger | Ends When | Use For |
|------|---------|-----------|---------|
| `lfd loop` | Manual start | PR limit hit | Continuous improvement |
| `lfd flow` | Manual start | "Done when" satisfied | One-off projects |
| `lfd subscribe` | File changes on main | One iteration | Reactive maintenance |
| `lfd schedule` | Cron | One iteration | Periodic tasks |

## Branch Model

```
main ────────────────────────────────►
  ↑                                  │
  │ lfops land (squash)              │ lfops rebase (keeps current)
  │                                  ↓
personal-main ◄──────────────────────
  ↑
  │ PR (auto-merge per iteration)
  │
iteration branches (🎨/designer/001, 002...)
```

1. Loop creates iteration branch from personal-main
2. Runs flow, creates PR → personal-main
3. Auto-merges to personal-main
4. Continues to next iteration
5. Stops when outstanding commits >= limit (default 5)

**Outstanding** = commits on personal-main ahead of main.

## Goal File

Lives in repo: `.lf/goals/{name}.md`

```markdown
---
area: [src/api, src/models]
flow: @ship
---

# Improve Test Coverage

Target 80% coverage on src/. Currently at 45%.

## How to Select Work

Each iteration, pick the module with lowest coverage that's
also high-churn (changed recently). Add tests for that module.

## Done When

Coverage >= 80% on src/
```

**Frontmatter:**
- `area` — Default pathset (can be overridden at CLI)
- `flow` — Default flow to run

## Commands

```bash
# Continuous improvement
lfd loop <goal>
lfd loop test-coverage

# One-off project
lfd flow <goal> --project <file>
lfd flow api-cleanup --project .design/auth-feature.md

# Reactive (triggered by file changes)
lfd subscribe <pathset> <goal>
lfd subscribe src/api api-cleanup

# Scheduled
lfd schedule "<cron>" <goal>
lfd schedule "0 9 * * *" daily-report

# Management
lfd status                    # All loops
lfd stop <loop-id>            # Stop a loop
lfd prs <loop-id>             # PRs for a loop

# Landing
lfops rebase <loop-id>        # Keep personal-main current
lfops land <loop-id>          # Squash-merge to main
```

## Design Docs

Detailed designs for each component:

1. [Database Schema](./loops-db-schema.md) — loops and loop_runs tables
2. [lfd loop Command](./loops-lfd-loop.md) — Core homeostasis loop
3. [lfd status/stop/prs](./loops-lfd-status.md) — Observability commands
4. [PR Limit Logic](./loops-pr-limit.md) — Outstanding commit counting
5. [lfops land](./loops-lfops-land.md) — Squash-merge to main

## What's Changing

- Delete `~/.lf/agents/` concept
- Goal file is the source of truth for work selection
- Loop config (personal-main, iteration, status) lives in DB only
- `lf` stays lightweight; `lfd` handles daemon/loop concerns
