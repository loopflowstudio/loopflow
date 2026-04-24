# Agent Health Detection

**Finish line:** The runboard shows whether each wave's agent is idle, running, blocked, or errored — derived from lfd terminal sessions, not from polling the agent directly.

## Context

Health detection is what separates the runboard from "just a tmux wrapper." Raw tmux shows you terminal output; the runboard shows you *status* — a semantic layer on top of the output.

Start with Claude Code patterns (the most common agent). Adapters are pluggable — one module per agent provider that parses terminal output and returns a status enum.

## What to build

- Health adapter interface: given recent terminal output, return `HealthState` (running, idle, blocked, error, done)
- Claude Code adapter: parse tool calls, thinking indicators, error patterns, completion signals
- Detection runs on lfd's existing `terminal_sessions` data — no new infrastructure
- Adapter registry: lfd picks the right adapter based on the wave's agent config

## What to skip

- Codex and OpenCode adapters (add when users need them — same interface, different patterns)
- Proactive alerting (push notifications when a wave errors)
- Automatic recovery (restart errored waves)
