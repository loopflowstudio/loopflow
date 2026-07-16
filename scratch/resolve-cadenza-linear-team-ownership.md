# W2-166 — Resolve Cadenza Linear team ownership (PR 2: fail-closed creation)

Task Session `ts_cb980c1479b34d47b5e5f02205f08091`. Serial PRs, one branch each.
This is the second code PR. It carries the design's **spine: creation fails
closed** — the one remaining ownership-policy change that makes "creating work
through Loopflow cannot silently attach it to a foreign team" true.

## Reconciliation of merged work against the design (2026-07-16)

The design's `## End-to-end proof` and `## PR sequence` items, checked against
what has actually landed on `main`:

| Design item | Where | State |
|---|---|---|
| `projectUpdate(teamIds:)` + `move_project_to_team` | `pm/linear.rs` (#930) | ✅ shipped, unit-tested (`move_project_to_team_sets_the_team_ids`) |
| Select Project `teams` in the initiative-projects read | `pm/linear.rs` (#907) | ✅ `LIST_INITIATIVE_PROJECTS_QUERY` selects `teams { nodes { id } }` |
| `PmProject.team_ids` field + Option decode | `pm/mod.rs` (#907) | ✅ present |
| `reteam` moves the Project ahead of issues | `ops/pm.rs::pm_reteam` (#930) | ✅ `move_project_to_team(&pm.id, …)` before issue moves |
| `doctor` flags a foreign-team Project | `ops/pm.rs::project_off_team` + `pm_sync` (#907) | ✅ pure classifier, unit-tested |
| Demote same-repo "waves share a team" from defect to allowed | `pm_sync` (#907) | ✅ diagnostic dropped |
| **Remove `config.linear.team`/default fallback from creation (fail closed)** | `ops/pm.rs` | ❌ **this PR** |
| **Bind the loopflow product waves to their team before removing the fallback (coupled)** | `wave/*/GOAL.md` | ❌ **this PR** |
| Live Cadenza inventory + dry-run + apply | operational, Cadenza checkout | ⏸ deferred — PR 3, needs a Linear-connected, human-gated apply; the design forbids moving live Cadenza data during the code phase |

Everything except fail-closed creation (and its coupled precondition) is on
`main`. PR #910's Project-move attempt was superseded by #930 and is closed.

## What this PR does

**Fail-closed creation.** Reads stay team-agnostic (an unbound wave still syncs
its existing issues via `resolve_team`'s fallback), but *creation* now requires
an explicit `pm.linear_team`:

- New `require_creation_team(repo, wave, provider) -> OpsResult<String>` returns
  the wave's bound team or errors with the exact recovery
  `lf pm init --wave <w> --team-key <KEY>` and performs **no** Linear side effect.
- Guarded at the four creation entry points, keyed on the create signal so
  updates/reads are untouched:
  - `pm_create_project_async` (always a create)
  - `pm_create_task_idempotent_async` (always a create)
  - `pm_update_async` when `options.id.is_none()` (task create, not update)
  - `pm_project_write_async` when `options.project.is_none()` (project create, not update)
- `resolve_team` keeps its `config.linear.team` fallback for the read path only,
  so the change is scoped exactly to creation.

**Coupled precondition — bind the loopflow waves.** `product`,
`infrastructure`, and `intelligence` are all unbound and today ride the
`config.linear.team` fallback (`60558c53-…`). Under fail-closed creation they
would start erroring, so this PR binds each to the **same team id they already
resolve to** — `pm.linear_team: 60558c53-…` in each `GOAL.md`. This is a
behavior-preserving, stable-ID adoption (no Linear mutation), exactly the
design's "adopt its id/key before the fallback is removed."

## Proof (this PR)

1. `resolve_team_prefers_wave_binding_over_config` already asserts an unbound
   wave resolves to `None` (no config in the temp repo). New unit test:
   `require_creation_team` returns the bound team for a bound wave and errors
   with the `lf pm init` recovery for an unbound wave.
2. `cargo test -p loopflow` (pm unit + reteam classification) green.
3. `cargo clippy --all-targets -- -D warnings`, `cargo fmt`.
4. Behavior preserved for loopflow's own waves: each now binds the id it
   already used, so `lf pm task/project create` in product/infra/intel keeps
   landing on `60558c53-…` — now explicitly, not by fallback.

## Out of scope (PR 3, deferred)

Live Cadenza inventory + `reteam --apply`. Operational, run from the Cadenza
checkout where the waves live, human-gated (moves live Linear data). Not a code
change in this worktree; its receipt is the migration record.
