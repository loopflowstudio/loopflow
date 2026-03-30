# Runboard: Real-time wave manager

## Problem

Multi-agent coordination UX is terrible. Engineers running 3+ agents lack visibility, control, and a sane way to coordinate handoffs. The runboard is the daily surface that makes loopflow sticky — what are my agents doing right now, and can I steer them.

## Scope

Phase 1 only — the cockpit. Not the beat sequencer (Phase 2) or portfolio (Phase 3).

## Data model

- **Wave record:** name, mode (loop/manual/cron), current flow step, status enum, active agent provider, branch, worktree path
- **Status enum:** `running` (agent executing), `idle` (loop between beats), `blocked` (waiting on human/CI), `error` (step failed), `done` (manual complete), `sleeping` (cron between runs)
- **Beat history:** ordered list of recent beats per wave — type (play/tune/silence), timestamp, outcome (PR, mutation, no-op)

## What to build

### 1. Wave status API

lfd HTTP endpoint returning all waves with current status, mode, step, and recent beats. WebSocket or SSE stream for real-time updates.

The status enum is derived from lfd's existing run/session state. Beat history comes from run journals.

### 2. Runboard UI

Wave list as expandable rows. Each row shows mode, current step, status, and mode-appropriate actions.

```
┌─────────────────────────────────────────────────────────┐
│ Wave          Mode     Step          Status    Actions   │
│─────────────────────────────────────────────────────────│
│ engbot        loop     implement     ████░░    ⏸ ⏭ ⏹    │
│ auth-fix      manual   gate          ████████  ✓ done    │
│ dep-scan      cron     (next: 6h)    sleeping  ▶ trigger │
└─────────────────────────────────────────────────────────┘
```

Expanding a row shows mode-appropriate detail:
- **Loop wave:** beat history (play/tune/silence rhythm), live agent output stream, pause/skip-step/stop
- **Manual wave:** step-by-step flow progress, result on completion, cancel
- **Cron wave:** run history, next scheduled time, manual trigger

Surface: lfd HTTP API serves the data. Concerto (macOS) and/or web UI in lfd renders it. tmux status line shows summary. Resolve during implementation based on what ships fastest.

### 3. Agent health detection

Pluggable agent output parsers that read lfd terminal sessions and return a `HealthState` enum. Start with Claude Code adapter — parse tool calls, thinking indicators, error patterns, completion signals. Add Codex/OpenCode adapters later (same interface, different patterns).

Agnostic at the display layer, specific at the detection layer.

## Execution model

Looping is the destination; manual is the onramp. Most users start with manual waves and graduate to looping. The runboard makes both first-class.

## What to skip

- Shared scratchpad (Phase 2 — requires coordination mechanism design)
- Beat sequencer grid (Phase 2)
- Portfolio cross-wave view (Phase 3)
- Adaptive workstyle engine
- Cross-wave conflict detection
- Codex/OpenCode health adapters (add when needed)

## Existing infrastructure to build on

- `lfd` daemon — already serves HTTP, manages wave state, records terminal sessions
- tmux plugin — already shows wave state in status line
- Concerto — macOS app with existing wave/workspace views
- `lfq` CLI — already queries lfd for wave status

## Validation

```bash
# Wave status API returns real-time data for all active waves
curl localhost:$LFD_PORT/api/waves | jq '.[] | {name, mode, status, step}'

# Runboard UI renders and updates in real-time
# (manual verification — launch 2+ waves, confirm live status)

# Health detection correctly identifies running/idle/blocked/error
# (launch a wave, observe status transitions in the runboard)
```

## Done when

- `lfq` or runboard UI shows all active waves with live status
- Expanding a wave shows mode-appropriate detail (loop beat history, manual progress, cron schedule)
- Health detection correctly classifies Claude Code agent states from terminal output
- An engineer can launch, monitor, and stop waves from the runboard without touching tmux directly
