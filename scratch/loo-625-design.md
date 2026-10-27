# LOO-625: Set up each area of work once

Accepted by Jack Heart, 2026-10-26, in the LOO-625 Task conversation. Brief: the Task
description. Decisions: `scratch/loo-625-direction.md`.

## Demo

```
# .lf/wave/infrastructure/config.yaml   (committed)
workflow: code
agent: claude
skill_agents:
  implement: claude:opus

lf set machine daytona --wave infrastructure    # personal, local store
lf set account.claude work --wave infrastructure

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
- `workflow:` is committed and edited by hand in the Wave's `config.yaml`; the
  `waves.workflow` column and its readers are deleted (one schema
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

- Machine: the existing Wave machine row. Accounts: `RouteScope::Wave` beside `Repo |
  Default`; resolution Wave > Repo > Default. Both are written by `lf set` (One verb).
- `workflow`, `agent`, `skill_agents` are edited in `config.yaml` by hand; no command
  writes committed config (Jack Heart, 2026-10-27).

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
`skill_agents`, launch/Task agent resolution per Wave, `waves.workflow` dropped (draft `wave_setup`), `RouteScope::Wave`,
`WaveDetailSnapshot.setup` (`work/wave/setup.rs`),
GOAL.md fallback marked LOO-640, this repository's `.lf/wave/*/config.yaml`, docs.

End to end: `a_task_run_takes_its_waves_agent_and_account_without_flags`
(`tests/linear/task_graph_launch_tests.rs`) runs `lf -b task run` with a Claude stand-in;
the step launches on the Wave's `claude:opus` with the Wave-routed `work` account.

Launches and `lf wave status` share one route chain
(`provider_account::nearest_provider_route`); `LF_WAVE_ID` is read by
`wave::context::ambient_wave_id`. `config::workflow_setting` is the one
Wave → repo → builtin `code` Workflow resolution (Task default and Wave setup).

`lf set` / `lf unset` (One verb, below; Jack Heart steers 2249b6d5, 1a78ed7e, 4d4360a5,
b4f19861): `lf/commands/setting.rs`, `scope_settings` table in draft `wave_setup`,
`ConfigLayer::Personal` in `config::config_layers`, source `personal` in `lf wave status`.
`lf wave workflow set`, `lf machine use` (its Task handoff moved to
`lf task move --machine` on main, #1720) and `lf account route set` are deleted; Desktop,
tests, docs and skills moved.

## Settled history (2026-10-27)

- Cross-repository Wave workflow migration dropped (Jack Heart steer 2a6771a3); manabot
  and etude-studio selections were written by hand.
- Keychain escape: launch tests use `LF_ACCOUNT_ISOLATION=isolated`.
- Concept review 2 (`lf set`, against Jack Heart steer b4f19861): shape holds. Root
  `--wave` scopes it, no `--wave` is the repository, `--global` every repository, store
  only. `global` has two writers for `agent`/`skill_agents` (`~/.lf/config.yaml` and
  `lf set --global`, the store value above the file), both reported `global`; kept as
  `questions.md` records, not a product block. Its one doc defect, `release/CRON_HOST.md`
  teaching `lf machine use`, now reads `lf set machine mini --wave infrastructure`
  (2e16d1c224); only immutable release notes still name the deleted forms.
- Concept review: one source vocabulary. `task_workflow_default` returns `SetupSource`;
  Task JSON, `lf wave status` and the missing-Workflow error say
  `task | personal | wave | repo | global | default`. Swift keeps `String?`.

## Remaining

- Gate: full suites (incl. `machine_commands`, `two_people`, `auth_tests`, Swift
  `WorkCLISubscriptionTests`), clippy, DTO fixtures.

Check (LF_* unset, 2026-10-27, after restoring the `lf set` work from the stash `lf checkout update` left and merging main's Fly removal): `--test integration wave_setup_tests` and CLI-shape tests (`reference_matches_the_compiled_cli`, `wave_resolution_matrix`, `machine_commands`, `cli_discovery`, `documented_commands`) pass; `--lib -- setting config:: scope_defaults` pass with `harness::claude` capture passing serially (concurrency flake); fmt and clippy `--all-targets` clean. Gate owns the full suites.

Compress (2026-10-27): `cargo test --lib lf::tests your_workflow_for_a_wave`, fmt, clippy `--all-targets`: pass.

## One verb: `lf set` (Jack Heart, 2026-10-27, accepted)

Prior art: gcloud named configurations (`gcloud config set <key> <value>`), git config
scopes with `--show-origin`, Slurm associations, kubectl contexts, SkyPilot workspaces.
All key the setting and take the scope once.

**`lf set` never modifies committed config** (Jack Heart). Every key it sets is personal
(local store), for you on this Machine. Shared setup lives in `.lf/wave/<name>/config.yaml`
/ `.lf/config.yaml`, edited by hand and reviewed like code; a personal value overrides the
group's value for you only.

```bash
lf set machine mini --wave infrastructure
lf set workflow auto --wave infrastructure             # your override of the Wave's workflow:
lf set agent claude:opus --wave infrastructure
lf set skill_agents.implement codex --wave infrastructure
lf set account.claude work@example.com --wave infrastructure
lf set machine mini                                    # no --wave: the repository, always
lf set account.claude me@example.com --global          # every repository
lf unset workflow --wave infrastructure                # back to the group's value
lf wave status infrastructure                          # every value and its source
```

- Scope (Jack Heart): `--wave` is the existing root selector (`lf --wave`, same as
  `lf --repo`); no `--wave` means the repository, even inside a Wave's Task worktree;
  `--global` means every repository and replaces `lf account route set --default`.
  Supersedes option B and the `lf wave set` spelling.
- Keys: `workflow`, `agent`, `skill_agents.<skill>`, `machine`, `account.<provider>`.
- Precedence: Task/flag > personal (`lf set` for the Wave, then for the repository)
  > Wave `config.yaml` > `.lf/config.yaml` > `~/.lf/config.yaml` > default. Sources in
  `lf wave status`: `task | personal | wave | repo | global | default` (exact naming is
  the implementer's call, but personal overrides must be distinguishable from the
  committed Wave value).
- Delete the forms it replaces: `lf wave workflow set` (folds into `lf set workflow`,
  now personal), the Wave/repository default form of `lf machine use` (Task
  handoff stays), and `lf account route set --wave` (added on this branch). Fold
  repository scope of account routes into `lf set` with no `--wave`; `--global` replaces
  `lf account route set --default`, so `lf account route set` is gone (`lf account route`
  stays as the read).
- Per-run overrides (`-a`, `--pool`, `--rent`) stay.
- Update README, `docs/work.md#set-it-up-once`, `orchestration.md`, `index.md`,
  `getting-started.md` and builtin skills that teach the old forms; regenerate
  `docs/lf-reference.md`; classify the new `wave` arg in `wave_resolution_matrix`.
- Acceptance tests: shared setup via committed `config.yaml`, personal via `lf set`.
