# 5 whys: database-locked failures → fast, safe local hotfixes (cbda52)

Jack Heart asked for this on 2026-10-26. The question is why diagnosing and hotfixing
the "database is locked" failures on his machine, against his live store, was slow
and risky. The lock fix itself belongs to db678e and is not in scope here.

Times are UTC on 2026-10-27 unless marked "local" (PDT, UTC−7).

## Recovery status (separate from this analysis)

- **Mitigated, not resolved.** At 04:16 the LOO-604 Session promoted hotfix
  `e9b2c2eac1` ("Replace usage receipts in place…", branch `hotfix/0.14.17-usage-update`,
  built on v0.14.16) to `~/.local/bin/lf` with `lf self install promote`. At 04:23 it
  then closed 17 interactive Sessions that had been idle for 12 hours or more.
- Lock warnings per minute: 23–24 at 04:19–04:20, 7 at 04:25, **none from 04:26 to
  04:35**, then 3 at 04:36. Total in `lf.log`: 891.
- The retention cursor moved off 26536 at 04:35 (it now reads 26562), so the
  hourly prune is making progress again.
- **27 processes still run the buggy 0.14.16 binary `lf-d49c57c…`**, and each one
  still does the slow delete. Desktop's bundled helper
  (`/Applications/Loopflow.app/Contents/MacOS/lf`) is also still 0.14.16, and it
  started new helpers at 21:21 and 21:23 local, after the promote.
- **Regression hazard:** the open release PR #1710 (v0.14.17, cut from main
  `a0d5a78571`) does **not** contain `e9b2c2eac1`. Both main and #1710 still run
  `DELETE FROM session_events WHERE seq=?1` at `session_events.rs:737`, and
  `retention.rs:176` does the same delete. #1710 also raises the reader floor
  ("lf < 0.14.17 can no longer open the store"). If 0.14.17 is installed as cut,
  the bug comes back, and every running 0.14.16 process loses the store again.
  db678e part B covers fixing main. #1710 needs that fix before it merges, or the
  release has to include the hotfix commit.

## What failed and who it affected

- Since 0.14.16 was installed (switch receipt at 17:02 local), each usage receipt
  deleted and reinserted its row in `session_events`. The foreign key
  `captured_event REFERENCES session_events(seq)` makes SQLite probe child rows on
  every delete. No index leads with `captured_event`, so each probe is
  `SCAN session_events`: 2.78M rows, about 8 GB, reading overflow pages. This runs
  under the write lock, and every other writer gives up after the 15 s busy timeout.
- Affected: Jack's Graphs (LOO-604 `sync`, LOO-610 `implement`), `lf task run`,
  `interrupt` and `sync`, Process ledger rows (335 dropped), Session activity (276),
  Claude conversation selection (136), account pinning (82), and the hourly prune.

## Timeline of the response (LOO-604 Session transcript)

| UTC | Event | Gap |
|---|---|---|
| 00:02 | First lock warnings (0.14.16 switch at 17:02 local) | |
| ~02:15 | LOO-604 Graph stops at `sync` on "database is locked" | |
| 03:26 | Jack asks "whats going"; the agent finds the stopped Graph | **~70 min before anyone noticed** |
| 03:30 | `lsof ~/.lf/loopflow.db` lists dozens of holders, with no way to tell the writer | |
| 03:33 | Blames LOO-626 (worktree locks), relaunches the Task | wrong lead |
| 03:39 | `sqlite3 BEGIN IMMEDIATE` probe: 2 s per lock; relaunches again | |
| 03:46 | Retracts LOO-626 and reads `lf.log` (no pid or version on any line) | |
| 03:47 | `sample` of the busiest lf shows `retain_history_in → vdbeColumnFromOverflow` | |
| 03:48 | `EXPLAIN QUERY PLAN` gives `SCAN session_events`; 6.7 s read-only | **diagnosis 18 min after starting** |
| 03:53 | Jack: "do A then B" (hotfix off the tag, then fix main) | |
| 03:54–04:02 | Can `lf release` cut from a tag? (no). Is an index possible? (no, see why 4). Finds July `~/.lf/bin/hotfix-*` dirs with no mechanism behind them | **8 min of process discovery** |
| 04:03–04:09 | Cold `cargo build --release` in a new worktree | 6 min |
| 04:10 | `promote --preview` refuses: "only a published candidate may advance…" | |
| 04:10–04:15 | Reads `build.rs`, rebuilds with `LOOPFLOW_BUILD_PROVENANCE=release LOOPFLOW_MIGRATION_AUTHORITY=published` | 5 min, second full build |
| 04:16 | Promotes. Preview verdict: "promote (no migration to apply)" | |
| 04:18 | Focused tests run for the first time, **after** the promote (4/4 pass) | |
| 04:18–04:24 | 35 processes still on the old binary. An ad hoc Python script reads transcripts to find idle Sessions, and 17 are killed | 6 min |

Diagnosis took 18 minutes once someone looked; the incident ran 70 minutes before
anyone did. Hotfix to installed took 28 minutes, and about 21 of those went to
process discovery and a rebuild that wasn't needed. Rollover is still incomplete
20 minutes after the promote.

## Causal chain

### Why 1: the incident ran unnoticed for 70 minutes, and 18 minutes went to finding the writer
- Evidence: every lock warning has the form `WARN loopflow::journal: ledger insert
  failed … error=… database is locked trace_id=… waited_seconds=15.5`. None carries
  a pid, lf version, binary hash or call site, so you can't tell old binaries from
  new ones, or victims from the holder. I confirmed this on the tail lines in
  `~/.lf/logs/lf.log`.
- Evidence: `lf doctor`, run at 04:37 with 27 buggy processes live, prints `ok` for
  store, install-selection, install-store and install-fallback. Its only warning is
  "not on cached origin/main". It says nothing about contention, the 891 lock
  warnings, the failed prunes, the stuck retention cursor, or processes on
  superseded binaries. It also took 18.8 s to run, probably waiting on the lock.
- Why that was possible: the store logs only the victims' busy timeouts, never a
  writer's held duration. SQLite has no "who holds the lock" query, so only the
  writer can report itself. The busiest-CPU `sample` heuristic worked this time
  because the holder was CPU-bound on overflow reads. An I/O-bound holder would
  have been invisible.

### Why 2: the fix shipped before any measurement or test
- Evidence: the promote happened at 04:16 and the first test ran at 04:18. The
  fix's effect was measured only afterwards, with `BEGIN IMMEDIATE` probes on the
  live store.
- **Reproduction on a clone works, and it's cheap** (I verified this in this
  Session at 04:33): `cp -c` of the 10.5 GB db plus WAL took **0.01 s**.
  `integrity_check(1)` returned ok. On the clone, the FK-checked
  `DELETE … WHERE seq=<observed row>` took **9.12 s cold and 2.13 s warm**, and the
  equivalent `UPDATE` took **0.01 s**. `EXPLAIN QUERY PLAN` of the child probe
  gives `SCAN session_events`. That's one statement, not simulated load. The clone
  was removed afterwards.
- Why it was skipped: no recipe says "clone the store, reproduce, measure the fix,
  then promote". `lf clone create` exists for offline data clones but wasn't
  reached for. A repro on a clone takes under a minute, while the live-store probes
  ran for six.

### Why 3: getting a hotfix onto the live store meant asserting release authority by hand
- Evidence: a normal release build of the hotfix is `validation_only`, and promote
  refuses it ("only a published candidate may advance a receipt-managed
  installation", `install.rs:2521`). The agent got past this by setting the
  build-time env vars `LOOPFLOW_BUILD_PROVENANCE=release` and
  `LOOPFLOW_MIGRATION_AUTHORITY=published`. The resulting receipt records the branch
  commit as `"source": "published", "source_identity": "release"`, and
  `lf doctor` reports "Build: release (release) · migrations published". Nothing on
  the machine distinguishes it from a real release.
- Why that's risky: the same env var that let a two-file fix run would also let a
  build carrying a migration draft advance the 10.5 GB store with no published
  release behind it. `store/copy.rs::selected()` skips the copy whenever authority
  is `Published`. `~/.lf/backups/` is empty, and promote retains only rollback
  bytes for the binary.
- Why the guard was there: LOO-226 (completed) requires that "development binaries
  never receive reliable-store authority". LOO-107 (completed) established that an
  exact-frontier repoint needs no drain, because it migrates nothing. Together
  they leave no path for a build that is **not** a release but migrates nothing,
  and that is exactly what a hotfix is. As AGENTS.md's Flexibility section
  predicts, the refusal was worked around, and the result is worse provenance and
  latent migration authority.
- What it cost: one wasted 6-minute build (the first one lacked the env vars, and
  changing them forces a rebuild), 5 minutes reading `build.rs`, and 8 minutes
  learning that the release tool can't cut from a tag. The July hotfix
  (`~/.lf/bin/hotfix-*`, Jul 17 20:12 to 20:13) left directories but no recipe.

### Why 4: the safest fix (an index) was impossible without a migration
- Evidence: `validate_schema` requires the product schema to match the migration
  chain exactly, and `_read_product_schema` includes every index. An extra
  `CREATE INDEX … ON session_events(captured_event)` on the live store would make
  every lf, old or new, refuse the store. So the hotfix was limited to changing the
  code path. In this case updating in place was the better fix anyway.
- Proposed only: none filed. Exact schema matching is the contract that keeps the
  reader floor honest (LOO-521). The real prevention is upstream: a CI check that
  every `REFERENCES` column leads an index, so this class can't ship again. That
  belongs with db678e.

### Why 5: the fix reaches new processes only, and nothing rolls the old ones over
- Evidence (ps at 04:27): 60 processes on 0.14.15 `lf-270110a…`, 30 on buggy 0.14.16
  `lf-d49c57c…`, 4 on 0.14.14 `lf-870c35a…` (alive since Oct 23), 29 on the hotfix,
  and Desktop's helper on 0.14.16. Some `session serve` and `__transport-serve` hosts
  have been running since Oct 24. `lf-d49c57c…` processes started at 21:17:02 local
  from a fresh cmux shell, during the promote window. The 0.14.15 processes can't
  reopen the store after 0.14.16's migration (`lf.log`: "this store needs lf 0.14.16
  or newer"); 5 Session recorders lost telemetry that way at 00:xx.
- Why that's possible: LOO-107 pins running children to their binary on purpose,
  so a repoint can't change code mid-session. That's correct, but it means a
  hotfix doesn't take effect until those processes exit, and lf neither shows
  which ones are stale nor offers a way to move them. LOO-226's safe-stop
  classification (native resume route, lifecycle relaunch) exists for promotion,
  but the agent rebuilt a weaker version by hand: last typed message per
  transcript, then `kill`. The hotfix's artifact set is CLI-only (`activation.app:
  null`), so Desktop's helper isn't touched, and the receipt still records
  `app_was_running: false` while Loopflow.app (pid 42667) was running.

### Branch: release coupling
- Earlier the same day, 0.14.16 raised the reader floor mid-run (#1701, "lf <
  0.14.16 can no longer open the store"), and running 0.14.15 bodies lost the store.
  #1710 raises it again for 0.14.17, carries the `session_facts_only` rewrite of a
  10.5 GB store, and doesn't include the hotfix. LOO-521's reader floor makes the
  break visible in the PR title, but nothing connects "an installed hotfix commit
  isn't in the release you're about to install" to the release or the install.

## Contradicting evidence and open questions

- The 3 warnings at 04:36 came right after the retention cursor advanced at 04:35.
  Does the prune itself hold the lock for a long time? `retention.rs` (v0.14.16:286
  and :341, main:176) deletes `session_events` rows one at a time, and each delete
  runs the same unindexed child probe (about 2 to 9 s per row on this store). If
  that's confirmed, the prune is a second holder that the hotfix doesn't touch.
  Belongs to db678e.
- Not every one of the 27 remaining `lf-d49c57c…` processes writes usage receipts.
  `session serve` hosts and `-i` launches probably do, and `task run` waiters
  probably don't. lf can't tell me which, and that is Why 1 again.
- `~/.lf/copies/` holds 5 draft-copy stores and **9 orphaned `.tmp*` snapshot dirs
  from today** (08:23 to 19:40 local). Each is about 10 to 11 GB apparent, with WALs
  up to 1.6 GB. The disk is 93% full (68 GB free). APFS sharing makes the real cost
  uncertain, but several were made during the contention window and look like
  interrupted snapshots. This matters for safety because every repro clone needs
  headroom.
- 76516 (`session connect`, 21:17:02 local) started on the old binary. Either it
  raced the gate flip, or something resolved the old path. I didn't confirm which.

## Proposed changes (filed as Tasks LOO-632–636; none approved or built)

| # | Task | Addresses | Demo |
|---|---|---|---|
| LOO-632 | Name the store's write-lock holder in logs and `lf doctor` | Why 1 | Clone the store and run the 0.14.16 delete under one lf. `lf.log` names its pid, version and call site with the held seconds, and `lf doctor` reports contention by binary. |
| LOO-633 | Promote exact-frontier hotfix builds without claiming release authority | Why 3 | Build the hotfix with a plain `cargo build --release`, then `promote --preview` and `promote`. The receipt and `lf --version` say "hotfix e9b2c2e on v0.14.16". A build with a draft is still refused, and the env-var bypass is gone. |
| LOO-634 | Show and roll over processes still running a superseded lf | Why 5 | After a promote, `lf doctor` lists stale processes by version, and one command moves the resumable ones (including Desktop's helper) onto the new binary. |
| LOO-635 | Ship a loopflow repo `hotfix` skill (`.lf/skills/`, not a builtin): clone, reproduce, test, build once, promote, roll over, PR to main | Why 2 and 3 (process) | The next hotfix follows the skill from the installed tag. The repro and the fix are measured on a clone before the promote, and the PR to main is opened in the same run. |
| LOO-636 | Remove abandoned store-copy snapshots | Safety branch | `~/.lf/copies/.tmp*` with no live owner are removed. `lf disk` reports the space reclaimed. |
| db678e | (existing; evidence comment added) fix main before #1710; prune deletes; CI check that FK columns lead an index | Regression hazard, Why 4 | (db678e's own demo) |
| — | LOO-521 (completed), LOO-107, LOO-226 (completed): prior attempts | Context | LOO-633 narrows LOO-226's rule instead of adding a third promotion path. |

Why this is the smallest set: LOO-632 and LOO-634 turn the two blind spots (who holds
the lock, who is still on old code) into facts lf reports. LOO-633 removes one
refusal and the manual bypass it forced. LOO-635 writes the recipe once, so the next
hotfix doesn't spend 21 minutes on discovery. A faster `cargo build` or a backup
system would cost more and wouldn't have changed this incident.

**Next useful action:** before #1710 merges, land db678e part B on main (or include
`e9b2c2eac1` in the release). Then take up LOO-632, because it is what would have
cut both the 70 minutes before anyone noticed and the 18 minutes of diagnosis.
