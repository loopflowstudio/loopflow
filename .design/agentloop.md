# Agent Looping

Agent loops run a goal continuously against a repo, producing PRs to a personal-main branch.

## Core Concepts

**Goal** — A role-based persona defined in `.lf/goals/{name}.md`. The goal is the primary artifact. Goals can run against multiple repos.

**Loop** — A runtime instance: (goal, repo). Each loop has its own personal-main branch and iteration counter.

**Personal-main** — An agent's integration branch (e.g., `designer-main`). Iteration PRs merge here, not to real main. User reviews and lands when ready.

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
2. Runs pipeline, creates PR → personal-main
3. Auto-merges to personal-main
4. Continues to next iteration
5. Stops when outstanding commits >= limit (default 5)

Outstanding = commits on personal-main ahead of main.

## Goal File

Lives in repo: `.lf/goals/{name}.md`

```markdown
---
area: [src/api, src/models]
pipeline: @ship
---

# Product Engineer

## Ultimate Goal

Build features users love. Ship quality code that solves real problems.

## Each Iteration

1. Look at the codebase and find something to improve
2. Make the change
3. Ensure tests pass

## Quality Bar

- Code compiles and tests pass
- Changes are focused and reviewable
- No obvious bugs or regressions
```

**Frontmatter:**
- `area` — Default pathset (can be overridden at CLI)
- `pipeline` — Default pipeline to run

## Database Schema

```sql
-- Loop/subscription configuration
loops (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,          -- loop, flow, subscribe, schedule
    goal TEXT NOT NULL,
    repo TEXT NOT NULL,
    personal_main TEXT NOT NULL,
    status TEXT NOT NULL,        -- idle, running, waiting, error
    iteration INTEGER DEFAULT 0,
    pr_limit INTEGER DEFAULT 5,
    merge_mode TEXT DEFAULT 'auto',

    -- Type-specific config
    project_file TEXT,           -- for flow
    pathset TEXT,                -- for subscribe (comma-separated)
    cron TEXT,                   -- for schedule
    area TEXT,                   -- area of responsibility override

    created_at TEXT,
    UNIQUE(type, goal, repo)
)

-- Each iteration attempt
loop_runs (
    id TEXT PRIMARY KEY,
    loop_id TEXT NOT NULL,
    iteration INTEGER NOT NULL,
    status TEXT NOT NULL,        -- running, completed, error
    started_at TEXT,
    ended_at TEXT,
    worktree TEXT,
    current_step TEXT,
    error TEXT,
    pr_url TEXT,
    FOREIGN KEY (loop_id) REFERENCES loops(id)
)
```

**Loop types:**
- `loop` — Continuous homeostasis
- `flow` — One-off project execution
- `subscribe` — Triggered by pathset changes on main
- `schedule` — Triggered by cron

**Loop status:**
- `idle` — Not running
- `running` — Currently executing an iteration
- `waiting` — Paused (outstanding >= limit)
- `error` — Last iteration failed

**Merge modes:**
- `auto` — PR auto-merges to personal-main (default)
- `pr` — PR to personal-main waits for approval
- `land` — Auto-merge to personal-main AND auto-merge personal-main → main

## Commands

Run from the repo directory (like `lf`).

### lfd loop

Continuous homeostasis. Runs iterations until PR limit is hit.

```bash
lfd loop <goal>
lfd loop designer
lfd loop designer -r src/auth,src/models   # area of responsibility override
```

### lfd flow

One-off execution. Runs until project is done.

```bash
lfd flow <goal> --project <file>
lfd flow designer --project .design/feature.md
lfd flow designer -r src/api --project .design/feature.md
```

### lfd subscribe

Triggered execution. Runs once when pathset changes on main.

```bash
lfd subscribe <pathset> <goal>
lfd subscribe src/api designer
lfd subscribe src/auth security
lfd subscribe "src/api,src/models" designer   # multiple paths
```

### lfd schedule

Scheduled execution. Runs on cron schedule.

```bash
lfd schedule "<cron>" <goal>
lfd schedule "0 9 * * *" reporter --project .design/daily-report.md
lfd schedule "0 */4 * * *" designer   # every 4 hours
```

### lfd status / stop / prs

Manage running loops and subscriptions:

```bash
lfd status                     # all loops (across all repos)
# ID       TYPE        GOAL       REPO              STATUS    ITER
# abc123   loop        designer   ~/myapp           running   3
# def456   subscribe   security   ~/myapp           idle      -
# ghi789   schedule    reporter   ~/myapp           idle      -

lfd stop <loop-id>             # stop/unregister by ID
lfd stop abc123

lfd prs <loop-id>              # PRs for a loop
lfd prs abc123
```

### lfops rebase

Keep personal-main rebased on main:

```bash
lfops rebase <loop-id>
```

Run periodically or trigger on main changes.

### lfops land

Squash-merge personal-main into main:

```bash
lfops land <loop-id>
```

Does:
1. Creates PR: personal-main → main (if not exists)
2. Squash merges (all iterations become one commit)
3. Closes individual iteration PRs
4. Resets personal-main to main

## Two Modes

### Homeostasis Loops

Continuous improvement with no end state. Agent keeps iterating until PR limit is hit.

```bash
cd ~/myapp
lfd agent designer
```

Use for: code quality, refactoring, documentation, dependency updates, security hardening.

### Project Flows

One-off execution with a concrete goal. Agent runs iterations until the project is complete, then stops.

```bash
cd ~/myapp
lfd flow designer --project .design/auth-feature.md
```

The goal file provides the persona/approach. The project file provides the specific task.

**Project file example** (`.design/auth-feature.md`):
```markdown
# Auth Feature

Add OAuth2 login with Google and GitHub providers.

## Requirements
- Login/logout buttons on header
- Store tokens in secure cookie
- Redirect to original page after auth

## Done when
- All tests pass
- Manual QA confirms flow works
```

The agent reads "Done when" to know when to stop. Each iteration checks if criteria are met.

### Comparison

| Aspect | `lfd loop` | `lfd flow` |
|--------|------------|------------|
| Ends when | PR limit hit | "Done when" satisfied |
| Purpose | Continuous improvement | Specific deliverable |
| Iterations | Unlimited (until limit) | As many as needed |
| Prompt | Goal only | Goal + Project |

## Workflow

**Continuous improvement:**
```bash
cd ~/myapp
lfd loop designer
# Agent runs iterations, PRs accumulate on personal-main
# User reviews at their pace
lfops land <loop-id>             # Ship when satisfied
```

**Concrete project:**
```bash
cd ~/myapp
lfd flow designer --project .design/auth-feature.md
# Agent works until done, then stops
lfops land <loop-id>
```

**Same goal, multiple repos:**
```bash
cd ~/app1 && lfd loop designer
cd ~/app2 && lfd loop designer -r src/frontend
```

**Keep in sync:**
```bash
lfops rebase <loop-id>           # After main moves forward
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                              STORAGE                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ~/.lf/lfd.db                        ~/myapp/.lf/goals/             │
│  ┌─────────────────────┐             ┌─────────────────────┐        │
│  │ loops               │             │ designer.md         │        │
│  │ ├─ id               │             │ product-engineer.md │        │
│  │ ├─ goal ────────────┼─────────────│ infra.md            │        │
│  │ ├─ repo             │             └─────────────────────┘        │
│  │ ├─ personal_main    │                                            │
│  │ ├─ status           │             Goal files live in repo,       │
│  │ ├─ iteration        │             portable across machines.      │
│  │ └─ pr_limit         │                                            │
│  ├─────────────────────┤                                            │
│  │ loop_runs           │                                            │
│  │ ├─ loop_id          │                                            │
│  │ ├─ iteration        │                                            │
│  │ ├─ status           │                                            │
│  │ ├─ pr_url           │                                            │
│  │ └─ worktree         │                                            │
│  └─────────────────────┘                                            │
│                                                                     │
│  DB is machine-local.                                               │
│  Tracks runtime state only.                                         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                            EXECUTION                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  cd ~/myapp && lfd loop designer                                    │
│       │                                                             │
│       ▼                                                             │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐           │
│  │ Load goal   │────▶│ Get/create  │────▶│ Spawn       │           │
│  │ from repo   │     │ loop in DB  │     │ subprocess  │           │
│  └─────────────┘     └─────────────┘     └──────┬──────┘           │
│                                                  │                  │
│                                                  ▼                  │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                     LOOP SUBPROCESS                          │  │
│  │                                                              │  │
│  │  while outstanding < limit:                                  │  │
│  │      1. Create worktree from personal-main                   │  │
│  │      2. Run pipeline (design → implement → polish)           │  │
│  │      3. Create PR → personal-main                            │  │
│  │      4. Auto-merge PR                                        │  │
│  │      5. Record loop_run in DB                                │  │
│  │      6. iteration++                                          │  │
│  │                                                              │  │
│  │  status = WAITING                                            │  │
│  │                                                              │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                           GIT BRANCHES                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  origin/main                                                        │
│       │                                                             │
│       │◄─── lfops land (squash merge)                               │
│       │                                                             │
│  designer-main  (personal-main)                                     │
│       │                                                             │
│       │◄─── lfops rebase (keep current with main)                   │
│       │                                                             │
│       ├── 🎨/designer/001  ─┐                                       │
│       ├── 🎨/designer/002   ├── iteration PRs auto-merge here       │
│       └── 🎨/designer/003  ─┘                                       │
│                                                                     │
│  Worktrees created as siblings:                                     │
│    ~/myapp.🎨-designer-001/                                         │
│    ~/myapp.🎨-designer-002/                                         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Migration

Current implementation uses separate agent files (`~/.lf/agents/*.md`) that reference goals. New model:

- Delete `~/.lf/agents/` concept
- Goal file is the source of truth
- Loop config (personal-main, iteration, status) lives in DB only
- CLI specifies repo and optional overrides

## What's Left

- [ ] New DB schema (loops, loop_runs tables)
- [ ] `lfd loop` command (homeostasis)
- [ ] `lfd flow` command (project)
- [ ] `lfd subscribe` command (pathset trigger)
- [ ] `lfd schedule` command (cron trigger)
- [ ] `lfd status/stop/prs` commands
- [ ] Daemon: watch main for subscribe triggers
- [ ] Daemon: cron scheduler for schedule triggers
- [ ] `lfops rebase` command
- [ ] `lfops land` command with PR cleanup
- [ ] Outstanding commit counting logic
- [ ] WAITING state and resume on land
- [ ] Project "Done when" checking logic
- [ ] Remove old agents.py / agent file parsing
