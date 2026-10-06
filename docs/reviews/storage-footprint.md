# Storage footprint review · 2026-10-06

Jack Heart requested an account of Loopflow's disk use on a nearly full 1 TB
development machine (LOO-390): what can be reclaimed, why the database reached
several GB, what customers should expect, and what stops the accumulation.
Measurements are allocated bytes (`du`, `stat %b`) on one APFS volume, taken
16:00–16:45 PDT while about forty agent processes were writing. `df` deltas
therefore include unrelated concurrent writes. Categories overlap where noted
and are not additive with Jack's earlier inventory.

Three statuses are used throughout: **confirmed** (measured or read in source),
**fixed here** (source change with a passing test), **proposed** (not built).

## What was reclaimed

| Action | Evidence of disposability | Freed |
| --- | --- | ---: |
| Removed 11 `$TMPDIR/.tmp*/candidate.db` directories | Written only by install preflight (`install.rs`); each held one copy plus its journal; no open handle; 5–48 h old | 10.37 GiB (`du`), 10.38 GiB (`df`) |
| `cargo clean` in `loopflow.make-the-release-and-ci` and `loopflow.resume` | Merged branches, no process, no build since October 5; output regenerates | 17.5 GiB (cargo), 16.7 GiB (`df`) |

Free space moved from 191 GiB at first measurement to 231 GiB; about 12 GiB of
that change came from other processes.

Jack's two leads could not be measured. No Instruments recording near 78.7 GiB
exists on the volume; the largest are seven `instruments*.ktrace` files totalling
1.5 GiB in `$TMPDIR`. Twelve temporary database copies (11.7 GiB) existed, not
81 (101 GiB). Free space was already 191 GiB at the first read, so both were
probably removed before this review began. That is an inference; nothing here
proves who removed them or what they contained.

## Inventory

`~/.lf` held 65 GiB at the start.

| Contributor | Size | Owner and purpose | Live | Regenerable | Retention today |
| --- | ---: | --- | --- | --- | --- |
| Rust `target/` in 11 Loopflow worktrees | 118 GiB before, 100 GiB after | Cargo, one cache per worktree | 6 active | Yes, minutes each | None; survives merge |
| `~/.lf/accounts/codex/*` | 18.4 GiB | Codex: `sessions/` 9.5 GiB, `logs_2.sqlite`, `thread_history_1.sqlite`, `state_5.sqlite` 5.6 GiB, `packages` 1.9 GiB | Yes | No (native history) | None; Loopflow sets no Codex retention |
| `~/.lf/traces` | 17.6 GiB, 991 dirs | Pre-August capture model | No writer or reader in source; newest August 28 | No | None |
| `loopflow.db.backup-*` beside the store | 13.6 GiB, 48 files | One full copy per migrating release | No reader | No | None; **fixed here** |
| `~/.lf/bin` | 6.5 GiB, 165 binaries | Content-addressed CLI per install; 62 `lfd-*` have no current writer | 3–4 running | Published ones yes | None |
| `~/.lf-machine/install/artifacts` | 5.5 GiB, 62 app bundles | Rollback app copies | 1 selected | Published ones yes | None |
| `~/.lf-retired` | 4.6 GiB | Stores retired by hand on October 2–4 | No | No | Manual |
| `loopflow.db` + WAL | 2.5 + 1.4 GiB | Shared store | Yes | No | None |
| `~/.lf/backups`, `lfd.db*` | 3.4 + 1.1 GiB | Hand-made backups, retired daemon store | No writer in source | No | None |
| `~/.lf/runs` | 3.4 GiB, 2,057 captures | `events.jsonl` 2.8 GiB, `context.json`, `manifest.json` | Yes | No | None |
| `$TMPDIR` | 14.3 GiB before | Preflight copies 11.7 GiB, Instruments ktrace 1.5 GiB, 1Password, leaked test dirs | Mixed | Mostly | OS reboot only |
| `/private/tmp` | 16 GiB | `claude-501` session scratch 8.8 GiB (two 1.6 GiB store copies), LOO-304 evidence about 4 GiB | Mixed | No | OS reboot only |
| `~/.lf/logs` | 0.96 GiB, 9,385 files | Legacy per-repo prompt logs; two files written since October 2 | Barely | No | None |

Unrelated to Loopflow and left alone: OrbStack 34 GiB, Magic Arena 25.5 GiB,
CoreSimulator 23.5 GiB, Xcode 20 GiB, Steam 16 GiB, Kata builds, Messages.

Left intact because the evidence is incomplete:

- `~/.lf/traces`, `~/.lf/backups`, `lfd.db*`, `~/.lf-retired`, `~/.lf/logs`:
  unique history that no current code reads. Nothing shows it is unwanted.
- `~/.lf/bin` and install artifacts: rollback state referenced by
  `~/.lf-machine/install/active.json` and receipts; running processes execute
  several of them.
- `/tmp/loo304-*` and `$TMPDIR/instruments*.ktrace`: LOO-304's retained
  benchmark receipts and recordings.
- `/private/tmp/claude-501/*/scratchpad`: other Sessions' working copies, one
  open by a live process.
- Release state for v0.13.7 and its notarization receipt.
- The other nine worktree `target/` directories: a process or a build within the
  day. `lf wt prune --dry-run` names eleven worktrees but would also remove
  unmerged `stale` branches, so it was not run.

## Why the database is several GB

Table sizes from `dbstat` on the live store (2,598 MB, zero free pages):

| Object | MB |
| --- | ---: |
| `session_events` rows | 2,031 |
| Four `session_events` indexes | 371 |
| `execs` and its indexes | 158 |
| Everything else | 38 |

Migration backups date the growth: 303 MB on October 2 (half of it free pages),
1,032 MB on October 5, 2,274 MB on October 6 at 11:44, 2,598 MB by 16:16.

1. **Why is the store 2.6 GiB?** `session_events` and its indexes are 92% of it.
   1.06 M of its 1.09 M rows are `observed` rows mirroring `events.jsonl`.
2. **Why a million rows in four days?** Since the October 2 capture cutover the
   recorder writes every capture event to the file and to SQLite, one
   transaction each. 68% of the rows are streaming increments: 299 k derived
   `text_delta`, 299 k raw `item/agentMessage/delta`, 64 k raw `outputDelta`,
   62 k `item_updated`, 11 k cumulative diffs.
3. **Why does an increment cost so much?** A three-character token is stored
   twice, raw and derived, each inside a 300–500 byte envelope, with five index
   entries. The `session_driver_exit` index covers every `observed` row
   (114 MB) to answer a question about a few thousand.
4. **Why is the rest large?** Complete items are also stored raw and derived
   (raw `item/completed` 335 MB, derived 221 MB, raw `item/started` 157 MB).
   Each capture stores its prompt in `manifest.json` (137 MB) and again as
   `user_input` (135 MB). The codex turn diff is restated in full on every
   change (151 MB). All of it also sits in `runs/*/events.jsonl`, and the
   provider keeps its own transcript.
5. **Why did nothing bound it?** No table has retention, no connection limits
   the WAL, and nothing reported size. Every reader that needs these rows wants
   a reduction (final answer, usage, turn outcome, steer acknowledgement); only
   `lf session history --events` replays them verbatim.

Competing explanations, tested:

- *Unreclaimed pages.* False now: the free list is empty. True before
  October 2, when 39,893 of 77,602 pages were free.
- *WAL growth.* The 1.4 GiB WAL is real but did not change size across ten
  minutes of writes while the store grew 8 MB. It is a high-water mark that
  SQLite reuses and never truncates. **Fixed here.**
- *Excessive writes from polling.* Confirmed and already remedied elsewhere.
  Desktop ran `session list --json` and `monitor ps --json` about every four
  seconds, each recorded as an Exec: 57–68 k rows a day, 94% of all 275 k Execs.
  The rate fell from about 2,900 to 250 an hour at 11:18 on October 6 when the
  LOO-382 build was installed. Exec rows are 6% of the store.
- *Useful history.* Complete items, usage, manifests and turn outcomes are read
  and kept. Nothing reads an individual increment after its item completes.

The database also caused the temporary copies. `lf home install preflight`
copied the whole store to validate lifecycle references against a migrated
snapshot, on every preflight, including when no migration was pending. The
install currency probe kills preflight after 30 seconds
(`lf/commands/install/published.rs`), and interrupts exit without running
destructors. Copy sizes of 0.27–1.9 GiB match copies cut off part way. The
timeout as the killer is read from source and consistent with the sizes, not
observed directly. Release's v0.13.6 installation independently sampled the same
copy restarting under concurrent writers; PR #1465 (v0.13.7) pinned its snapshot
so it finishes. That made the copy complete, not unnecessary: a finished copy
still costs the store's size on every probe. Whether any stranded directory was
written by a build containing #1465 was not checked.

## What changed

| Fix | Effect | Proof |
| --- | --- | --- |
| History keeps what increments add up to. A run of deltas becomes one event at the first delta's position; a Turn keeps its last cumulative diff; raw increment notifications stay in `events.jsonl` only. | Replaying all 2,046 retained capture files through the same rule keeps 27.5% of rows and 72% of bytes (`scripts/benchmarks/storage/replay_history.py`). Write transactions fall by the same 72%. | `history_keeps_what_streamed_increments_add_up_to`; the existing streamed-prose recovery test still passes with the capture directory deleted |
| Preflight reads an exact-frontier store in place. Only a pending migration takes #1465's pinned snapshot. | No store copy on routine installs or currency probes; preflight no longer scales with store size. | `an_exact_store_is_validated_in_place` |
| Write connections set `journal_size_limit` to 64 MiB. | The WAL is truncated when a checkpoint resets it. Expected to return about 1.4 GiB here after install. | `a_burst_does_not_leave_its_wal_on_disk` |
| A committed migration keeps the two newest fingerprinted backups. | About 10.6 GiB returns at this machine's next migrating release. Hand-named and unfingerprinted backups are never touched. | `a_committed_migration_keeps_only_the_newest_backup_generations` |

Existing rows are not rewritten. The installed effect needs a release and is not
yet observed.

## Customer growth

Assumptions, from October 2–6 here: 1,227 captures (one agent launch or Flow
step) in 4.3 days. Per capture, before these fixes:

| Component | Mean | Median |
| --- | ---: | ---: |
| SQLite history and indexes | 2.0 MB | 1.0 MB |
| `events.jsonl` | 1.2 MB | 0.8 MB |
| `context.json` + `manifest.json` | 0.24 MB | — |
| Provider transcript and provider databases (Codex) | about 1.1 MB | — |
| **Total** | **about 4.6 MB** | |

A Task run through the `code` Flow stored a median 10 MB of history across about
ten captures; the ninetieth percentile was 63 MB and the largest 249 MB.

| Use | Captures/day | Per month | Per year | After these fixes |
| --- | ---: | ---: | ---: | ---: |
| Light: one small Task a day | 5 | 0.7 GB | 8 GB | about 7 GB |
| Heavy: several agents all day | 100 | 14 GB | 168 GB | about 148 GB |
| This machine | 285 | 39 GB | 480 GB | about 420 GB |

Fixed costs per release: 50 MB of CLI and about 90 MB of app bundle kept
forever, roughly 7 GB a year at weekly releases, plus one store copy per
migrating release before the backup fix. Before LOO-382, Desktop polling added
about 30 MB a day whether or not anyone worked.

Development-only costs, not customer costs: 118 GiB of Cargo output, benchmark
receipts, Instruments recordings, test temporary directories, and 165 locally
promoted binaries.

The dominant rate is per-capture history kept in three places with no expiry.
The fixes here shrink one of the three by about a third. Bounded growth needs
the retention decisions below.

## Producer audit and proposals

No Loopflow producer expires anything except per-minute cron receipts, the
Desktop cache and launch journal (both size-capped), and dead process receipts
through `lf monitor prune`. `lf home doctor` reports no sizes.

In priority order, all **proposed**:

1. **Decide the system of record for capture events.** SQLite and
   `events.jsonl` hold the same events. Raw provider notifications that also
   have a derived event are about 39% of remaining history payload. Keeping raw
   lines only in the file would roughly halve the store again, but
   `session history --events` would read two sources and SQLite would stop being
   a complete copy. Proof: the replay script's row and byte counts.
2. **Capture retention.** Expire `runs/<key>` payload and its `observed` rows a
   set time after the Session's Task is done, keeping identity, usage, outcome
   and final answer. A 90-day window would cap a heavy user near 40 GB. Needs a
   decision on what history is worth; started-Task and captured rows are
   protected by triggers today.
3. **Binary and artifact retention.** Keep what `active.json` and `switch.json`
   reference plus the retained published set; delete the rest after a settled
   switch. About 11 GiB here.
4. **Narrow `session_driver_exit`.** Index only rows carrying an outcome.
   About 110 MB here; needs a migration.
5. **Store each prompt once.** `manifest.json`, `user_input` and `context.json`
   each carry it.
6. **Report storage in `lf home doctor`.** Store, WAL, backups, captures,
   binaries, with a warning threshold. This is the missing measurement: none of
   this was visible until the disk filled.
7. **Interrupt cleanup for temporary directories.** `exit(130)` skips
   destructors for release downloads, screenshot profiles and `/tmp/lf-codex-*`.
   Test helpers place worktrees beside their `TempDir`, so those never clean up.
8. **Codex account homes.** Loopflow creates them and sets no retention; Codex
   owns `logs_2.sqlite` and `sessions/`. Check whether Codex exposes a limit.
9. **Legacy directories.** `traces`, `backups`, `lfd.db*`, `logs`, `cache` have
   no writer. An explicit, reviewed archive-or-delete would return about 23 GiB.

Supported cleanup available now:

```sh
lf wt prune --dry-run          # worktrees that are terminal or inactive
cargo clean                    # inside a merged worktree
lf monitor prune --dry-run     # dead process receipts
```
