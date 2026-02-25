# Direction Taxonomy Restructuring — Review

## What was implemented

Replaced 4 role-based directions (`infra-engineer`, `designer`, `product-engineer`, `ceo.md`) and 3 value directions (`values/craft`, `values/flow`, `values/scale`) with 21 composable quality-focused directions organized into 5 groups:

- `infra/` — security, performance, reliability, observability
- `ux/` — visibility, feedback, consistency, affordance, error-prevention, accessibility, dynamics, aesthetics
- `craft/` — care, clarity, simplicity, scale
- `creativity/` — alive, living, musical
- `ceo/` — focus, immediacy, truth

Groups expand at runtime: `-d craft` becomes `[care, clarity, simplicity, scale]`. Individual directions work standalone: `-d clarity`. Compose freely: `-d ux,craft`.

Added `wave/infra/` with architecture report and 4-pass roadmap (core boundary cleanup, contract hardening, orchestration expansion, direction aliases).

## Key choices

**Build-time codegen for groups.** `build.rs` scans `directions/*/` and generates `BUILTIN_DIRECTION_GROUPS` as a `LazyLock<HashMap>`. No runtime filesystem scanning for builtins. Same pattern already proven for steps/flows.

**Single expansion point.** `expand_direction_names()` in `flow.rs` handles all group resolution — user groups first, builtin groups, then pass-through. Called from `prompt.rs`, `lf/commands/flow.rs`, and `lfd/executor/wave/fork.rs`.

**Recursive expansion with dedup.** Uses a queue (BFS) so a user group containing `craft` recursively expands to craft's members. `HashSet` prevents duplicates while preserving insertion order.

**No compatibility aliases.** Old direction names are gone. Clean break — no external consumers, no migration shim.

**Fork flows use `infra`, `ux`, `ceo` directions.** `wave-reduce`, `wave-polish`, `wave-expand` all fork with the same 3-direction split. Arbitrary but reasonable demo defaults.

## How it fits together

```
build.rs
  └─ scans directions/*/ → generates BUILTIN_DIRECTION_GROUPS

expand_direction_names(names, repo)
  └─ resolve_direction_group(name, repo)
       ├─ user: .lf/directions/{name}/ (markdown stems)
       └─ builtin: BUILTIN_DIRECTION_GROUPS[name]

prompt.rs::gather_context()  ──┐
lf/commands/flow.rs::run_fork() ──┤── all call expand_direction_names()
lfd/executor/wave/fork.rs ────┘

discovery.rs::list_directions() ── merges builtin_direction_names() + builtin_direction_group_names() + user
```

## Risks and bottlenecks

- **No migration path.** Users with `direction: values` or `direction: infra-engineer` in config will get a `DirectionNotFound` error. Intentional — no external consumers.
- **21 direction files.** More surface area than the old 4 roles. Each is small (7-10 lines) but maintenance cost increases.
- **Fork defaults are arbitrary.** `wave-reduce` forks across `infra`, `ux`, `ceo`. Works for demo but not principled.
- **Docker tests fail locally** — pre-existing, unrelated to this branch. They require `/var/run/docker.sock`.

## What's not included

- Direction aliases in lfd (designed in `wave/infra/04-lfd-direction-aliases.md`, not implemented)
- Migration path from old direction names
- Concerto UI for browsing/selecting direction groups
- Per-step direction group inheritance

## Test coverage

All pass locally:
- `cargo fmt --check` — clean
- `cargo clippy -- -D warnings` — clean
- `cargo test --all` — 435 pass, 2 fail (pre-existing docker socket tests)
- `uv run pytest python/tests/` — 47 pass
- Golden prompt test (`with_direction_group`) validates end-to-end prompt output for group expansion

New tests added:
- `flow.rs`: 7 unit tests for `expand_direction_names` (pass-through, group expansion, user override, dedup, recursive)
- `context_tests.rs`: 2 integration tests for direction group expansion through full context gathering
- `discovery_tests.rs`: direction discovery includes groups and nested members
- `flow_tests.rs`: `builtin_wave_reduce_expands_publish_subflow` validates fork direction assignment

## Gate fixes applied

- Fixed stale `values` reference in `docs/config.md`
- Fixed stale `values` references in `PROMPT_STYLE.md`
- Fixed stale `flow` and `scale` preview data in `DirectionTypeahead.swift`
