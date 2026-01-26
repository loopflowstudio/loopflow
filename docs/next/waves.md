---
layout: default
title: Waves
---

# Waves

*Coming soon.*

The `lfd` daemon tracks sessions and orchestrates waves. Define waves as markdown files—what they do, when they trigger, how they merge. They run in the background, each in its own worktree. You review the PRs.

## Installation

```bash
lfd install
```

This installs a launchd service that starts automatically at login. The daemon runs at `~/.lf/lfd.sock`.

Check status:

```bash
lfd status
```

## Session Tracking

When you run tasks in auto mode, sessions are automatically logged to `~/.lf/lfd.db`. This enables:

- Session history queries per worktree
- Live status updates in Concerto
- `lfops status` to show running sessions

Sessions are logged via fire-and-forget calls—if the daemon isn't running, tasks still execute normally; they just aren't tracked.

### Querying Sessions

Via CLI:

```bash
lfops status              # Show active sessions
```

Via socket (see [API Reference](api.md)):

```json
{"method": "sessions.list", "params": {}}
{"method": "sessions.history", "params": {"worktree": "/path/to/worktree"}}
```

## Waves

Waves are autonomous tasks that run on triggers. Define them as markdown files in `~/.lf/waves/`.

### Creating a Wave

```bash
lfd new my-wave
```

This creates `~/.lf/waves/my-wave.md`:

```markdown
---
repo: /path/to/repo
pipeline: ship
trigger:
  kind: manual
---

Optional prompt or instructions for the wave.
```

### Wave Definition

| Field | Description |
|-------|-------------|
| `repo` | Repository path (required) |
| `pipeline` | Pipeline name to run (required) |
| `trigger.kind` | Trigger type (see below) |
| `trigger.interval_seconds` | For interval triggers |
| `trigger.cron` | For cron triggers |
| `trigger.grace_minutes` | Cooldown between runs (default: 60) |
| `context` | Additional context files |
| `emoji` | Display emoji for UI |
| `merge_mode` | How to merge work: `auto`, `pr`, `silent` |
| `personal_main` | Branch for wave's work (auto-allocated if not set) |

### Trigger Types

| Type | Description |
|------|-------------|
| `manual` | Only runs when explicitly started |
| `main-changed` | Runs when main branch updates |
| `interval` | Runs every N seconds |
| `loop` | Continuous: restarts immediately after completing |
| `cron` | Runs on cron schedule |

### Managing Waves

```bash
lfd list                  # List waves and status
lfd start my-wave         # Start a wave
lfd stop my-wave          # Stop a running wave
```

### Example: Continuous Review Wave

```markdown
---
repo: /Users/me/myproject
pipeline: review
trigger:
  kind: loop
emoji: 🔍
---

Continuously review incoming changes and fix issues.
```

### Example: Daily Cleanup Wave

```markdown
---
repo: /Users/me/myproject
pipeline: polish
trigger:
  kind: cron
  cron: "0 9 * * *"
emoji: 🧹
---

Run polish pipeline every morning at 9am.
```

### Personal Main Branches

Each wave gets its own "personal main" branch to accumulate work without polluting the real main branch. This is auto-allocated when the wave first runs.

```markdown
---
repo: /Users/me/myproject
pipeline: ship
trigger:
  kind: loop
merge_mode: auto
personal_main: review-wave-main
---
```

Merge modes:
- `auto` — Create PR to personal-main, auto-merge
- `pr` — Create PR, wait for human approval
- `silent` — Direct merge to personal-main (no PR)

To land wave work to real main:

```bash
lfops land --squash     # Squash personal-main to main
```

## Database

Session and wave state is stored in SQLite at `~/.lf/lfd.db` (WAL mode).

### Sessions Table

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT | UUID |
| task | TEXT | Task name |
| repo | TEXT | Repository path |
| worktree | TEXT | Worktree path |
| status | TEXT | running, waiting, completed, error |
| started_at | TEXT | ISO8601 timestamp |
| ended_at | TEXT | ISO8601 or NULL |
| pid | INTEGER | Process ID |
| model | TEXT | Model name |
| run_mode | TEXT | auto or interactive |

### Wave Runs Table

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT | UUID |
| wave_name | TEXT | Wave definition name |
| status | TEXT | idle, running, waiting, error, stopped |
| started_at | TEXT | ISO8601 timestamp |
| ended_at | TEXT | ISO8601 or NULL |
| pid | INTEGER | Process ID |
| worktree | TEXT | Current worktree path |
| iteration | INTEGER | Run count |
| error | TEXT | Error message or NULL |
| main_sha | TEXT | Main branch SHA at start |

## Socket Protocol

The daemon listens on a Unix socket at `~/.lf/lfd.sock`. See [API Reference](api.md) for the full protocol.

## Logs

Wave and daemon logs are written to `~/.lf/logs/`. Each wave run creates a timestamped log file.

## Troubleshooting

### Daemon Not Running

```bash
lfd install               # Reinstall launchd service
launchctl list | grep lfd # Check if loaded
```

### Stale Socket

If the socket exists but the daemon isn't responding:

```bash
rm ~/.lf/lfd.sock
lfd install
```

### Wave Stuck

Waves that show "running" but their process is dead are automatically cleaned up by the daemon's periodic check. Force cleanup:

```bash
lfd stop my-wave
```
