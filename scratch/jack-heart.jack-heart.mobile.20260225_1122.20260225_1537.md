# wave_name: reverse branch naming for worktrees

## Scope

Normalize worktree naming around the wave identity (`{name}`) instead of full branch metadata, and treat absolute branch input as an existing branch when it already exists on `origin`.

## Current behavior

For default schema `{user}.{name}.{timestamp}.{words}`:

- `lf ops wt create jack-heart.mobile.20260225_1122`
  - resolves `wave_name = mobile`
  - creates worktree at `../loopflow.mobile`
  - checks out `jack-heart.mobile.20260225_1122` tracking `origin/jack-heart.mobile.20260225_1122` when local branch is missing

- `lf ops wt create mobile`
  - creates a schema branch as before
  - still places worktree at `../loopflow.mobile`

## Design decisions

- `parse_branch_name(branch, config)` reverse-parses schema strings using regex patterns per placeholder.
- `wave_name(branch, config)` is a pure string-level wrapper around `parse_branch_name`.
- Only trailing `{words}` is optional during reverse parsing. `{timestamp}`/`{date}` remain required when present in schema to avoid misparsing arbitrary dotted names.
- Worktree path resolution is schema-aware through `worktree_path_with_config(...)` and falls back to filesystem sanitization when reverse parsing fails.
- `create_with_schema(...)` checks `origin/<input>` first; if present, it adds a worktree for that branch instead of minting a new branch.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow parse_branch_name`
- `cargo test -p loopflow wave_name`
- `cargo test -p loopflow create_with_schema_uses_existing_remote_branch_and_wave_worktree_name`

Note: `cargo test --all` in this environment still reports unrelated docker-socket failures (`/var/run/docker.sock`).

## Follow-ups

- Keep Concerto wave-name input validation strict (`[a-z][a-z0-9-]*`) so branch parsing stays unambiguous.

## Next: drop {words} from schema, "Design a Wave" NUX

### Drop `{words}` from branch schema

Default schema becomes `{user}.{name}.{timestamp}`. The words concept moves out of branch naming entirely — timestamp handles uniqueness, words handle memorability.

This simplifies `compile_schema`: no more optional trailing segments, no variant generation. One schema → one regex.

### "Design a Wave" NUX

Replace the current wave creation flow in Concerto with a single "Design a Wave" button.

**Flow:**

1. User hits "Design a Wave"
2. Concerto generates a random word-pair name (e.g. `aurora-fugue`) using the existing `MAGICAL × MUSICAL` lists
3. Wave is created with that placeholder name, branch becomes `jack-heart.aurora-fugue.20260225_1122`
4. `design` step runs — interactive session to figure out what you're actually building
5. `design` renames the wave to something meaningful (e.g. `mobile`), branch becomes `jack-heart.mobile.20260225_1122`

The random name gets you into flow immediately — no upfront naming ceremony. Design earns the real name.

### Changes needed

- **Schema**: change default from `{user}.{name}.{timestamp}.{words}` to `{user}.{name}.{timestamp}`
- **`compile_schema`**: delete optional-segment logic (no more `{words}` stripping)
- **`generate_word_pair`**: keep it, but it's now used only for initial wave name generation in Concerto
- **Concerto NUX**: "Design a Wave" button → random name → launches `design` step
- **`design` step**: add responsibility for renaming the wave once intent is clear (wave rename + branch rename + worktree move)
