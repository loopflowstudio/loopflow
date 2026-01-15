---
layout: default
title: Daemon
---

# Daemon (lfd)

Background service for session tracking and agent orchestration.

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
- Live status updates in Maestro
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

## Agents

Agents are autonomous tasks that run on triggers. Define them as markdown files in `~/.lf/agents/`.

### Creating an Agent

```bash
lfd new my-agent
```

This creates `~/.lf/agents/my-agent.md`:

```markdown
---
repo: /path/to/repo
pipeline: ship
trigger:
  kind: manual
---

Optional prompt or instructions for the agent.
```

### Agent Definition

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

### Trigger Types

| Type | Description |
|------|-------------|
| `manual` | Only runs when explicitly started |
| `main-changed` | Runs when main branch updates |
| `interval` | Runs every N seconds |
| `loop` | Continuous: restarts immediately after completing |
| `cron` | Runs on cron schedule |

### Managing Agents

```bash
lfd list                  # List agents and status
lfd start my-agent        # Start an agent
lfd stop my-agent         # Stop a running agent
```

### Example: Continuous Review Agent

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

### Example: Daily Cleanup Agent

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

## Database

Session and agent state is stored in SQLite at `~/.lf/lfd.db` (WAL mode).

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

### Agent Runs Table

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT | UUID |
| agent_name | TEXT | Agent definition name |
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

Agent and daemon logs are written to `~/.lf/logs/`. Each agent run creates a timestamped log file.

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

### Agent Stuck

Agents that show "running" but their process is dead are automatically cleaned up by the daemon's periodic check. Force cleanup:

```bash
lfd stop my-agent
```
