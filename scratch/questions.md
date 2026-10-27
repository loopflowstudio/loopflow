# LOO-625 open questions and assumptions

- Migration drops `waves.workflow`. Other repositories' selections were written by hand to their Wave config.yaml (Jack Heart, 2026-10-27); no cross-repository migration code.
- Wave setup reports the Tasks Machine; a capacity default (pool/on-demand) shows as
  this Machine with source `personal`, not the capacity name.
- Swift does not mirror `WaveDetailSnapshot.setup` (no UI uses it yet).
- Acceptance: one end-to-end `lf task run` proves a Wave's agent and account route;
  the second Wave and the no-Wave repository default are proven through the launch
  preview and `lf wave status`. Machine is proven through `lf wave status` only
  (test machines are local).
- `lf set` keys add `machine.tasks` and `machine.session` beside `machine` (both), so the
  separately tested Session/Tasks Machine split survives deleting `machine use --session/--tasks`.
- Sources: every `lf set` value for a Wave or repository (incl. machine and account) reports
  `personal`; Wave vs repository scope of a personal value is not shown. `--global` values rank
  with `~/.lf/config.yaml` (below repo config) and report `global`, matching account routes.
- `lf set workflow --global` and `lf set machine --global` are refused (no global Workflow or
  Machine default exists).
- `lf set account.<provider>` takes an ordered comma list (`work@,personal@`).
- `lf wave workflow show` now reports the effective selection (personal, Wave, repo); the
  roadmap `workflow` field still reads only the committed Wave file.
- `lf unset machine.session` does not move the Session; it runs where it is until set again.
- Every config load now opens the local store read-only to look for `lf set` values (git
  repository discovery only when any exist). Gate should watch command latency.
- After merging main (#1720 moved Task handoff to `lf task move --machine`), `lf machine use`
  had no form left, so it is deleted; Desktop's Task handoff calls `lf task move --machine`.
- `lf set agent|skill_agents.<skill> --global` and `~/.lf/config.yaml` both write the global
  layer; the store value outranks the file and both report `global`. Kept (Jack Heart's steer
  gives `--global` the same keys); if one way per layer should hold, either refuse those keys
  under `--global` like `workflow`/`machine`, or report `global` values with their path.
