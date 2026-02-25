# Direction Taxonomy Restructuring

## Status: Complete

Replaced role-based directions (`infra-engineer`, `designer`, `product-engineer`, `ceo.md`, `values/`) with composable quality-focused direction groups organized by concern.

## Taxonomy

| Group | Members | Focus |
|-------|---------|-------|
| `infra/` | security, performance, reliability, observability | System qualities |
| `ux/` | visibility, feedback, consistency, affordance, error-prevention, accessibility, dynamics, aesthetics | User experience heuristics |
| `craft/` | care, clarity, simplicity, scale | Building things right |
| `creativity/` | alive, living, musical | Momentum and feel |
| `ceo/` | focus, immediacy, truth | Strategic judgment |

`-d craft` expands to all members; `-d clarity` works standalone. Compose freely via `-d ux,craft`.

## Key decisions

**Groups over roles.** Old directions coupled concerns (designer = ux + craft + aesthetics). New directions are orthogonal.

**No compatibility aliases.** `infra-engineer`, `designer`, `product-engineer`, `values` are gone. Users get a clear error. No external consumers.

**Build-time codegen for group membership.** `build.rs` scans `directions/*/` at compile time and generates a `LazyLock<HashMap>`. No runtime filesystem scanning for builtins.

**User groups override builtins.** `.lf/directions/craft/` takes precedence over the builtin `craft/` group.

## How it fits together

`expand_direction_names()` in `flow.rs` is the single expansion point. It checks user groups first, then builtin groups, then passes through unrecognized names as standalone directions. Both `prompt.rs` (context gathering) and `fork.rs` (fork execution) call this before loading direction content.

The build script generates `BUILTIN_DIRECTION_GROUPS` as a `LazyLock<HashMap><&str, &[&str]>` compiled into the binary. Discovery (`lf list`) reads the same map.

## What changed from the original branch

1. **`ceo.md` → `ceo/` group** — decomposed into immediacy (speed, bias to action), focus (kill things, errors of omission), truth (contrarian thinking, raise the ceiling)

2. **`values/` → `craft/` + `creativity/`** — craft covers building things right (care, clarity, simplicity, scale); creativity covers momentum and feel (alive, musical)

## Risks noted

- **No migration path.** Users with `direction: values` in config will get an error. Intentional.
- **Fork flow defaults are arbitrary.** `wave-reduce` forks across `infra`, `ux`, `ceo`. Demo split, not principled.
- **21 direction files.** More surface area than the old 4 roles. Maintenance cost increases.

## Test results

All suites pass: `cargo fmt`, `cargo clippy`, `cargo test` (60 tests including flow, context, discovery, golden), `uv run pytest python/tests/` (47 tests).

## Gate fixes applied

- Fixed stale `values` reference in `docs/config.md`
- Fixed stale `values` references in `PROMPT_STYLE.md`
