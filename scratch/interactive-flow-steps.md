# Interactive Flow Steps

Flows can contain interactive steps. When a flow reaches an interactive step, it pauses and waits for the user to connect.

## The Model

```
lf flow pair                    # starts flow: design → implement → polish
                                #              ↓
                                #         [interactive]
                                #              ↓
                                #         PAUSED

lfd connect swift-falcon        # attaches terminal to paused session
                                # user works with Claude Code
                                # session exits naturally
                                #              ↓
                                #         autocommit
                                #              ↓
                                #         implement → polish (continue in auto)
```

Same model works for `lfd loop` / `lfd watch` — when they hit an interactive step, the agent pauses until connected.

## State Machine Architecture

Flow execution is a state machine, not a process loop. State lives in the database.

```
┌─────────────────────────────────────────────────────────────┐
│                        Database                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                   │
│  │  Wave    │  │ FlowRun  │  │ StepRun  │                   │
│  │ status   │──│ position │──│ status   │                   │
│  └──────────┘  └──────────┘  └──────────┘                   │
└─────────────────────────────────────────────────────────────┘
        ↑                ↑                ↑
        │                │                │
   ┌────┴────┐     ┌─────┴─────┐    ┌─────┴─────┐
   │ Daemon  │     │ Executor  │    │  Connect  │
   │ (tick)  │     │ (auto)    │    │  (user)   │
   └─────────┘     └───────────┘    └───────────┘
```

**Key principle:** No executor owns the flow. Any process can:
1. Read current state from DB
2. Execute the next step
3. Write results back to DB

Benefits:
- Daemon restart doesn't lose progress
- Interactive pauses are natural (just another state)
- Multiple executors could parallelize (future)
- Testable — inject state, verify transitions

## Wave Status

`WaveStatus` already has the values we need:

```python
class WaveStatus(str, Enum):
    IDLE = "idle"
    RUNNING = "running"    # executing auto step
    WAITING = "waiting"    # paused at interactive step (already exists!)
    ERROR = "error"
```

No changes needed — `WAITING` already means "paused, waiting for something".

## Stored Session State

Use existing models — no new fields needed.

When pausing at an interactive step:
1. `StepRun` created with `status=WAITING`, `run_mode="interactive"`
2. `FlowRun` tracks current position via `current_step`
3. `Wave.status` becomes `WAITING`

```python
# Existing models handle this:
class StepRunStatus(str, Enum):
    RUNNING = "running"
    WAITING = "waiting"      # ← paused at interactive step
    COMPLETED = "completed"
    FAILED = "failed"

class StepRun:
    step: str               # which step
    flow_run_id: str        # links to FlowRun
    wave_id: str            # which wave
    worktree: str           # where to run
    run_mode: str           # "auto" or "interactive"
    status: StepRunStatus   # WAITING when paused
```

The prompt is regenerated at connect time via `gather_prompt_components()` using the wave's fields (area, direction, etc.).

## CLI Commands

### `lfd connect <wave>`

Attaches terminal to a wave's waiting interactive step:

```bash
lfd connect swift-falcon
# Terminal becomes Claude Code session
# Working directory: wave's worktree
# Prompt: assembled from wave config (area, direction) + step
```

When session exits normally (exit code 0):
1. Autocommit changes
2. StepRun status → COMPLETED
3. Wave status → RUNNING
4. Daemon ticks flow forward

When user aborts (Ctrl+C, exit code != 0):
1. No commit
2. StepRun status → FAILED
3. Wave status → IDLE (can retry)

### `lfd status`

Shows waves with their status:

```
swift-falcon    waiting     design (step 1/3 of pair)
ruby-hawk       running     implement
jade-sparrow    idle
```

### `lf flow` (CLI, no daemon)

When run from CLI without daemon, same model:

```bash
lf flow pair
# Creates FlowRun, ticks until interactive
# Prints: "Waiting at 'design'. Run: lfd connect swift-falcon"
# Exits — user connects later
```

Or with `--attach` flag:
```bash
lf flow pair --attach
# Creates FlowRun, ticks until interactive
# Immediately attaches (like calling connect)
```

## Flow Execution Changes

Replace the in-memory loop with a state machine tick.

### Current (process loop)

```python
def run_flow(flow, ...):
    for step in flow.steps:      # ← state in memory
        result = _run_step(...)
        if result != 0:
            return result
    return 0
```

### New (state machine)

```python
def tick_flow(flow_run_id: str) -> TickResult:
    """Advance a FlowRun by one step. Called by daemon or CLI."""
    flow_run = get_flow_run(flow_run_id)
    wave = get_wave(flow_run.wave_id)
    flow = load_flow(flow_run.flow)

    # Find next step
    next_step = get_next_step(flow, flow_run.current_step)
    if not next_step:
        # Flow complete
        flow_run.status = FlowRunStatus.COMPLETED
        save_flow_run(flow_run)
        return TickResult.FLOW_COMPLETE

    # Check if step is interactive
    step_file = gather_step(wave.worktree, next_step.name)
    is_interactive = step_file.config.interactive if step_file else False

    # Create StepRun record
    step_run = StepRun(
        id=uuid4(),
        step=next_step.name,
        flow_run_id=flow_run.id,
        wave_id=wave.id,
        worktree=str(wave.worktree),
        run_mode="interactive" if is_interactive else "auto",
        status=StepRunStatus.WAITING if is_interactive else StepRunStatus.RUNNING,
    )
    save_step_run(step_run)

    if is_interactive:
        # Pause — user must connect
        wave.status = WaveStatus.WAITING
        save_wave(wave)
        return TickResult.WAITING_INTERACTIVE

    # Auto step — execute now
    exit_code = execute_step(step_run, wave)

    step_run.status = StepRunStatus.COMPLETED if exit_code == 0 else StepRunStatus.FAILED
    step_run.ended_at = datetime.now()
    save_step_run(step_run)

    if exit_code != 0:
        flow_run.status = FlowRunStatus.FAILED
        save_flow_run(flow_run)
        return TickResult.STEP_FAILED

    # Update flow position
    flow_run.current_step = next_step.name
    save_flow_run(flow_run)

    return TickResult.STEP_COMPLETE
```

The daemon calls `tick_flow()` repeatedly until it returns WAITING_INTERACTIVE or FLOW_COMPLETE.

## Connect Implementation

```python
def connect(wave_id: str):
    """Attach terminal to a wave's waiting interactive step."""
    wave = get_wave(wave_id)
    if wave.status != WaveStatus.WAITING:
        raise Error(f"Wave not waiting (status: {wave.status})")

    # Find the WAITING StepRun
    step_run = get_waiting_step_run(wave_id)
    if not step_run:
        raise Error("No waiting step run found")

    # Update statuses
    step_run.status = StepRunStatus.RUNNING
    wave.status = WaveStatus.RUNNING
    save_step_run(step_run)
    save_wave(wave)

    # Assemble prompt fresh using wave's config
    components = gather_prompt_components(
        Path(step_run.worktree),
        step_run.step,
        run_mode="interactive",
        direction=wave.direction,
        context_config=ContextConfig(area=wave.area),
    )
    prompt = format_prompt(components)

    # Launch interactive session (blocks until user exits)
    command = build_model_interactive_command(...)
    os.chdir(step_run.worktree)
    exit_code = subprocess.run([*command, prompt]).returncode

    # Session ended
    step_run.status = StepRunStatus.COMPLETED if exit_code == 0 else StepRunStatus.FAILED
    step_run.ended_at = datetime.now()
    save_step_run(step_run)

    if exit_code == 0:
        # Commit and continue flow
        autocommit(Path(step_run.worktree))
        wave.status = WaveStatus.RUNNING
        save_wave(wave)
        # Daemon will pick up and tick the flow forward
    else:
        # User aborted
        wave.status = WaveStatus.IDLE  # or ERROR?
        save_wave(wave)
```

## Concerto Integration

In Concerto, "connecting" means showing the embedded Ghostty terminal:

```swift
// When user clicks "Connect" on a waiting wave
func connectToWave(_ wave: Wave) {
    guard wave.status == .waiting else { return }

    // Show embedded terminal running the session
    // Terminal runs: lfd connect <wave.name>
    appState.activeSession = InteractiveSession(
        waveId: wave.id,
        worktreePath: wave.worktreePath
    )
}
```

The daemon emits events that Concerto observes:
- `wave.waiting` → show "Connect" button
- `wave.connected` → show embedded terminal
- `step_run.completed` → terminal closes, show results

## Edge Cases

### User Ctrl+C during interactive session

Ctrl+C = abort. StepRun marked FAILED, wave goes to IDLE. User can:
- `lfd connect` again to retry the same step
- `lfd cancel <wave>` to abandon the flow

### Multiple interactive steps in sequence

```yaml
steps:
  - design      # interactive
  - refine      # interactive
  - implement   # auto
```

Flow ticks:
1. `design` → StepRun WAITING, wave WAITING
2. User connects → session runs → COMPLETED
3. Daemon ticks → `refine` → StepRun WAITING, wave WAITING
4. User connects → session runs → COMPLETED
5. Daemon ticks → `implement` → runs in auto → COMPLETED
6. Flow complete

### Daemon restart mid-flow

State is in DB. Daemon restarts, reads wave status:
- RUNNING + no active StepRun → tick to create next step
- WAITING → wait for user connect
- Recovery is automatic

### Concurrent connect attempts

Only one `lfd connect` can run at a time per wave. Second attempt sees StepRun already RUNNING, fails with "already connected".

### Timeout

If user never connects, wave stays WAITING forever. Options:
- No timeout (fine for MVP)
- Configurable timeout → wave.status = IDLE, StepRun = FAILED

## Data Model Changes

### No new fields needed

Existing models handle everything:

| Model | Field | Purpose |
|-------|-------|---------|
| `Wave` | `status=WAITING` | Wave is paused at interactive step |
| `FlowRun` | `current_step` | Tracks position in flow |
| `StepRun` | `status=WAITING` | This step needs user connection |
| `StepRun` | `run_mode="interactive"` | Distinguishes from auto steps |

### FlowRun needs position tracking

```python
class FlowRun:
    # ... existing fields ...
    current_step: str | None = None   # last completed step (already exists)
    step_index: int = 0               # position in flow.steps list
```

### New event types

```python
class EventType(str, Enum):
    # ... existing ...
    WAVE_WAITING = "wave.waiting"           # paused at interactive step
    WAVE_CONNECTED = "wave.connected"       # user attached
    STEP_RUN_WAITING = "step_run.waiting"   # step needs connection
```

## Implementation Order

1. **Add `step_index` to FlowRun** — Track position in flow
2. **Create `tick_flow()` function** — State machine step executor
3. **Check step frontmatter** — Determine if interactive
4. **Create StepRun with WAITING status** — Pause at interactive
5. **`lfd connect` command** — Find waiting StepRun, run session
6. **Daemon integration** — Call `tick_flow()` when wave status is RUNNING
7. **Concerto integration** — Show connect button for WAITING waves

## Open Questions

1. **What if user runs `lf design` directly in the worktree while wave is paused?** — Could conflict. Options: warn, block, or let them (their problem).

2. **Retry semantics** — After Ctrl+C abort, does `lfd connect` retry the same step or fail? Current design: retry (create new StepRun).

3. **Flow vs step granularity** — Should `tick_flow` run one step or run until pause/complete? Current design: one step per tick, daemon loops.

4. **Parallel steps in flows** — Existing flow.py supports parallel batches. How does that interact with state machine? Probably: all parallel steps get StepRuns, wait for all to complete before next tick.
