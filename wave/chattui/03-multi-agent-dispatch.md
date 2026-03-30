# 03: Multi-Agent Dispatch

Choose which agent runs a step: Claude Code, Codex, OpenCode, or a custom command.

## Done when

Wave config or step config can specify the agent. Concerto launches the right command in the tmux session. OpenCode's GUI can be launched for interactive steps.

## Already done

- `Wave.agent: String?` and `Wave.stepAgents: [String: String]?` exist in the model layer — per-wave and per-step agent config is stored and surfaced in UI (FlowProgressPills shows override agents)
- `RepoState.updateWave(..., stepAgents: ...)` persists agent config through `LocalWaveService`

## What to build

- Command templates: `claude`, `codex`, `opencode` — map agent name to launch command in the tmux session
- Wire agent selection into the step launch path so the right binary is invoked (currently model-only, not used at launch time)
- For GUI agents (OpenCode): launch the GUI app pointing at the right directory/session instead of a terminal command
- Fallback: if agent not installed, warn and offer alternatives
