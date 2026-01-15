# Deferred Work: Loop Trigger, Cron Backend, Context Paths Add Button

Spec for deferred items 1, 2, and 5 from agentsvoices.

## What to build

1. **Loop trigger execution** — daemon re-triggers agents with `trigger: loop` immediately after completion
2. **Cron trigger backend** — daemon evaluates cron expressions and triggers agents on schedule
3. **Context paths add button** — UI button to add context paths to agents in AgentDetailPanel

---

## 1. Loop Trigger Execution

### Current state

`triggers.py:26-28` returns `True` for loop triggers, but the daemon (`daemon.py:193`) only checks triggers for agents that are *not running*. After an agent completes, the daemon marks it completed but doesn't immediately re-trigger.

### What's needed

After marking an agent run completed, check if `trigger == "loop"` and spawn a new run immediately.

```python
# daemon.py

def _update_completed_runs() -> None:
    """Check all running agents and mark completed ones."""
    conn = get_db(DEFAULT_DB_PATH)
    cursor = conn.execute(
        "SELECT id, agent_name, pid FROM agent_runs WHERE status = 'running'"
    )
    rows = cursor.fetchall()

    loop_agents = []  # Agents to re-trigger

    for row in rows:
        pid = row["pid"]
        if not pid:
            continue

        try:
            os.kill(pid, 0)
        except OSError:
            # Process exited - mark as completed
            conn.execute(
                """UPDATE agent_runs
                   SET status = 'completed', ended_at = ?
                   WHERE id = ?""",
                (datetime.now().isoformat(), row["id"]),
            )
            _log(f"Agent {row['agent_name']} completed (was PID {pid})")
            loop_agents.append(row["agent_name"])  # Track for re-trigger

    conn.commit()
    conn.close()

    return loop_agents  # Return list of completed agents for loop check


def run_daemon(check_interval: int = 30) -> None:
    """Run the daemon loop."""
    # ... existing setup ...

    while True:
        try:
            completed_agents = _update_completed_runs()
            agents = list_agent_files()

            for agent in agents:
                # Loop trigger: check if just completed
                if agent.trigger == "loop" and agent.name in completed_agents:
                    if not _is_agent_running(agent.name):  # Double-check
                        _log(f"Loop re-triggering agent: {agent.name}")
                        pid = _spawn_agent_run(agent)
                        if pid:
                            worktree = get_agent_worktree_path(agent)
                            _record_run_start(agent, pid, worktree, None)
                    continue  # Don't run normal trigger check

                # ... rest of trigger logic unchanged ...
```

### Constraints

- Don't spawn duplicate runs (check `_is_agent_running` first)
- Keep existing error handling — if spawning fails, log and continue
- No delay between completion and re-trigger (that's what "loop" means)

---

## 2. Cron Trigger Backend

### Current state

UI shows cron option (`TriggerKind.cron`), frontmatter supports `cron:` field, but `triggers.py` doesn't evaluate cron expressions.

### What's needed

Add cron evaluation to `should_trigger()`. Use standard 5-field cron syntax: `minute hour day month weekday`.

```python
# triggers.py

from datetime import datetime

def should_trigger(
    agent: AgentFile,
    last_run_at: datetime | None,
    last_main_sha: str | None,
) -> bool:
    """Check if an agent's trigger condition is met."""
    if agent.trigger == "manual":
        return False

    if agent.trigger == "loop":
        return True

    if agent.trigger == "cron":
        return _cron_matches(agent.cron, last_run_at)

    if agent.trigger == "main-changed":
        return _main_changed(agent.repo, last_main_sha)

    if agent.trigger == "interval":
        return _interval_elapsed(agent.interval_seconds, last_run_at)

    return False


def _cron_matches(cron_expr: str | None, last_run_at: datetime | None) -> bool:
    """Check if current time matches cron expression.

    Format: minute hour day month weekday
    Supports: numbers, *, */N (every N)
    """
    if not cron_expr:
        return False

    now = datetime.now()

    # Don't trigger if we already ran this minute
    if last_run_at and (now - last_run_at).total_seconds() < 60:
        return False

    parts = cron_expr.split()
    if len(parts) != 5:
        return False

    minute, hour, day, month, weekday = parts

    return (
        _matches_field(minute, now.minute) and
        _matches_field(hour, now.hour) and
        _matches_field(day, now.day) and
        _matches_field(month, now.month) and
        _matches_field(weekday, now.weekday())  # 0=Monday
    )


def _matches_field(pattern: str, value: int) -> bool:
    """Check if a cron field matches a value."""
    if pattern == "*":
        return True

    if pattern.startswith("*/"):
        try:
            step = int(pattern[2:])
            return value % step == 0
        except ValueError:
            return False

    try:
        return int(pattern) == value
    except ValueError:
        return False
```

### AgentFile update

Add `cron` field to `AgentFile` dataclass:

```python
# markdown.py

@dataclass
class AgentFile:
    """Parsed agent markdown file."""
    name: str
    path: Path
    repo: Path
    pipeline: list[str]
    trigger: str
    context: list[str]
    prompt: str
    interval_seconds: int | None = None
    cron: str | None = None  # Add this
    fresh: bool = False


def parse_agent_file(path: Path) -> AgentFile | None:
    # ... existing parsing ...

    return AgentFile(
        # ... existing fields ...
        cron=config.get("cron"),  # Add this
    )
```

### Constraints

- Keep cron parsing minimal — no third-party dependencies
- Support common cases: `0 9 * * *` (9am daily), `*/15 * * * *` (every 15 min)
- Don't support complex features like ranges (`1-5`) or lists (`1,15`) initially
- Weekday uses Monday=0 to match Python's `datetime.weekday()`

---

## 3. Context Paths Add Button

### Current state

AgentDetailPanel shows existing context paths as removable chips but has no way to add new paths.

### What's needed

Add a "+" button that shows a file picker or text field to add context paths.

```swift
// AgentDetailPanel.swift - update configSection

// Context
VStack(alignment: .leading, spacing: 6) {
    HStack {
        Text("Context")
            .font(.caption)
            .foregroundStyle(.secondary)

        Spacer()

        Button {
            showingContextPicker = true
        } label: {
            Image(systemName: "plus")
                .font(.caption)
        }
        .buttonStyle(.borderless)
        .popover(isPresented: $showingContextPicker) {
            contextPickerPopover()
        }
    }

    // ... existing chips display ...
}


@State private var showingContextPicker = false
@State private var newContextPath = ""

private func contextPickerPopover() -> some View {
    VStack(alignment: .leading, spacing: 12) {
        Text("Add Context Path")
            .font(.caption)
            .foregroundStyle(.secondary)

        HStack {
            TextField("src/schema.py", text: $newContextPath)
                .textFieldStyle(.roundedBorder)
                .frame(width: 200)

            Button("Add") {
                if !newContextPath.isEmpty {
                    contextPaths.append(newContextPath)
                    hasChanges = true
                    newContextPath = ""
                    showingContextPicker = false
                }
            }
            .disabled(newContextPath.isEmpty)
        }

        Text("Relative to repo root")
            .font(.caption2)
            .foregroundStyle(.tertiary)
    }
    .padding(12)
}
```

### Constraints

- Text field input is sufficient — file picker adds complexity for little gain
- Path is relative to agent's repo root
- Validate non-empty before adding
- Clear field after adding

---

## Done when

### Loop trigger

```bash
# Create agent with loop trigger
cat > ~/.lf/agents/test-loop.md << 'EOF'
---
repo: /tmp/test-repo
pipeline: [echo-hello]
trigger: loop
---
Test loop agent.
EOF

# Start daemon in foreground
python -m loopflow.maestro.daemon

# Output should show:
# [timestamp] Triggering agent: test-loop (trigger: loop)
# [timestamp] Started agent test-loop (PID X)
# [timestamp] Agent test-loop completed (was PID X)
# [timestamp] Loop re-triggering agent: test-loop
# [timestamp] Started agent test-loop (PID Y)
# ... repeats until stopped
```

### Cron trigger

```bash
# Create agent with cron trigger (every minute for testing)
cat > ~/.lf/agents/test-cron.md << 'EOF'
---
repo: /tmp/test-repo
pipeline: [echo-hello]
trigger: cron
cron: * * * * *
---
Test cron agent.
EOF

# Start daemon in foreground
python -m loopflow.maestro.daemon

# Wait for minute boundary
# Output should show:
# [timestamp] Triggering agent: test-cron (trigger: cron)
```

### Context paths add button

1. Open Maestro, Cmd+Shift+A to open Agents window
2. Select an agent
3. Click "+" button next to Context
4. Enter a path like `src/models.py`
5. Click Add
6. Chip appears in context list
7. Save changes
8. Verify path appears in agent file frontmatter
