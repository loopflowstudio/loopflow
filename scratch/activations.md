# Rename "Trigger" / "Agent Mode" to "Stimulus"

Replace "trigger" and "agent mode" with "stimulus" throughout the codebase and documentation. The neural analogy is intentional—agents respond to stimuli.

## What to Build

Replace `AgentMode` enum with a `Stimulus` dataclass. The stimulus determines when an agent runs.

```python
@dataclass
class Stimulus:
    kind: Literal["once", "loop", "watch", "cron"]
    cron: str | None = None  # only when kind == "cron"
```

## Data Structures

### Before

```python
class AgentMode(str, Enum):
    LOOP = "loop"
    WATCH = "watch"
    CRON = "cron"

class Agent:
    mode: AgentMode
    watch_paths: str | None  # separate field
    cron: str | None         # separate field
    area: list[str]
```

### After

```python
@dataclass
class Stimulus:
    kind: Literal["once", "loop", "watch", "cron"]
    cron: str | None = None

class Agent:
    stimulus: Stimulus
    area: list[str]  # also serves as watch paths for WATCH stimulus
```

Key simplifications:
- `watch_paths` removed — use `area` instead (what you watch = what you work on)
- `cron` moved into `Stimulus` — only CRON needs extra config
- `AgentMode` enum → `Stimulus` dataclass

## Usage

```python
# One-shot
Agent(stimulus=Stimulus("once"), area=["src/"])

# Loop
Agent(stimulus=Stimulus("loop"), area=["src/"])

# Watch (monitors area for changes)
Agent(stimulus=Stimulus("watch"), area=["src/api/"])

# Cron
Agent(stimulus=Stimulus("cron", cron="0 9 * * *"), area=["src/"])
```

---

## Terminology Mapping

### Global Find/Replace

| Before | After |
|--------|-------|
| Agent Mode | Stimulus |
| agent mode | stimulus |
| AgentMode | Stimulus |
| mode: AgentMode | stimulus: Stimulus |
| agent.mode | agent.stimulus |
| trigger | stimulus (or "activates" as verb) |
| triggered | activated |
| TRIGGERED | ACTIVATED |

### In Prose

| Before | After |
|--------|-------|
| "The three modes" | "The four stimuli" (once, loop, watch, cron) |
| "triggers one iteration" | "activates one iteration" |
| "When triggered" | "When activated" |
| "Trigger Types" | "Stimulus Types" |
| "Loop mode" | "Loop stimulus" |
| "Watch mode" | "Watch stimulus" |
| "Cron mode" | "Cron stimulus" |

---

## Documentation Changes

### README.md

**Before:**
```markdown
| Agent Mode | Runs when |
|------------|-----------|
| **Loop** | Continuously until stopped |
| **Watch** | Paths change on main |
| **Cron** | On schedule |
```

**After:**
```markdown
| Stimulus | Runs when |
|----------|-----------|
| **Once** | Single run (one-shot) |
| **Loop** | Continuously until stopped |
| **Watch** | Paths change on main |
| **Cron** | On schedule |
```

### docs/index.md

Same table change as README.md.

### docs/agents.md

**Before:**
```markdown
## The Three Modes

| Mode | Runs when | Command |
|------|-----------|---------|
| **Loop** | Continuously until stopped | `lfd loop` |
| **Watch** | Paths change on main | `lfd subscribe` |
| **Cron** | On schedule | `lfd schedule` |
```

**After:**
```markdown
## Stimulus Types

| Stimulus | Runs when | Command |
|----------|-----------|---------|
| **Once** | Single run | `lfd run` |
| **Loop** | Continuously until stopped | `lfd loop` |
| **Watch** | Paths change on main | `lfd subscribe` |
| **Cron** | On schedule | `lfd schedule` |
```

Also update:
- "When files change under the watched paths on main, triggers one iteration."
  → "When files change in the area on main, activates one iteration."

### docs/lfd.md

**Section rename:**
```markdown
## Triggers        →    ## Stimulus Commands
```

**Help text updates:**
- "triggers one iteration" → "activates one iteration"
- Remove `-p, --path` flag from `lfd subscribe` (use area instead)

---

## Code Changes

### Python

| File | Changes |
|------|---------|
| `src/loopflow/lfd/models.py` | `AgentMode` → `Stimulus` dataclass, `mode` → `stimulus` |
| `src/loopflow/lfd/agent.py` | Function renames, field access, remove watch_paths logic |
| `src/loopflow/lfd/logging.py` | `trigger_log` → `stimulus_log`, "TRIGGERED" → "ACTIVATED" |
| `src/loopflow/lfd/cli.py` | Remove `-p` from subscribe, update help text |
| `src/loopflow/lfd/daemon/server.py` | Function calls |
| `src/loopflow/lfd/migrations/` | New migration for schema changes |
| `src/loopflow/lfd/README.md` | Internal docs terminology |

### Swift

| File | Changes |
|------|---------|
| `swift/LoopflowCore/Models/Agent.swift` | `AgentMode` → `Stimulus` struct, `mode` → `stimulus` |

### Function Renames

| Before | After |
|--------|-------|
| `should_trigger_watch()` | `should_activate_watch()` |
| `should_trigger_cron()` | `should_activate_cron()` |
| `check_watch()` | `check_watch_stimulus()` |
| `check_cron()` | `check_cron_stimulus()` |
| `run_watch_check()` | `run_watch_check()` (keep) |
| `run_cron_check()` | `run_cron_check()` (keep) |
| `_determine_mode()` | removed — stimulus is explicit |

### Logging

```python
# Before
trigger_log = get_lfd_logger("lfd.trigger")
trigger_log.info("TRIGGERED")

# After
stimulus_log = get_lfd_logger("lfd.stimulus")
stimulus_log.info("ACTIVATED")
```

---

## Database Migration

Replace columns:
- `mode` → `stimulus_kind`
- `watch_paths` → removed (use `area`)
- `cron` → `stimulus_cron`

```sql
ALTER TABLE agents RENAME COLUMN mode TO stimulus_kind;
ALTER TABLE agents RENAME COLUMN cron TO stimulus_cron;
ALTER TABLE agents DROP COLUMN watch_paths;
```

---

## CLI Changes

Commands stay the same: `lfd loop`, `lfd subscribe`, `lfd schedule`, `lfd run`

`lfd subscribe` simplifies—no separate `-p` flag:

```bash
# Before
lfd subscribe ship src/api/ -p src/api -p tests/

# After
lfd subscribe ship src/api/   # watches area for changes
```

---

## Constraints

- CLI command names unchanged
- `ONCE` stimulus for one-shot runs (`lfd run`)
- `area` serves dual purpose: context for agent + watch paths for WATCH stimulus
- No backwards compatibility shims

---

## Done When

```bash
# No "trigger" in lfd code (except tests)
grep -ri "trigger" src/loopflow/lfd/ --include="*.py" | grep -v test | grep -v __pycache__
# Output: empty

# No "AgentMode" references
grep -r "AgentMode" src/loopflow/lfd/ --include="*.py"
# Output: empty

# No "Agent Mode" in docs
grep -ri "agent mode" docs/ README.md
# Output: empty

# Stimulus dataclass exists
grep "class Stimulus" src/loopflow/lfd/models.py
# Output: class Stimulus:

# Docs use "Stimulus" terminology
grep -i "stimulus" docs/agents.md | head -3
# Output: lines with "Stimulus Types", etc.
```
