# LOO-625: Set up each area of work once

Accepted by Jack Heart, 2026-10-26, in the LOO-625 Task conversation. Brief: the Task
description. Decisions: `scratch/loo-625-direction.md`.

## Demo

```
# .lf/wave/infrastructure/config.yaml   (committed)
workflow: code
agent: claude
skills:            # or whatever key .lf/config.yaml already uses for per-skill agents
  implement: claude:opus

lf machine use daytona infrastructure          # personal, local store
lf account route set claude work --wave infrastructure

lf wave status infrastructure --json   # each value with its source: task | wave | repo | global | default
lf task run LOO-xxx                    # runs code on Opus, on daytona, billed to work — no flags
```

A second Wave with different values runs its Task its own way; a Task with no Wave
uses `.lf/config.yaml` and the repository's routes.

## Two layers

**Shared (committed): `.lf/wave/<name>/config.yaml`.** Same schema as `.lf/config.yaml`,
holding only that Wave's values. One loader: `load_config` already merges
`~/.lf/config.yaml` → `.lf/config.yaml`; the Wave file is the next layer, then the Task.
The repository is the Wave of Tasks without one: no Wave file means repo values.

- Repo-only keys (`release`, `planning`, `exclude`, repo `linear.team`, …) in a Wave
  `config.yaml` are an **error** (Jack Heart). Decide the key set from `Config`, not a
  hand list where avoidable.
- `workflow:` is committed. `lf wave workflow set <workflow> <wave>` writes the Wave's
  `config.yaml`; the `waves.workflow` column and its readers are deleted (one schema
  draft via `scripts/new_migration.py`). Customized graph content (`wave_workflows`)
  is out of scope unless it conflicts.
- `agent:` and per-skill agents are shared. They must actually apply to launches
  (Task runs, Graph steps, `lf skill`) with precedence Task/flag > Wave > repo > global.
- GOAL.md stays prose. Its frontmatter (`id`, `linear.initiative`, `chat`,
  `context_budgets`, `agent`, `skill_agents`) moves into the Wave `config.yaml`;
  `WaveConfig` is replaced by the shared `Config` reader plus whatever Wave-only keys
  (`id`, `linear.initiative`, `chat`) need. Released lf reads GOAL.md frontmatter, so the
  branch reads it as a fallback (shared-file rule): mark the fallback with a removal
  Task filed in the same change. This repository's own `.lf/wave/*/GOAL.md`
  frontmatter stays until that release is installed; add the `config.yaml` files beside it.

**Personal (local store rows).** Machine and account routes, per Wave.

- Machine: `lf machine use <machine> <wave>` already stores it; keep it.
- Accounts: add a Wave scope to `RouteScope` (`profile.rs:197`, today `Repo | Default`):
  `lf account route set ... --wave <wave>`. Resolution Wave > Repo > Default.

## Effective setup

`lf wave status <wave>` (and `--json`) reports workflow, agent, per-skill agents,
machine and account routes, each with value and source
(`task | wave | repo | global | default`). DTO rule: no defaults; add fixture under
`tests/fixtures/dto/` and each language's fixture test if Swift mirrors it.

## Acceptance

- Two Waves in one repo with different workflow, agent, machine and account settings each
  run a Task with those settings, no per-command flags (integration test).
- A Task with no Wave uses repo settings.
- `lf wave status <wave> --json` reports each effective setting and its source.
- README "set it up once per area of work" lines gain agent and account examples.
- Dead `skill_agents` / unused `WaveConfig.agent` paths deleted, not left beside the new reader.

## Delete — do not maintain

- `waves.workflow` column, `SqliteStore::{selected_wave_workflow, select_wave_workflow}`,
  their tests; `WorkValue::WaveWorkflow` becomes `HistoricalWaveWorkflow` (wire name
  `wave_workflow` kept so stored planning edits still decode; writes ignore it, like
  `HistoricalProjectWorkflow`). Preserve: Wave default Workflow, now read from the Wave
  `config.yaml`.
- `WaveConfig.{agent, skill_agents, context_budgets}` and GOAL.md as the write target
  (`update_wave_goal_config`). Preserve: `id`, `linear.initiative`, `chat` on the Wave
  file; GOAL.md frontmatter read only as fallback when `config.yaml` is absent.
- `ContextBudgets::resolve` re-reading Wave frontmatter: Wave budgets arrive through the
  layered `Config` with their source path.

## Implementation decisions (2026-10-26)

- Per-skill agents key: `skill_agents:` (map skill → agent), the key Wave frontmatter
  already used; `.lf/config.yaml` had none. Precedence: env override > flag/Task >
  `skill_agents[skill]` > skill frontmatter `agent` > `agent:` > skill `default_agent`.
- Wave layer: `config.yaml` wins whole when present; frontmatter fallback otherwise.
- Repo-only keys rejected in a Wave file: `planning`, `release`, `exclude`,
  `merge_method`, `land`, `pr`; unknown keys also rejected (key set from `Config`).
- Account route Wave scope id is the WaveId.

## Status (2026-10-27)

Implemented: Wave `config.yaml` layer (`config::load_wave_config`, `config_layers`),
`skill_agents`, launch/Task agent resolution per Wave, `lf wave workflow set` writing
`config.yaml`, `waves.workflow` dropped (draft `wave_setup`), `RouteScope::Wave` +
`lf account route set --wave`, `WaveDetailSnapshot.setup` (`work/wave/setup.rs`),
GOAL.md fallback marked LOO-640, this repository's `.lf/wave/*/config.yaml`, docs.

End to end: `a_task_run_takes_its_waves_agent_and_account_without_flags`
(`tests/linear/task_graph_launch_tests.rs`) runs `lf -b task run` with a Claude stand-in;
the step launches on the Wave's `claude:opus` with the Wave-routed `work` account.

Launches and `lf wave status` share one route chain
(`provider_account::nearest_provider_route`); `LF_WAVE_ID` is read by
`wave::context::ambient_wave_id`.

Remaining: none known; gate owns affected suites.
