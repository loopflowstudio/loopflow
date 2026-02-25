# 04: lfd-Managed Direction Aliases

User-defined direction aliases stored in lfd's sqlite. Pure expansion — no markdown content.

`designer` → `[ux, craft, aesthetics]`. Managed via HTTP API + lfq wrappers. Not supported in `lf` standalone.

## Why this phase exists

The direction taxonomy restructuring decomposed roles into orthogonal directions. But users still think in terms like "designer" or "security-reviewer" — composite perspectives that map to a specific set of directions.

Repo-level groups (`.lf/directions/<group>/`) solve this for project-specific presets. But personal presets ("my definition of designer") are user state, not repo state. They should live in lfd.

## Scope

### In scope

1. **sqlite schema**
   - `direction_aliases` table: `name TEXT PRIMARY KEY, directions TEXT NOT NULL` (JSON array of direction names).
   - Standard CRUD. No versioning, no history.
2. **HTTP API**
   - `GET /direction-aliases` — list all.
   - `GET /direction-aliases/:name` — get one.
   - `PUT /direction-aliases/:name` — create or update. Body: `{ "directions": ["ux", "craft", "aesthetics"] }`.
   - `DELETE /direction-aliases/:name` — remove.
3. **lfq CLI wrappers**
   - `lfq direction create <name> -d <directions...>`
   - `lfq direction list`
   - `lfq direction delete <name>`
4. **Resolution integration**
   - Wave execution in lfd resolves direction aliases before passing to the engine's `expand_direction_names()`.
   - Resolution order: lfd aliases → repo groups → builtin groups → standalone pass-through.
   - Aliases expand one level (no recursive alias resolution). The engine's `expand_direction_names()` already handles recursive BFS expansion with dedup for builtin/repo groups — lfd aliases just prepend one resolution layer.
   - Integration point: call `expand_direction_names()` in `flow.rs` already serves `prompt.rs`, `fork.rs`, and `lf/commands/flow.rs`. lfd alias resolution happens upstream in the executor before reaching this function.

### Out of scope

- `lf` CLI support (lfd-only feature)
- Direction aliases with their own markdown content
- Recursive alias expansion (alias pointing to another alias)
- Concerto UI for managing aliases (can come later)
- Sharing/exporting aliases between users

## Contract

- Aliases are expansion-only: they resolve to a list of direction names, nothing more.
- An alias that shadows a builtin group name wins (lfd aliases take precedence).
- Deleting an alias referenced by a running wave doesn't affect in-flight runs (snapshot already expanded).
- Invalid direction names in an alias are passed through — validation happens at load time, not at alias creation.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy -p loopflow --all-targets -- -D warnings`
- `cargo test -p loopflow direction`
- `uv run pytest tests/e2e/test_api_smoke.py -v`
- Manual: `lfq direction create designer -d ux,craft,aesthetics && lfq direction list`

## Done when

- `lfq direction create designer -d ux,craft,aesthetics` persists to sqlite.
- A wave with `direction: [designer]` expands to `[ux, craft, aesthetics]` member directions during execution.
- `lfq direction list` shows all aliases with their expansions.
- Aliases take precedence over builtin groups of the same name.
