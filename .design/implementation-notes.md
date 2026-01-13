# Implementation Notes - Agents Architecture

## Status: Complete

All done-when criteria from the design doc are met.

## What Was Built

### Python Side
- `src/loopflow/maestro/markdown.py` - Parses agent markdown files with YAML frontmatter
- `src/loopflow/maestro/triggers.py` - Trigger evaluation (manual, main-changed, interval)
- `src/loopflow/maestro/launchd.py` - launchd plist management for daemon
- `src/loopflow/maestro/daemon.py` - Daemon loop that monitors triggers and spawns agent runs
- `src/loopflow/maestro/agent.py` - TriggerKind enum (manual, main-changed, interval)
- `src/loopflow/cli/agent.py` - Complete CLI with all commands
- `src/loopflow/cli/__init__.py` - Registered agent subcommand

### Swift Side
- `Maestro/Maestro/Models/Agent.swift` - Agent model with status, trigger, iteration tracking
- `Maestro/Maestro/Services/AgentService.swift` - Reads agent files and SQLite runtime state
- `Maestro/Maestro/Services/SetupService.swift` - Daemon lifecycle management
- `Maestro/Maestro/Views/WorktreeSidebar.swift` - Agents section in sidebar
- `Maestro/Maestro/AppState.swift` - Agent state and daemon setup on first launch

### CLI Commands
```bash
lf agent new <name>        # Creates ~/.lf/agents/<name>.md
lf agent list              # Lists agents from markdown files
lf agent edit <name>       # Opens in $EDITOR
lf agent rm <name>         # Deletes markdown file
lf agent show <name>       # Shows agent details
lf agent run <name>        # Runs single iteration (for daemon)
lf agent start <name>      # Triggers a run now
lf agent stop <name>       # Sends SIGTERM
lf agent logs <name>       # Tails log file
lf agent daemon            # Runs daemon loop
lf agent daemon install    # Installs launchd plist
lf agent daemon uninstall  # Removes launchd plist
lf agent daemon status     # Shows daemon status
```

## Done-When Verification

1. `lf agent new foo` creates `~/.lf/agents/foo.md` with template
2. `lf agent list` shows all agents with status
3. `lf agent start foo` triggers a run, updates DB
4. `lf agent daemon` runs the trigger loop
5. `lf agent daemon install` sets up launchd plist
6. Maestro.app installs daemon on first launch (via SetupService.ensureDaemonRunning)
7. Maestro.app shows agents section in sidebar (WorktreeSidebar agents section)
8. Agents survive app quit and computer restart (launchd KeepAlive)

## Test Results

- Python: 246 tests passed
- Swift: 11 tests passed

## Key Design Decisions

### Agent definitions as markdown
Agents are defined as markdown files with YAML frontmatter in `~/.lf/agents/`. This is consistent with `.claude/commands/*.md` format - the prompt IS the file body.

### SQLite for runtime state only
The `agent_runs` table tracks running/completed runs with PID, status, timestamps. Agent definitions live in markdown files, not the database.

### launchd for daemon management
The daemon is a standard macOS LaunchAgent. It starts at login, restarts if it crashes, and survives app quit. The Swift app installs it on first launch but doesn't manage it directly.

### Swift app reads but doesn't run
The Maestro app reads agent state from markdown files and SQLite, but delegates actual agent execution to `lf agent start/stop`. This keeps the app simple and ensures CLI and app behavior match.
