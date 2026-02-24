# Review: Flatten Wave Model + Listen Stimulus

Branch: `jack-heart.chords.20260224_0921`

## What was implemented

Replaced the recursive Wave enum (Voice | Chord) with a flat struct and added inter-wave communication via `StimulusKind::Listen`.

**Removed:**
- `wave_type`, `parent_wave_id`, `position` columns from waves
- Tree-specific store operations (join/leave, child listing, chord assembly)
- `/v0/waves/join` and `/v0/waves/leave` HTTP routes
- Python `join()`/`leave()` client methods
- ~1300 lines of chord-tree machinery from Rust store/types/routes

**Added:**
- `StimulusKind::Listen = 5` with `source_wave_id` on `Stimulus`
- `chords` + `chord_members` tables (schema only — no CRUD yet)
- Migration 013: drops tree columns, creates chord tables, recreates stimuli table with nullable `cron` and `source_wave_id`
- Listen stimulus validation (prevents self-listening, requires source_wave_id)
- Python client `add_stimulus(kind="listen", source_wave_id=...)` + test
- `Listen` classified as auto stimulus (disabled on stop, re-enabled on run)

## Key choices

**Table recreation over ALTER TABLE** for migration 013. SQLite can't ALTER COLUMN, so we recreate the stimuli table to make `cron` nullable (was `NOT NULL DEFAULT ''`) and add `source_wave_id`. The `pending_activations` table is recreated too since it FKs into stimuli. Data is preserved via backup tables.

**Listen as a stimulus, not a step.** A listen stimulus fires when its source wave completes — it's just another trigger alongside loop/cron/watch/once. No special iteration-aware scheduling. This is simpler and more composable than the nested model.

**Schema-only chord tables.** Migration 013 creates `chords`/`chord_members` but no domain type, store ops, or API exists yet. Phase 02 builds the CRUD layer on top.

**No listen in config files yet.** `parse_schema_stimulus` handles once/loop/watch/cron but not listen. This is intentional — listen requires a `source_wave_id` which references another wave, making it API-first. Phase 03 addresses schema file support.

## How it fits together

```
Wave (flat struct) ──── Stimulus (kind + source_wave_id)
                           │
                     ┌─────┼─────┐
                     │     │     │
                   once  cron  listen ──→ source wave
                   loop  watch
```

Chords are a separate grouping concept: `chords` → `chord_members` → `waves`. They're orthogonal to stimuli. A wave can listen to any other wave regardless of chord membership.

## Risks and bottlenecks

- **Migration 013 is destructive.** Drops columns and recreates tables. Existing databases must run migration before running new code. No rollback path.
- **Listen trigger is fire-and-forget.** The listening wave doesn't know *what* its source did (no PR content or diff injection yet). Phase 03 addresses this.
- **Chord tables are empty scaffolding.** Until Phase 02 ships, the tables exist but aren't used. This is fine — they don't affect existing functionality.

## What's not included

- **Chord CRUD** (Phase 02): domain type, store ops, HTTP routes, Python client
- **Listen in schema files** (Phase 03): `parse_schema_stimulus` doesn't handle `listen` kind yet
- **Source context injection** (Phase 03): listening wave gets no context about what the source did
- **Sidecar → listening terminology cleanup** (Phase 03): some display strings may still use old names
- **Nested chord orchestration** (Symphonia/studio): inherited triggers, child scheduling, beat-grid execution

## Test results

- Rust: all tests pass, `cargo fmt` clean, `cargo clippy` clean
- Python: 38 tests pass
- Key new tests: `listen_stimulus_kind_storage_value_is_stable`, `listen_stimulus_is_auto_stimulus`, `add_stimulus_with_listen_source_sends_correct_body`

## Note for Phase 02

`wave/chords/02-chord-crud.md` references "migration 012" in two places — the actual migration is 013. The numbering shifted when intermediate migrations (011, 012) were added before the chord tree removal.
