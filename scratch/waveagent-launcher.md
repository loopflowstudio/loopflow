---
requires: existing code
produces: code changes
---
The keystone: launch a Wave's goal as a continuously-looping top-level agent that
spawns monitorable subagents via `lf op dispatch` + `lfq`.

This is the culmination of the waveagent work. The substrate exists: goal render
(`render_goal`/`GoalRenderContext` in `engine/flow.rs`), the `--operate` prompt
(`OPERATE.md`, `PromptComponents.operate`), the dispatch endpoint
(`POST /v0/waves/{id}/dispatch {flow, task}` → its own attachable tmux session), and
the lfq cockpit (`lfq sessions`/`lfq attach`). This unit wires them into a launchable,
continuously-looping goal agent.

## 1. `lf goal <name>` — launch a wave's goal as a looping agent

New top-level command (in `rust/loopflow/src/lf/mod.rs` + `commands/`). `lf goal <name>`:
- Resolves the wave `<name>` (its repo/worktree) and loads its goal via `load_goal`
  (`wave/<name>/GOAL.md` → builtin fallback).
- Builds the goal prompt with `render_goal` + a full `GoalRenderContext`: `flows =
  available_flow_names(repo)`, `roadmap` = the wave's `roadmap:` handle from GOAL.md,
  `memory` = `wave/<name>/MEMORY.md`, `metrics`, `in_flight` = the wave's open
  dispatches (reuse the executor's in-flight lister if reachable, else empty).
- Launches an **interactive** agent session with `operate = true` (so `<lf:operate>`
  /OPERATE.md is injected) carrying that goal prompt as the task. Reuse
  `prepare_launch_prompt` (add a `prepare_goal_launch` wrapper that sets operate=true,
  surface = interactive, and uses the rendered goal as the message/task). This is the
  top-level looping agent — a long-lived session the human watches.
- The goal/operating prompt already instructs continuous looping (read roadmap →
  dispatch → re-measure → repeat); the session is long-lived, so "continuous loop" is
  the agent's prompted behavior within one session. A `--once` flag may cap it to a
  single iteration for demos; default is the looping prompt.

## 2. `lf op dispatch` — spawn a monitorable subagent from the loop

New `lf op dispatch --wave <w> --flow <f> --task <t>` (in `commands/ops/`):
- POSTs to `/v0/waves/{id}/dispatch {flow, task}` (the endpoint added in the dispatch
  unit) and prints the created run/session id.
- This is how the looping agent spawns real work as its **own attachable tmux session**
  (not an inline shell-out), so each subagent is independently monitorable/steerable.
- Add a client method if needed; follow existing `lf op` command patterns.

## 3. Teach the loop to use dispatch + lfq (OPERATE.md + render_goal prompt)

Update `rust/loopflow/src/engine/builtins/OPERATE.md` "Delegate Work" section (and the
`render_goal` operating text in `engine/flow.rs` if it duplicates guidance) to name the
concrete mechanism:
- To do real work, run `lf op dispatch --wave <this-wave> --flow <flow> --task "<task>"`.
  The child runs as its own tmux session that you and the human can monitor and steer.
- The human watches/enters subagents with `lfq sessions` (live sessions, needs-input
  flagged) and `lfq attach <id>` (drop into one over tmux to answer an interactive
  step).
- Keep it tight and imperative (PROMPT_STYLE.md).

## 4. Tests

- `lf goal <name>` builds a launch prompt that contains `<lf:operate>` and the rendered
  goal (`<lf:goal-context>` / the goal body), with an interactive surface. Follow the
  `prepare_launch_prompt`/`skill_launch_seed` test patterns in `launch.rs`/`run.rs`.
- `lf op dispatch` calls the dispatch endpoint with the right flow/task (mock the HTTP
  layer; assert behavior, not mock wiring).
- OPERATE.md contains the `lf op dispatch` + `lfq` guidance (a simple contains-check is
  fine, or assert the rendered operate section mentions dispatch).

## Guardrails

- Reuse `prepare_launch_prompt`, `render_goal`, the dispatch endpoint, and lfq — do not
  reinvent. Do not build a supervisor/heartbeat or persistent 24/7 daemon here; the
  looping is the agent's prompted behavior in a long-lived interactive session.
- Do not touch trigger/cron code or delete unrelated functions.
- `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test -p loopflow`,
  `uv run pytest python/tests/` green. Regenerate goldens only if prompt text changed.

## Output

`lf goal <name>` launches a wave's goal as a looping top-level agent (operate on); the
agent uses `lf op dispatch` to spawn subagents as their own monitorable tmux sessions;
`lfq sessions`/`lfq attach` let a human watch and steer them. Tests green.
