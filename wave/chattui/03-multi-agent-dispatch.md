# 03: Multi-Agent Dispatch

Choose which agent runs a step: Claude Code, Codex, OpenCode, or a custom command.

## Done when

Wave config or step config can specify the agent. Concerto launches the right command in the tmux session. OpenCode's GUI can be launched for interactive steps.

## What to build

- Agent config per wave or per step (extends existing `agent:` field in wave yaml)
- Command templates: `claude`, `codex`, `opencode` — map agent name to launch command
- For GUI agents (OpenCode): launch the GUI app pointing at the right directory/session instead of a terminal command
- Fallback: if agent not installed, warn and offer alternatives
