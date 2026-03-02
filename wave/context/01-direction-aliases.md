# 01: Direction Aliases

**Finish line:** `lfq direction create designer -d ux,craft,aesthetics` works. Waves using `designer` expand it automatically.

Personal direction aliases stored in lfd's sqlite. Absorbs `wave/backlog/04-lfd-direction-aliases.md`.

## What to build

User-defined direction aliases that expand to sets of directions. `designer` -> `[ux, craft, aesthetics]`. Managed via HTTP API + lfq CLI. lfd-only (personal state, not repo state).

## Scope

### In scope

1. **sqlite schema**: `direction_aliases` table — `name TEXT PRIMARY KEY, directions TEXT NOT NULL` (JSON array).
2. **HTTP API**:
   - `GET /direction-aliases` — list all
   - `GET /direction-aliases/:name` — get one
   - `PUT /direction-aliases/:name` — create or update. Body: `{ "directions": ["ux", "craft", "aesthetics"] }`
   - `DELETE /direction-aliases/:name` — remove
3. **lfq CLI**:
   - `lfq direction create <name> -d <directions...>`
   - `lfq direction list`
   - `lfq direction delete <name>`
4. **Resolution integration**: lfd resolves aliases before `expand_direction_names()`. Order: lfd aliases -> repo groups -> builtin groups -> pass-through.

### Out of scope

- `lf` CLI support (lfd-only)
- Aliases with their own markdown content
- Recursive alias expansion
- Concerto UI for managing aliases
- Sharing/exporting aliases

## Contract

- Aliases are expansion-only: resolve to direction names, nothing more.
- Alias shadowing a builtin group wins (lfd aliases take precedence).
- Deleting an alias doesn't affect in-flight runs (already expanded).
- Invalid direction names pass through — validation at load time, not creation.

## Done when

- `lfq direction create designer -d ux,craft,aesthetics` persists to sqlite
- A wave with `direction: [designer]` expands to member directions during execution
- `lfq direction list` shows aliases with expansions
- Aliases take precedence over builtin groups of the same name
- `cargo test -p loopflow direction` passes
- `uv run pytest tests/e2e/test_api_smoke.py -v` passes
