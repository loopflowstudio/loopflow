---
asana_id: '1213751585305659'
linear_id: 8d48e039-250f-4156-9c8f-b8a87aa8bff0
notion_id: 32af8f99-3d81-8121-b592-d35ee90a848d
---
# Ingest becomes PM-native

## Problem

`lf ingest` was written before PM integration existed. It still thinks locally: read `wave/<name>/`, pick a file, move it into `scratch/`. That made sense when the filesystem was the source of truth. It is the wrong mental model once a wave has a PM provider.

For PM-backed waves, the authoritative queue now lives in Asana/Linear/Notion. `lf ingest` should not behave like a local-file picker that sometimes gets prefreshed by a flow. It should behave like a PM-native pickup step whose local files are the materialized view of the tracker.

The stale-data bug is one symptom of that mismatch. The deeper issue is that ingest still belongs to the pre-PM world.

## Approach

Make `ingest()` PM-native by having it refresh provider-backed waves itself before it reads local wave files. The logic:

1. After resolving the wave name and `main_repo`, check `wave_pm_is_enabled(main_repo, wave)`.
2. If enabled, call `pm_pull(main_repo, PmPullOptions { wave }, progress)`.
3. If `pm_pull` fails, log a warning via `progress.warning()` and continue — the local wave directory is still usable.
4. Proceed with existing `list_wave_items` → `select_wave_item` → move-to-scratch logic unchanged.

This is ~10 lines of new code in `ingest()`. No new functions, no new modules, no new abstractions.

### Why inside `ingest` rather than only in flows

Because this is not really a flow bug. It is an ingest responsibility bug.

The `build-or-silent` flow already chains `op: pm pull` → `ingest`, but that only papers over the mismatch:
- Manual `lf ingest` still behaves like the old local-only command.
- Any new flow that uses `ingest` would have to remember the PM prelude.
- The command that decides what work to pick would still not own the refresh that makes that decision trustworthy.

Putting the refresh inside `ingest` makes the command match the current product model: for PM-backed waves, ingest starts from PM. Flows can still call it, but they no longer need to make it correct first.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Only rely on flows to chain `pm pull` before `ingest` | Zero code change | Manual `lf ingest` stays broken. Every new flow must remember the pattern. |
| Add a `--refresh` flag, default off | Explicit control | Wrong default — stale data should be the exception, not the rule. |
| Add a `--no-refresh` flag, default on | Opt-out escape hatch | Premature. Add it if someone needs offline ingest. |
| Separate `pm-ingest` command | Clear separation | Fragments the UX. Users shouldn't need to know which ingest to call. |

## Key decisions

**PM-backed ingest starts from PM.** The local `wave/<name>/` directory remains the mechanism ingest reads from, but for provider-backed waves it is treated as a refreshed local mirror, not the source of truth.

**Pull uses `main_repo` path.** Wave directories live in the main repo, not worktrees. `pm_pull` resolves its wave dir via `require_wave_dir`, which needs the main repo root. Ingest already computes `main_repo` — pass that to `pm_pull`.

**Refresh is part of pickup, not a separate concern.** The command that chooses work is responsible for seeing current PM state before it chooses.

**Warn on failure, don't block.** A network blip or missing credentials shouldn't prevent picking work from the local roadmap. The warning surfaces the issue without halting the pipeline.

**No deduplication with flow-level `pm pull`.** When `build-or-silent` runs `op: pm pull` then `ingest`, the pull happens twice. This is fine — `pm_pull` is idempotent (remote-wins overwrite) and the second call is fast. Correctness over cleverness.

**Existing tests unchanged.** Current ingest tests don't set up wave PM config, so `wave_pm_is_enabled` returns false and the new code path is skipped. New tests specifically cover the refresh path.

## Scope

- In scope: call `pm_pull` from `ingest` when PM is configured, warn on failure
- In scope: tests for the refresh-then-pick path and the warn-on-failure path
- Out of scope: `--no-refresh` flag, offline mode, priority bucket mapping changes
- Out of scope: stable run/item identity (item 06)
- Out of scope: changing the `build-or-silent` flow (the redundant pull is acceptable)

## Done when

- `cargo test -p loopflow ingest` passes with new tests covering:
  - ingest with PM enabled refreshes before picking
  - ingest with PM enabled but pull failure warns and continues
  - ingest without PM config behaves exactly as before (existing tests)
- `cargo clippy -- -D warnings` clean
- Manual verification: `lf ingest` on a wave with `pm` block calls `pm_pull` (visible in progress output)
- A new item added remotely appears as the next pick
- A reprioritized item changes pick order
- A deleted remote item is no longer eligible
