# Store migrations in local-first tools: research

How comparable tools evolve an on-disk store when several versions run at once
and developers run HEAD builds against real data. Sources read 2026-10-27;
anything marked *(inferred)* was not read in source.

Pinned revisions: openai/codex `19d7301`, anomalyco/opencode (ex-sst) `cdaba26`,
zed-industries/zed `37faef8`, atuinsh/atuin `5e82ba2`, chromium/chromium
`sql/meta_table.h@745fa52a48`, `history_database.cc@33cc35e666`.

## Comparison

| | Id scheme | Older binary, newer store | Destructive changes | HEAD/dev builds | Backup |
|---|---|---|---|---|---|
| Codex | `0001_…sql` sequential ints, one dir per DB file | Opens it: `ignore_missing: true` | Same-PR drop, or a few weeks later | Same `~/.codex` *(inferred)*; `CODEX_HOME`, `CODEX_SQLITE_HOME` | `db-backups/` on corruption, then rebuild |
| OpenCode | `YYYYMMDDHHMMSS_name`, generated | Opens it silently; set-based runner | Direct, sometimes same day | Released channels plus `dev` share `opencode.db`; local/branch builds get `opencode-<channel>.db` | None found |
| Zed | Array index per domain; SQL text stored | Opens it: older list is a prefix *(inferred from loop)* | Table rebuild in-step | Per-channel dir: `0-dev`, `0-nightly`, `0-preview`, `0-stable`, plus `0-global` | Moves a failing DB aside, recreates |
| Atuin | sqlx timestamps | **Refuses**: sqlx default | Direct (`drop table events`) | Same DB | None found |
| Chromium | Integer `version` + `compatible_version` in `meta` | Opens it if `compatible_version ≤ own version` | Floor rises only when needed | Per-channel profile dirs *(known, not read)* | Snapshot on milestone upgrade; restore on downgrade |
| Git / Hg | `repositoryformatversion` + `extensions.*` / `.hg/requires` | Refuses unknown version or extension | Teach readers, wait, then write by default | n/a | n/a |

## Codex

`codex-rs/state/` runs six SQLite files (`state_5.sqlite`, `logs_2.sqlite`,
`goals_1.sqlite`, …; `src/sqlite.rs:48-53`), each with its own sqlx migration
dir. SQLite is an index: per the `src/lib.rs` header it "extracts rollout metadata
from JSONL rollouts and mirrors it into a local SQLite database". A leased
backfill (`runtime/backfill.rs`) can rebuild it. That lowers the stakes of every
migration.

Version skew is handled by one deliberate relaxation (`src/migrations.rs`):

> Allow an older Codex binary to open a database that has already been migrated
> by a newer binary running in parallel… Known migration versions are still
> validated by checksum, so this only relaxes the "database is ahead of me" case.

There is no compatible-version check. Older binaries survive only because most
migrations are additive. Drops ship anyway. In #23300 (2026-05-19) goals moved
into their own DB and `0034_drop_thread_goals.sql` dropped the old table in the
same PR, "intentionally no backfill". Logs moved out on 2026-03-06 (#13772) and
the table was dropped on 2026-04-01 (#16433). That 26-day gap was not enforced by
any mechanism.

Sequential ids collide. In #28671 two merged PRs both claimed `0038`. The fix
renumbered one to `0039` and added `repair_legacy_recency_migration_version`,
which rewrites `_sqlx_migrations` rows on databases that had already applied the
wrong number. Ids become fixed at merge; there is no draft stage.

Until #21847 (2026-05-08, "sqlite: no more destructive version bumps"), raising
`STATE_DB_VERSION` changed the filename and deleted older files. Codex used that
while SQLite was disposable, then dropped it once the data mattered.

## OpenCode

`packages/core/script/migration.ts` diffs the Drizzle schema against a committed
`packages/core/schema.json` snapshot. It emits one `<timestamp>_<name>.ts`
and regenerates `migration.gen.ts`. CI runs `--check` to fail on ungenerated
migrations. Each new migration rewrites the shared snapshot, so concurrent schema
PRs conflict in git rather than at runtime *(inferred)*.

The runner (`src/database/migration.ts`, `applyOnly`) applies by set: any id in
the code that is missing from the `migration` table runs, and ids it doesn't know
are ignored. An older binary therefore opens a newer DB with no check at all. A
separate `data_migration` table tracks data-only passes. Drops are direct:
`20261024002554_session_allowance` added a column and
`20261024175301_drop_session_allowance` dropped it about 17 hours later.

The data-dir story was in flux this week. `packages/cli/src/database-path.ts`
gives `latest/dev/beta/next/prod` the shared `opencode.db`. Any other channel,
including `local` source builds, gets `opencode-<channel>.db`, and `OPENCODE_DB`
overrides it. #58331 (2026-10-23) moved `dev` to `opencode-dev.db`. #59631
(2026-10-27) reverted it at Dax's request because "existing dev installs silently
switched to an empty database… so sessions and MCP sign-ins looked gone."

## Zed

`crates/sqlez/src/migrations.rs`: each domain declares an ordered array of SQL
strings. The `migrations` table stores `(domain, step, migration_text)`.
On open, a stored step whose text differs fails ("Migration changed for {domain}
at step {index}") unless the domain allows the change. Only steps past the
stored prefix run. An older binary iterates its shorter array and skips the rest,
so it tolerates a newer DB *(inferred from the loop; no explicit guard)*. Ids are
array positions, so concurrent appends conflict textually at merge.

`crates/db/src/db.rs:164` puts each channel's DB in its own directory,
`0-{dev,nightly,preview,stable}/db.sqlite`, plus a `GlobalDbScope` (`0-global`)
for state shared across channels. A local `cargo run` never touches the user's
stable data. If opening fails, the directory is moved aside to a backup and
recreated, with an in-memory fallback (doc comment at `db.rs:170-172`).

## Atuin

Atuin is the counterexample. `crates/atuin-common/src/db/mod.rs:97` calls
`sqlx::migrate!(dir).run(pool)` with defaults, so an older binary that finds a
newer applied migration fails hard. Issues #3524 and #3904 report "migration
20260224000100 was previously applied but is missing" from two installs on PATH.
The maintainers' answer is to uninstall the old one. This is the failure Codex's
`ignore_missing` exists to avoid.

## Chromium

`sql/meta_table.h` documents the pair:

> The compatible version number is the lowest current version embedded in
> Chrome code that can still use this database… if an optional column is added
> to a table in version 3, the new code will set the version to 3, and the
> compatible version to 2… check the version number when you're upgrading, but
> check the compatible version number to see if you can use the file at all.

In `components/history/core/browser/history_database.cc`,
`kCurrentVersionNumber = 70` and `kCompatibleVersionNumber = 16`. So 54
migrations have run without locking out older readers. A store whose compatible
version exceeds the binary's current version returns `INIT_TOO_NEW`.
`kMinimalVersionNumber = 15` is the oldest store that can still be upgraded.
`RazeIfIncompatible` wipes stores below a supported floor.

Above the SQLite layer, `chrome/browser/downgrade/downgrade_manager.cc` writes a
`Last Version` file into the profile. On a milestone upgrade it snapshots user
data, keeping one per milestone (`max_number_of_snapshots`). When it detects a
downgrade it restores a matching snapshot, or moves the profile aside.

## Git / Mercurial

`Documentation/technical/repository-version.adoc` says a binary that doesn't
understand the format version "MUST NOT operate on that repository". It
prefers versioning individual files and adding "data that gracefully degrades",
and rolls out format changes by "teach git to read the new format, and allow
writing… with a config switch… after a long period… switch to writing the new
format by default". `extensions.*` and Mercurial's `.hg/requires` *(known, not
read this pass)* list named features. A binary refuses only if a listed feature
is unknown to it, not because a number is higher.

## Patterns that fit Loopflow

1. **`(version, compatible_version)` instead of a reader floor that rises on
   every break** (Chromium; git extensions as the named variant). Additive
   migrations leave the floor where it is. Only a contract step raises it.
   *Cost:* each migration's author has to state whether it breaks older readers,
   and a test should open the new store with the released reader's queries.

2. **Tolerate-ahead runner** (Codex `ignore_missing`; Zed prefix; OpenCode set
   semantics). Older lf skips applied ids it doesn't know, provided the floor in
   (1) admits it. That combination is what lets a main build migrate the real
   store while released processes keep running. Codex has the tolerance without
   the floor, and its drops depend on luck. Atuin has neither, and users get
   locked out. *Cost:* the floor becomes the only guard, so a wrong "additive"
   label shows up as runtime errors in old processes.

3. **Ids fixed at merge, never at release** (all subjects). Sequential ints
   collide across concurrent PRs (Codex #28671, which needed history-rewrite
   repair code). Timestamp-plus-name ids applied as a set (OpenCode, Atuin) don't
   collide. A committed schema snapshot that every migration rewrites (OpenCode
   `schema.json`) turns concurrent schema PRs into git conflicts. Draft-until-cut
   has no counterpart in any subject. *Cost:* timestamp order is authoring order,
   not merge order, so two PRs touching one table need the snapshot conflict to
   force an ordering.

4. **Expand → wait → contract** (git's rollout doctrine; Codex's logs move did it
   by hand over 26 days). No subject gates the contract step on live processes or
   elapsed time in code. Loopflow can, because its store already records
   LfProcess rows with versions. A contract migration could stay pending until no
   live process is older than the expand release and N days have passed since it
   shipped. Then it raises `compatible_version`. *Cost:* the runner needs "pending
   until predicate" migrations, the expand side has to dual-write for the window,
   and the 30-day wait goes on a clock nobody watches unless `lf doctor` shows it.

5. **Snapshot before the floor rises** (Chromium per-milestone snapshots; Codex
   `db-backups/` and Zed's move-aside, both only on corruption). Take a `VACUUM
   INTO` copy before any migration that raises `compatible_version`, and keep the
   last one or two. That gives a downgrade path without carrying old code.
   *Cost:* disk use and copy time on large stores. Restore loses writes made
   after the snapshot.

6. **Per-channel store only for builds that can't be trusted** (Zed: every
   channel; OpenCode: shared for dev/beta/stable, separate file for local and
   branch builds). OpenCode's revert this week shows the cost of separating the
   channel people use every day: their data "looked gone". If (1) through (3) hold,
   main builds can share `~/.lf/loopflow.db`. Only unmerged-branch builds would
   need their own file, empty or copied, which replaces lf-world and
   copy-on-draft for everything except schema work in progress.

7. **Make the store rebuildable** (Codex: SQLite derived from JSONL rollouts,
   leased backfill; it used destructive filename bumps until the data became
   load-bearing, #21847). This applies only where Loopflow has an upstream log. It is a
   way to lower stakes for some tables, not a migration mechanism.
