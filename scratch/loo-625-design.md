# LOO-625: Set up each area of work once

Accepted by Jack Heart, 2026-10-26, in the LOO-625 Task conversation. Brief: the Task
description. Jack Heart’s final command shape is recorded under **One verb** below;
it supersedes the earlier `lf wave set` spellings.

## Demo

```
# .lf/wave/infrastructure/config.yaml   (committed)
workflow: code
agent: claude
skill_agents:
  implement: claude:opus

lf set machine daytona --wave infrastructure    # personal, local store
lf set account.claude work --wave infrastructure

lf wave status infrastructure --json   # each value with its source: task | personal | wave | repo | global | default
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

**Personal (local store rows).** Overrides of shared setup, Machine defaults and account routes.

- Machine: the existing Wave machine row. Accounts: `RouteScope::Wave` beside `Repo |
  Default`; resolution Wave > Repo > Default. Both are written by `lf set` (One verb).
- `lf set` may override `workflow`, `agent` and `skill_agents.<skill>` for one person.
  Shared values are edited in `config.yaml` by hand; `lf set` never writes that file
  (Jack Heart, final steer 2026-10-27).

## Effective setup

`lf wave status <wave>` (and `--json`) reports workflow, agent, per-skill agents,
machine and account routes, each with value and source
(`task | personal | wave | repo | global | default`). DTO rule: no defaults; add fixture under
`tests/fixtures/dto/` and each language's fixture test if Swift mirrors it.

## Acceptance

- Two Waves in one repo with different workflow, agent, machine and account settings each
  run a Task with those settings, no per-command flags (integration test).
- A Task with no Wave uses repo settings.
- `lf wave status <wave> --json` reports each effective setting and its source.
- README "set it up once per area of work" lines gain agent and account examples.
- Dead `skill_agents` / unused `WaveConfig.agent` paths deleted, not left beside the new reader.

## Delete — do not maintain

The following removals are implemented; do not restore their authorities.

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
- The account report's duplicate route chain and `provider_routes`' obsolete
  `NEVER_DISPLAYED` classification. Preserve watched Wave setup through planning
  revision triggers in the same migration draft.

## Implementation decisions (2026-10-26)

- Per-skill agents key: `skill_agents:` (map skill → agent), the key Wave frontmatter
  already used; `.lf/config.yaml` had none. Precedence: env override > flag/Task >
  `skill_agents[skill]` > skill frontmatter `agent` > `agent:` > skill `default_agent`.
- Wave layer: `config.yaml` wins whole when present; frontmatter fallback otherwise.
- Repo-only keys rejected in a Wave file: `planning`, `release`, `exclude`,
  `merge_method`, `land`, `pr`; unknown keys also rejected (key set from `Config`).
- Account route Wave scope id is the WaveId.

## Current implementation and review

- `config::config_layers` loads shared and personal values; `workflow_setting` owns
  default Workflow resolution for Tasks and Wave status. `SetupSource` owns the source
  vocabulary for Task JSON, Wave status and missing-Workflow errors.
- `nearest_provider_route` owns account precedence for launches and reports.
  `WaveDetailSnapshot.setup` reports effective values; Swift does not consume it yet.
- `lf set` / `lf unset` own personal writes. The replaced `wave workflow set`,
  `machine use` and `account route set` forms, their dispatch and exclusive tests are
  deleted. Main #1720 owns Task handoff through `lf task move --machine`.
- Cross-repository migration was dropped by Jack Heart (steer 2a6771a3); the two
  external selections were written by hand. Do not write other repositories.
- LOO-640 owns the remaining GOAL.md fallback after a compatible release is installed.
  No other obsolete setup path remains selected for deletion.

Compress removes the account report's duplicate route chain, redundant Wave-key
filtering before deserialization, and per-skill rescans of all config layers. The
settings command enters its async runtime once; config reads retain no extra store
connection. Personal precedence and the Session/Tasks Machine split stay intact.

Review assumptions and acceptance limits remain in `scratch/questions.md`. One real
Task run with a Claude stand-in proves the Wave agent and account route; previews and
status prove the second Wave and repository fallback. Launch tests use isolated
account homes to avoid the native Keychain.

Prior evidence: full nextest had 3028/3032 passing; all four failures passed later
focused checks, including the two Machine tests serially. No complete post-fix
full-suite pass is recorded; gate/CI owns it. Exact commands, logs, counts and the
17,210-row startup budget are preserved at
`e40add38068dbfd2022bcf8f1a589dde020eedb3:scratch/loo-625-design.md`.
The duplicate direction note and obsolete concept-review proposal are preserved at
that commit under `scratch/loo-625-direction.md` and `scratch/concept-review.html`;
the global-file/store overlap remains an implementation assumption in `scratch/questions.md`.

## Remaining

- None selected. Gate (2026-10-27, LF_* unset): `scripts/test.py --reuse-passing` with a filter over the changed modules and setup/Machine/account/DTO/CLI suites passed (Rust 550/550, architecture, website, Swift 638); a forced `cargo clippy --all-targets -D warnings` re-lint passed. Log: /tmp/loo625-gate/gate.log. CI owns the full Rust matrix.

Check (2026-10-27, network-isolated with inherited `LF_*` unset): `cargo test -p loopflow --lib config::` 43 passed; focused `cargo nextest run -p loopflow --lib --test integration` 10/10 passed (setup, account status, Machine defaults, DTO and revision coverage); `scripts/check_migrations.py`, `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed. Logs: `/tmp/loo625-compress-*.log`, including the earlier partial boundary run (46 passed, account-status assertion failed while the draft was changing; its stable-source rerun passed). Gate/CI owns materialized/full-suite coverage.

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
  now personal), `lf machine use` (Task
  handoff uses `lf task move --machine`), and `lf account route set --wave` (added on this branch). Fold
  repository scope of account routes into `lf set` with no `--wave`; `--global` replaces
  `lf account route set --default`, so `lf account route set` is gone (`lf account route`
  stays as the read).
- Per-run overrides (`-a`, `--pool`, `--rent`) stay.
- Update README, `docs/work.md#set-it-up-once`, `orchestration.md`, `index.md`,
  `getting-started.md` and builtin skills that teach the old forms; regenerate
  `docs/lf-reference.md`; classify the new `wave` arg in `wave_resolution_matrix`.
- Acceptance tests: shared setup via committed `config.yaml`, personal via `lf set`.
