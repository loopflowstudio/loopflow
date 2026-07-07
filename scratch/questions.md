# Open questions / assumptions — goal-md-research

## Rebase onto origin/main (done)

Rebased `goal-md-research` onto `origin/main` and force-pushed a clean branch.
Build + `cargo clippy --all-targets` + lfdb/waves/dto tests all green.

**The conflict, in one line:** the branch removes `primary_flow` from the wave
model; main independently **collapsed the wave model from multi-repo to
single-repo** (migrations `051_drop_dead_tables`, `052_wave_single_repo`). Every
conflict was the intersection of those two edits. Resolution rule applied
throughout: **keep main's single-repo shape, drop `primary_flow`.**

Concrete resolutions worth knowing:

- **`bridge.rs`** — main deleted the whole file; branch only tweaked a test in
  it. Accepted the deletion.
- **Migration renumber** — branch's `051_drop_wave_primary_flow.sql` collided
  with main's new `051`/`052`. Renamed to **`053_drop_wave_primary_flow.sql`**
  (runs after `052` adds the single-repo columns; the column still exists to be
  dropped). Updated `ALL_MIGRATIONS` and the `RENAME_CONVERGENCE_MIGRATIONS`
  tolerance list to `053_...`.
- **`catalog.rs` wave queries** — took main's single-repo SELECT/INSERT
  (`repo, worktree, branch, status, iteration, cycle_start_iteration` inline),
  stripped `primary_flow`; INSERT dropped from 17→16 params, renumbered.
- **`rows.rs::map_wave_row`** — column indices shifted down by one after
  removing `primary_flow` (goal=7 … cycle_start_iteration=15). Kept main's
  legacy-NULL goal fallback. Verified against `sqlite.rs::upsert_wave` bind
  order (16 params, matches).
- **`WaveSnapshot`** (`lf/commands/waves.rs`) — had a `primary_flow` field
  (never read, only constructed). Removed field + 3 constructions. This was not
  in the conflict set; caught by the compiler after `--continue`, folded in as a
  fixup to the implement commit.
- **`LocalWaveService.swift`** — main's side was empty (it builds the single-repo
  `Wave` later at the `return Wave(repo:...)`); dropped the branch's stale
  multi-repo `Wave(repos:, flow:"")` block + its self-contained
  `parseRepoWorkFromJSON` helper.

## Pre-existing Swift WIP — preserved in `stash@{0}`, do NOT reapply

At session start the tree was dirty with in-progress work removing the **`flow`
field from the Swift `Wave` model** (`Wave.swift`, `WaveViewModel.swift`,
`LocalWaveService.swift`). It was **incomplete** (~40 call sites — views + tests
— still reference `Wave.flow`) and never committed.

I stashed it to do the rebase, then tried to reapply. **It no longer applies
cleanly and should not be force-reapplied:** the stash was authored against the
*old multi-repo* `Wave.swift`, so its diff would *reintroduce* the
`repos:`/`RepoWork` multi-repo initializer that main just collapsed away. I
aborted the reapply and left the branch clean; the WIP is safe in
**`stash@{0}`** (`git stash show -p stash@{0}`).

**To finish the Swift `flow` removal** (the Swift half of the `primary_flow`
migration in `goal-md-spec.md`), don't `stash pop` — reauthor against the
current single-repo `Wave.swift`: drop the `flow` stored property + its init
params, remove the `flow` computed passthrough in `WaveViewModel`, drop
`flow: json["primary_flow"]` in `LocalWaveService`, and fix the ~40 call sites
(most are `makeWave(flow:...)` test helpers). Small and mechanical, but a
distinct unit of work from this rebase.
