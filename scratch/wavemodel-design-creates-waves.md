# Design Creates Waves + Split-Wave

## Objective

Make `lf design` the direct wave creation path. Add `split-wave` to decompose oversized waves. Simplify wave data model — YAML on disk is the source of truth, no schema abstraction.

## Current state

Implemented:

- `design` wave-plan path creates `wave/<name>/` directly (README, YAML, roadmap files, scratch doc)
- `design` implement path exits with `lf implement` guidance
- `split-wave` ops step: mitosis semantics, parent fully consumed
- Wave schema abstraction removed: no built-in waves, no schema discovery/resolution, no `schema_ref`/`schema_name` in database
- `name` field removed from wave YAML — directory name is canonical
- `read_wave_config()` reads YAML once at creation time

## Decisions

- Wave YAML on disk is the source of truth. `lfd` reads it at creation, doesn't store a separate copy.
- `lfd` tracks which waves it's operating, not wave state. Config updates write back to YAML.
- `split-wave` is mitosis: parent deleted, children replace it. Default 2, argument overrides.
- Roadmap items move as-is; Vision/Goals/Risks/Metrics are rewritten per child.
- `split-wave` is non-interactive (ops step); review the result afterward.

## Future direction

- `lfd` becomes fully stateless for wave config — `flow`, `direction`, `area` columns on `waves` table are runtime-only, not persistent store
- Import from YAML or default configs for wave creation
- Config updates via API write back to `wave/<name>/<name>.yaml`

## Remaining risks

- Prompt-driven file creation quality depends on agent adherence to wave conventions

## Out of scope

- Making `lfd` fully stateless (future branch)
- Automated split-wave without human confirmation
- Changes to `add-to-wave` or `wave-plan`
