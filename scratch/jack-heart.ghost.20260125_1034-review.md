# Design Review: Embedded Interactive Sessions & Step/Flow Reorganization

This PR implements embedded interactive sessions in Concerto and reorganizes the step/flow system for clarity.

## What was implemented

### 1. Embedded Interactive Sessions (Concerto)

Users can now run interactive steps (like `design`) directly in Concerto's embedded Ghostty terminal.

**Key changes:**
- `GhosttyManager`: Added `onSessionClosed` callback, `registerActiveSession()`, `destroyActiveSession()`
- `GhosttyTerminalView`: Added `sessionId` parameter for lifecycle tracking
- `InteractiveSessionView`: Shows session header with End button, connects terminal closure to state cleanup
- `FlowPicker`: Added Interactive toggle to switch between auto and interactive modes
- `WaveDetailPanel`: Switches to session mode when interactive session is active

**State management:**
- `InteractiveSession` struct in `SessionState` tracks active session
- One session at a time (MVP constraint)
- Terminal process killed when user clicks End or closes Concerto

### 2. Tick-based Flow Execution (lfd)

Flows with interactive steps now pause at those steps, allowing CLI connection.

**Key changes:**
- `TickResult` enum: STEP_COMPLETE, FLOW_COMPLETE, WAITING_INTERACTIVE, STEP_FAILED
- `tick_flow()` in runner.py: State machine executor that advances one step per tick
- `_run_tick_loop()` in worker.py: Runs tick loop for flows with interactive steps
- `_has_interactive_steps()`: Checks if any step in flow has `interactive: true` frontmatter
- `lfd connect <wave>`: Attaches terminal to a wave's waiting interactive step

**Data model:**
- `step_index` field on FlowRun for position tracking
- `flow_run_id` and `wave_id` fields on StepRun for linking
- Migration `m_2026_01_26_step_index.py` adds column if missing

### 3. Step/Flow Reorganization

Moved from flat Python-based flows to organized YAML-based flows.

**Directory structure:**
```
src/loopflow/lf/builtins/
├── steps/
│   ├── code/       # debug, implement, compress, gate
│   ├── interactive/  # design, explore, refine
│   ├── ops/        # consolidate, synthesize, validate
│   └── plan/       # review, reduce, polish, expand, iterate, etc.
├── flows/
│   ├── code/       # ship, pair, grind, incident, start
│   └── plan/       # roadmap-reduce, roadmap-polish, roadmap-expand, research, publish
└── directions/
    ├── roles/      # ceo, designer, product-engineer, infra-engineer
    └── values/     # craft, flow, scale
```

**Key changes:**
- Flows now use YAML format instead of Python
- `Flow` class unified (was `Flow` + `FlowDef`)
- Steps/directions support subdirectory organization with `find_md_in_dir()` helper
- Autopromote: If no flow exists but a step does, creates single-step flow

### 4. Swift Cleanup

- Removed `NewWaveSheet` (replaced with direct creation + inline name editing)
- Removed duplicate `decodeOptionalStringArray` helper
- Simplified `WorktreeService` to always use lfops (never calls wt directly)
- Fixed typo: "an wave" → "a wave"
- Fixed `LaunchAgents` path (was `LaunchWaves`)
- Faster reconnect loop at startup (500ms for first 10 attempts)

## Key choices

| Decision | Why | Alternatives rejected |
|----------|-----|----------------------|
| YAML for flows | Simpler than Python, easier to parse, no import machinery | Keep Python (more flexible but harder to validate) |
| Tick-based execution | Survives daemon restart, clean interactive pauses | Keep in-memory loop (simpler but loses state) |
| One interactive session | MVP simplicity | Multiple sessions (more complex lifecycle) |
| Kill process on End | Clean termination, no orphans | Detach (complex, would need session resume) |
| Autopromote steps to flows | Simpler UX for running single steps | Require explicit flow definition |

## How it fits together

```
User clicks "Run" with Interactive ON
    → SessionState.launchInteractiveSession()
    → WaveDetailPanel shows InteractiveSessionView
    → GhosttyTerminalView runs `lf <step>`
    → User works with Claude Code
    → Process exits or user clicks End
    → GhosttyManager.onSessionClosed fires
    → SessionState.endInteractiveSession()
    → WaveDetailPanel returns to config mode
```

For daemon flows with interactive steps:
```
Daemon calls run_wave_iterations()
    → _has_interactive_steps() returns true
    → _run_tick_loop() creates FlowRun, calls tick_flow()
    → tick_flow() hits interactive step
    → Creates StepRun with status=WAITING
    → Sets Wave status to WAITING
    → Returns WAITING_INTERACTIVE
    → Worker exits, wave paused

User runs `lfd connect <wave>`
    → Finds WAITING StepRun
    → Runs interactive session
    → On success: autocommit, mark COMPLETED, set wave RUNNING
    → Daemon picks up and continues
```

## Risks and bottlenecks

| Risk | Mitigation |
|------|------------|
| Terminal crashes during session | Process is killed, session cleared, user can retry |
| Daemon restart while WAITING | State in DB, daemon picks up where it left off |
| User runs `lf design` directly in worktree while wave waiting | Could conflict; not blocked (user's problem) |
| Large flows with many steps | step_index handles position; no in-memory state |

## What's not included

- Multiple concurrent interactive sessions (MVP: one at a time)
- Session resume after Concerto restart
- Token summary bar in session view
- Concerto "Connect" button for WAITING waves (waves in WAITING show in sidebar, but no explicit connect button yet)
- Detach interactive session to external terminal
