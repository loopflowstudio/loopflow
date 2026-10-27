# LOO-636: Remove abandoned store-copy snapshots

Status: implemented on this branch (3a83a46532, 44ae0958ef); plan written after the fact
on 2026-10-27 to record the decisions for gate and review.

## Problem

On 2026-10-26 Jack's `~/.lf/copies/` held 9 orphaned `.tmp*` snapshot dirs (each
~10–11 GB apparent, WALs up to 1.6 GB) with the disk at 93%. `store::copy::create`
stages a snapshot in a `tempfile` dir; a killed process never runs its `Drop`, and
nothing else removed them. Every hotfix repro clone needs that headroom.

## Demo

```
lf <draft build> …            # SIGKILL it mid-snapshot → copies/.tmpXXXX remains
lf disk clean --dry-run       # lists "abandoned snapshot" with its real freed bytes
lf disk clean                 # removes it, however recent; logs to logs/disk.log
```

The next draft build's `create` also clears staging dirs before it snapshots.

## Chosen approach

- **Ownership by lock, not age.** `create` already holds `copies/.lock` from staging to
  rename. `copy::idle_lock` try-locks it; when free, every `copies/.tmp*` belongs to a
  dead process. The disk sweep (`lf disk clean` and the periodic `rotate_when_due`)
  removes them without the 24 h grace; while a snapshot runs, it skips them.
  `create` itself clears them first, since it already holds the lock.
- **Named prefix.** Staging uses `tempfile::Builder` with prefix `.tmp` (also
  tempfile's default, so pre-existing orphans match). Copy-age rotation excludes
  staging dirs so they're counted once, under reason "abandoned snapshot".
- **Freed bytes are real.** `disk::allocated_bytes` reads APFS
  `ATTR_CMNEXT_PRIVATESIZE` per file (bytes a delete frees now) via `getattrlist`;
  falls back to `st_blocks` when unavailable and on Linux.
- Failure paths (snapshot/sanitize error) already dropped the `TempDir`; rename then
  `keep()` preserves that until the rename succeeds.

## Alternatives rejected

- PID files in staging dirs: a second ownership record beside the lock that
  already exists; PID reuse makes it weaker.
- Age-only grace: a 10 GB orphan would sit for a day, the exact headroom problem.
- Signal handlers to clean on SIGINT/SIGTERM: miss SIGKILL/crash; the sweep covers all.

## Risks and findings

- `PRIVATESIZE` applies to all Home/worktree measurements: the live DB's footprint now
  excludes blocks it shares with copies. Accepted (see `questions.md`).
- A fork-inherited flock held by a child briefly only delays a sweep; never unsafe.
- Live `~/.lf/copies` holds no `.tmp*` today (3 copy stores only), so the demo needs a
  fresh interrupted snapshot.

## Scope

Exclusions: draft-copy stores themselves (existing age rotation owns them), the
database-locked fix (db678e), LOO-433.

## Done when

- A killed snapshot's staging dir is gone after the next `lf disk clean` or draft
  build, regardless of age, and the report shows its private (APFS) bytes.
- A staging dir is never removed while a snapshot holds `copies/.lock`.
- Gate: `cargo test -p loopflow --lib -- disk:: store::copy` passes;
  fmt and `clippy --all-targets -- -D warnings` pass.

Compress 2026-10-27: rotation's removal/log bookkeeping is one `disk::Sweep::remove`, shared by
age rotation and `remove_abandoned_snapshots`; `copy::is_staging` names the prefix test.

Realign 2026-10-27: code matches every claim above; origin/main has not moved past the
branch's merge base. Remaining: gate, then the demo against a SIGKILLed snapshot.

Proposed infrastructure Wave memory (not written; realign ran outside a memory-curating
Graph, and the memory sits at 15687/16000 tokens): "Disk measurements on APFS count
`ATTR_CMNEXT_PRIVATESIZE`, so clones report only what removal frees; prove a staging
dir abandoned by the lock its writer holds, not by age or PID files."

Check 2026-10-27 (compress): `cargo test -p loopflow --lib -- disk:: store::copy` 9 pass; clippy clean; fmt and affected suites owned by gate.

Concept review 2026-10-27: no change. The user-facing concept is one new removal reason,
"abandoned snapshot", in the existing `lf disk clean` report and `disk.log`; ownership rides
the lock `create` already holds, so no new record or command. Considered and kept:
`create`'s own pre-snapshot clearing is unlogged (garbage by construction, and it buys the
headroom before the next sweep is due); private-size measurement for all categories answers
"what does clean free", the question `lf disk` exists for. No open product choice.
