# Agents Architecture

Background agents that run pipelines automatically.

## Core Concepts

**Agent** - A named automation that runs a pipeline when triggered.

```markdown
# ~/.lf/agents/docs-bot.md
---
repo: /Users/jack/src/myproject
pipeline: [implement, polish, land]
trigger: main-changed
context: [src/, docs/]
---

Update documentation for any new public APIs.

Focus on:
- New exported functions
- Changed type signatures
- README updates for new features
```

The filename is the agent name. The body is the prompt. Config lives in YAML frontmatter.

**Triggers:**
- `manual` - Only runs when explicitly started
- `main-changed` - Runs when origin/main has new commits
- `interval` - Runs every N seconds

## File Layout

```
~/.lf/
├── agents/                    # Agent definitions (markdown, user-managed)
│   ├── docs-bot.md
│   └── feature-bot.md
├── agents.db                  # Runtime state only (ephemeral)
└── logs/
    └── agents/
        ├── docs-bot.log
        └── feature-bot.log
```

**Why markdown files for definitions?**
- Consistent with `.claude/commands/*.md` prompt format
- The prompt IS the file body - no separate field
- Config in frontmatter, just like Jekyll/Hugo
- Version controllable, easy to edit
- Can live in repo (`.lf/agents/`) or global (`~/.lf/agents/`)

**Why SQLite for runtime state?**
- Fast concurrent access
- Atomic updates
- Easy to query "what's running"

## Runtime State Schema

```sql
CREATE TABLE agent_runs (
    id TEXT PRIMARY KEY,
    agent_name TEXT NOT NULL,
    status TEXT NOT NULL,  -- running, completed, error
    started_at TEXT NOT NULL,
    ended_at TEXT,
    pid INTEGER,
    worktree TEXT,         -- which worktree it created/used
    iteration INTEGER DEFAULT 0,
    error TEXT
);

CREATE INDEX idx_agent_runs_name ON agent_runs(agent_name);
CREATE INDEX idx_agent_runs_status ON agent_runs(status);
```

## Components

### 1. Agent Daemon (`lf agent daemon`)

A lightweight process managed by launchd:

```xml
<!-- ~/Library/LaunchAgents/com.loopflow.agents.plist -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "...">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.loopflow.agents</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/lf</string>
        <string>agent</string>
        <string>daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>~/.lf/logs/daemon.log</string>
</dict>
</plist>
```

**What the daemon does:**
1. Loads all agent definitions from `~/.lf/agents/`
2. For each agent, checks if trigger condition is met
3. If triggered, spawns `lf agent run <name>` as subprocess
4. Sleeps, loops

**Daemon loop (pseudocode):**
```python
while True:
    for agent in load_agents():
        if should_trigger(agent):
            if not is_running(agent):
                spawn_agent_run(agent)
    sleep(30)  # check every 30s
```

### 2. Agent Runner (`lf agent run <name>`)

Runs a single agent iteration:

1. Create worktree for this run (or reuse existing)
2. Execute pipeline steps: `implement → polish → land`
3. Update runtime state in DB
4. Clean up worktree if configured

### 3. CLI Commands

```bash
# Daemon management
lf agent daemon              # Run daemon (foreground, for launchd)
lf agent daemon install      # Install launchd plist
lf agent daemon uninstall    # Remove launchd plist

# Agent management
lf agent list                # List all agents and status
lf agent new <name>          # Create new agent markdown file
lf agent edit <name>         # Open agent .md in $EDITOR
lf agent rm <name>           # Remove agent definition

# Manual control
lf agent start <name>        # Trigger agent run now
lf agent stop <name>         # Stop running agent (SIGTERM)
lf agent logs <name>         # Tail agent logs
```

### 4. Swift App Integration

**On first launch, the app:**
1. Checks if daemon is installed (`launchctl list | grep com.loopflow.agents`)
2. If not, installs the launchd plist and starts the daemon
3. Shows a brief "Setting up background agents..." indicator

**On quit:**
- Daemon keeps running (it's managed by launchd, not the app)
- User can disable via app preferences if they want

Maestro app shows agents in sidebar:

```
┌─────────────────────────────────────┐
│  WORKTREES                          │
│  ├─ main                    clean   │
│  └─ feature-x            3 modified │
│                                     │
│  AGENTS                             │
│  ├─ docs-bot              ● running │
│  │  └─ 12 iterations, last 5m ago   │
│  └─ feature-bot             ○ idle  │
│     └─ trigger: main-changed        │
└─────────────────────────────────────┘
```

**App capabilities:**
- View agent status (reads from `agents.db`)
- Start/stop agents (calls `lf agent start/stop`)
- View logs (reads from `~/.lf/logs/agents/`)
- Edit agent config (opens markdown in $EDITOR)

**App does NOT:**
- Run agents itself
- Need to be open for agents to work
- Manage daemon lifecycle (that's launchd's job)

## Module Rename

```
src/loopflow/
├── maestro/     →  agents/       # Rename: it's about agents
│   ├── agent.py     → models.py  # Agent, AgentRun dataclasses
│   ├── agents.py    → api.py     # Public API: list, start, stop
│   ├── daemon.py                 # Daemon loop
│   ├── runner.py                 # Single agent run logic
│   ├── triggers.py               # Trigger evaluation
│   └── db.py                     # Runtime state DB
├── cli/
│   └── agent.py                  # CLI commands
```

## App-Managed Daemon

The Swift app owns daemon lifecycle - no manual CLI setup needed.

**First launch flow:**
```swift
func ensureDaemonRunning() async {
    let plistPath = "~/Library/LaunchAgents/com.loopflow.agents.plist"

    if !FileManager.default.fileExists(atPath: plistPath) {
        // Write plist (bundled in app resources)
        writeDaemonPlist(to: plistPath)
    }

    // Check if running
    let result = shell("launchctl list com.loopflow.agents")
    if result.exitCode != 0 {
        // Start it
        shell("launchctl load \(plistPath)")
    }
}
```

**Plist points to bundled Python:**
```xml
<key>ProgramArguments</key>
<array>
    <string>/Applications/Loopflow Maestro.app/Contents/Resources/lf</string>
    <string>agent</string>
    <string>daemon</string>
</array>
```

Or if `lf` is installed via Homebrew/pipx, use that path instead.

**Preferences:**
- [ ] Run background agents (toggle to unload daemon)
- [ ] Start at login (toggle RunAtLoad in plist)

## Migration Path

1. Rename `loopflow/maestro/` → `loopflow/agents/`
2. Move agent definitions to markdown files (frontmatter + prompt body)
3. Simplify DB to runtime state only
4. Add daemon with launchd integration
5. Update Swift app to read new format

## Done When

1. `lf agent new foo` creates `~/.lf/agents/foo.md` with template
2. `lf agent list` shows all agents with status
3. `lf agent start foo` triggers a run, updates DB
4. `lf agent daemon` runs the trigger loop
5. `lf agent daemon install` sets up launchd plist
6. Maestro.app installs daemon on first launch
7. Maestro.app shows agents section in sidebar
8. Agents survive app quit and computer restart

## Open Questions

- **Per-repo vs global agents?** Could support both: `~/.lf/agents/` for global, `.lf/agents/` for repo-specific
- **Worktree strategy?** Each run creates fresh worktree vs reuses named worktree
- **Concurrency?** Can same agent have multiple runs? Probably not - skip if already running
- **Rate limiting?** Max runs per hour/day to prevent runaway costs
