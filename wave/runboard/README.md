# Runboard

## Vision

The daily surface for multi-agent coordination. A wave manager with real-time agent visibility — what are my agents doing right now, and can I steer them without drowning in tmux tabs.

The runboard is the lowest of three zoom levels on the same system:

| Altitude | Surface | Question | Timeframe |
|----------|---------|----------|-----------|
| Low | Runboard (cockpit) | "What are my agents doing?" | Right now |
| Mid | Beat Sequencer (studio) | "What's the rhythm?" | Daily |
| High | Portfolio (gallery) | "How's it going?" | Weekly |

Build the cockpit first. Earn the right to go higher.

The runboard serves two audiences with one surface:
- **Power users** (Dustin, Garry) who already have custom tooling — the runboard is the daily surface where their imported workstyles run visibly
- **Engineering leads** who just want to see what their 3-5 agents are doing — no workstyle configuration needed

## Strategy

The wave is the unit of display, not the agent. Each wave row shows mode, current step, status, and mode-appropriate actions. Expanding a row reveals detail appropriate to the wave's execution mode:
- **Loop wave:** beat history (play/tune/silence rhythm), live agent output, pause/skip/stop
- **Manual wave:** step-by-step flow progress, result on completion, cancel
- **Cron wave:** run history, next scheduled time, manual trigger

Looping is the destination; manual is the onramp. Most users start with manual waves and graduate to looping as they trust the system. The runboard makes both first-class, but the aspiration is ambient awareness of looping agents — not a task launcher.

### Data model

- **Wave record:** name, mode (loop/manual/cron), current flow step, status, active agent provider, branch, worktree path
- **Status enum:** `running` | `idle` | `blocked` | `error` | `done` | `sleeping`
- **Beat history:** ordered list of recent beats per wave — type (play/tune/silence), timestamp, outcome

### Health detection

Start with Claude Code output patterns parsed from lfd terminal sessions. Agent-specific parsers are pluggable — agnostic at the display layer, specific at the detection layer. Add Codex/OpenCode adapters as users need them.

### Surface

lfd HTTP API serves the data. Concerto (macOS) and/or a web UI in lfd renders it. tmux status line shows summary; detail view lives in a richer surface. Resolve during implementation based on what ships fastest.

## Goals

- An engineering lead with no custom tooling can use the runboard within 5 minutes of install
- Dustin and Garry see their imported workstyles running with real-time visibility they don't have in their custom tools
- Daily active usage — the thing you open every morning
- The runboard is the surface that makes loopflow sticky

## Risks

- "Just a tmux wrapper" perception — the runboard must show information that raw tmux can't (wave context, beat history, cross-wave health)
- Platform players (Claude Agent Teams, Codex App) building good-enough orchestration UX into their products
- Building for power users (Dustin/Garry) produces something too complex for the engineering lead onramp
- Shared scratchpad and cross-wave coordination (Phase 2) may be where the real value is, not the status dashboard (Phase 1)

## Metrics

- Time from install to first wave visible in runboard (<5 min)
- Daily active users checking runboard
- Percentage of users who graduate from manual to loop mode within 2 weeks
- Retention: daily usage for 2+ weeks
