# Direction Taxonomy Restructuring — Review

## What was implemented

Replaced role-based directions (`infra-engineer`, `designer`, `product-engineer`, `ceo.md`, `values/`) with composable quality-focused direction groups organized by concern:

| Group | Members | Focus |
|-------|---------|-------|
| `infra/` | security, performance, reliability, observability | System qualities |
| `ux/` | visibility, feedback, consistency, affordance, error-prevention, accessibility, dynamics, aesthetics | User experience heuristics |
| `craft/` | care, clarity, simplicity, scale | Building things right |
| `creativity/` | alive, musical | Momentum and feel |
| `ceo/` | focus, immediacy, truth | Strategic judgment |

Direction groups expand at two points: prompt context gathering (`gather_context`) and fork execution. `-d craft` expands to all members; `-d clarity` works standalone.

## Key choices

**Groups over roles.** Old directions coupled concerns (designer = ux + craft + aesthetics). New directions are orthogonal — compose them freely via `-d ux,craft`.

**No compatibility aliases.** `infra-engineer`, `designer`, `product-engineer`, `values` are gone. Users get a clear error. The old names had no external consumers.

**Build-time codegen for group membership.** `build.rs` scans `directions/*/` at compile time and generates a static map. No runtime filesystem scanning needed for builtins.

**User groups override builtins.** `.lf/directions/craft/` takes precedence over the builtin `craft/` group. Same expansion logic, user wins.

## How it fits together

`expand_direction_names()` in `flow.rs` is the single expansion point. It checks user groups first, then builtin groups, then passes through unrecognized names as standalone directions. Both `prompt.rs` (context gathering) and `fork.rs` (fork execution) call this before loading direction content.

The build script generates `BUILTIN_DIRECTION_GROUPS` as a `phf::Map<&str, &[&str]>` compiled into the binary. Discovery (`lf list`) reads the same map to show available groups.

## Risks and bottlenecks

- **No migration path.** Users with `direction: values` in `.lf/config.yaml` will get an error. This is intentional — the old names are not aliased.
- **Fork flow defaults are arbitrary.** `wave-reduce` forks across `infra`, `ux`, `ceo`. This is a demo split, not a principled one. Fine for now.
- **21 direction files.** More surface area than the old 4 roles. Quality of individual directions is strong but maintenance cost increases.

## What's not included

- No changes to the direction loading or rendering pipeline — only the taxonomy and expansion layer.
- No changes to Concerto's direction picker beyond updating the typeahead data source.
- No backwards-compat shims for old names. Intentional.

## Test results

| Suite | Result |
|-------|--------|
| `cargo fmt --check` | Pass |
| `cargo clippy -- -D warnings` | Pass |
| `cargo test` (flow, context, discovery, golden) | 60 tests pass |
| `cargo test -- engine::flow::tests` | 24 tests pass |
| `uv run pytest python/tests/` | 47 tests pass |

## Gate fixes applied

- Fixed stale `values` reference in `docs/config.md` (now lists `craft`, `creativity`, `ceo`)
- Fixed stale `values` references in `PROMPT_STYLE.md` (now uses `craft`)
