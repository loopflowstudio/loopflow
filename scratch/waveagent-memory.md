---
requires: existing code
produces: code changes
---
Make `wave/<name>/` a two-file surface — `GOAL.md` (intent) + `MEMORY.md` (memory) —
and inject both into the wave loop's rendered context.

Full design context: `scratch/waveagent-sessions.md` (read the "Wave on disk: two
files, both injected" and object-model sections). This unit is the **foundation**
milestone from that doc, scoped small. Do not build dispatch-through-lfd, lfq, or the
object-model normalization here — only the two-file surface + injection.

## Goal

Today the wave's authored surface is `wave/<name>/goal.md` (lowercase). Rename it to
`GOAL.md` and add a sibling `MEMORY.md` that carries the wave's curated memory. Both
files render into the wave loop's prompt so the agent reads its intent and its
accumulated memory each iteration. `MEMORY.md` is the continuity substrate — the thing
that makes work compound.

No backwards compatibility for the lowercase `goal.md` name — migrate everything to
`GOAL.md` (CLAUDE.md: don't keep both). This is an internal repo file, not a published
API.

## Workflow

1. **Resolver rename `goal.md` → `GOAL.md`.**
   - `rust/loopflow/src/engine/flow.rs`: `find_goal_path` currently checks
     `wave/<name>/goal.md` as its first lookup — change to `wave/<name>/GOAL.md`.
     (grep for `"goal.md"` and `wave/` to find the exact spot; it was ~line 694.)
   - `rust/loopflow/src/lfd/http/routes/wave_config.rs`: `goal_path()` builds
     `repo.join("wave").join(name).join("goal.md")` (~line 109) — change to
     `GOAL.md`. `read_wave_config` reads it; keep that behavior.
   - Update the override tests in `flow.rs` (`load_goal_prefers_wave_goal_md`,
     `load_goal_ignores_legacy_goal_paths`, etc.) to the new filename.

2. **Add `MEMORY.md` read + render into the loop context.**
   - `rust/loopflow/src/engine/flow.rs`: add a `memory: String` field to
     `GoalRenderContext` (alongside `flows`, `roadmap`, `metrics`). In `render_goal`,
     render it as a `<lf:wave-memory>\n{memory}\n</lf:wave-memory>` section
     (omit/placeholder cleanly when empty, mirroring how `metrics`/`flows` handle
     empty). Keep `render_goal` pure — it only formats what the context gives it.
   - `rust/loopflow/src/lfd/executor/wave/mod.rs`: `build_wave_run_command`
     (~line 128) builds the `GoalRenderContext`. Read `wave/<name>/MEMORY.md` from the
     run's worktree (return empty string if the file is absent — absence is normal,
     not an error) and pass it as `memory`. Add a small helper
     `read_wave_memory(repo: &Path, wave_name: &str) -> String`.

3. **Create a default `MEMORY.md` on wave creation.**
   - Find where wave creation writes `wave/<name>/goal.md` (grep `render_goal_md` /
     `goal_value_from_content` in `wave_config.rs`, and the wave-create route in
     `routes/waves.rs`). When a wave is created and the file is written, also write a
     starter `wave/<name>/MEMORY.md` if it does not already exist. Starter content:
     a one-line H1 title plus a short placeholder, e.g.:
     ```
     # <name> — wave memory

     Curated memory for this wave: roadmap progress, decisions, and learnings.
     Kept bounded — summarize and prune rather than append without limit.
     ```
     Do not overwrite an existing `MEMORY.md`.

4. **Migrate the repo's own goals wave.**
   - `wave/goals/goal.md` → `wave/goals/GOAL.md` (git mv; preserve content).
   - Add `wave/goals/MEMORY.md` with a real short memory seed for the goals wave
     (a few lines: what this wave is steering, current state). Keep it terse.

5. **Tests.**
   - Resolver test: a wave with `wave/<name>/GOAL.md` resolves; a lowercase
     `goal.md` no longer resolves (update existing override tests).
   - `render_goal` test: given a `GoalRenderContext` with non-empty `memory`, the
     output contains the `<lf:wave-memory>` section with that content; empty memory
     renders cleanly.
   - A `build_wave_run_command`-level test (or a `read_wave_memory` unit test) showing
     `MEMORY.md` content reaches the rendered prompt.
   - Update any DTO/fixture round-trip touched. Do NOT add a `memory` column to the
     `Wave` record or DTO — memory is a file, read at render time, never a wire field.

## What matters

- `GOAL.md` + `MEMORY.md` are files under `wave/<name>/`, read at render time. No new
  DB columns, no wire-DTO fields for goal body or memory.
- `render_goal` stays pure; the executor does the file reads.
- Absence of `MEMORY.md` is normal (empty string), not an error.

## Guardrails

- Scope: only the two-file surface + injection into the goal render. Nothing about
  dispatching subagents, lfq, Attention, or renaming `WaveRun`.
- No `goal.md` lowercase compatibility shim.
- `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test --all` must pass. If you
  touch Python/Swift fixtures, run their tests too (see TESTING.md).

## Output

Working code with tests, committed. The wave loop's rendered prompt now carries the
wave's `GOAL.md` body and `MEMORY.md` memory.
