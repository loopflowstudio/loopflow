# Migration identity and ordering: history

Research for the faster-ci migration redesign: main-time migration ids and
reader floors, plus staged expand → wait → contract for destructive changes.
Sources: `git log` over every migration path, the registries and scripts,
PR bodies, and `release/v0.13.1/RECOVERY.md`.

## Timeline

| Era | Dates | Identity / order | Introduced by | Why | What it broke |
| --- | --- | --- | --- | --- | --- |
| 1. Timestamped Python | Jan 20–23 | `m_YYYY_MM_DD_name.py`; lexical | 993a869d | "explicit, timestamped schema migrations" | Same-day ordering was forced with `z_`/`zz_` prefixes (`m_2025_01_22_z_rename_sessions`, `m_2025_01_23_zz_stimulus`). The first files were dated 2025 by mistake and sorted correctly only by luck. After 3 days the 8 files were collapsed into `baseline.py` with auto-reset (a2e1c10a) |
| 2. Sequential `NNN_` | Feb 2 – Jul 13 | 3-digit serial chosen in the PR; registry is a hand-edited `include_str!` list | 5638647485, #317 | Shared SQLite/Postgres chain | Two duplicate prefixes landed: `016_` (#452 vs #458, same day) and `061_` (#870 vs #871). Both were kept, with a comment saying "No renumbering — that would require a corrective migration". At least one renumber happened at rebase: #792 "renumbered 043→046 after #782's 042–045". #452 and #508 added 016–019 and 022–023 in reversed file order, and `010_` was reused after deletion. 55 registry edits. Ended by a reset to one `001_initial.sql` in #872 ("60+ accreted migrations") |
| 3. Release-namespaced ordinals | Jul 14–17 | `0.11.NNN`; the allocator reads origin/main | #876, #885 | Forward-only chain scoped to a release | Peak churn: 37 ordinals and 38 first-parent edits to `migrations.rs` in 4 days. #935 shipped with "an ordinal-only rebase may be needed if another migration lands first". Renames: f5c9ba6e (008/009→010/011), #940 (009/010→011/012), and a branch draft that went 020→022→024 and never landed (the `0.11.028` gap). The **migration fire** (below) |
| 4. Hash-named drafts, cut at release | Jul 17 – Oct 1 | `drafts/<name>__<128-bit>.sql`, no ordinal, `-- depends_on:`; release cut topo-sorts (ties broken by name) into `<ver>.NNN_release.sql` | #1076 (ENG-23), #1081 (ENG-24), #1111 | "two branches racing to add a migration would contend for the same serial number … a branch merely behind main's migrations would collide on merge" | Ordinal churn stopped. The publication-boundary bugs that followed are listed below. Tasks piled up drafts: #1296, #1354 and #1382 each added 2–3 |
| 5. One plain-named draft per Task | Oct 1 → | `drafts/<name>.sql`, edited in place | #1385 (LOO-344) | "leaving maintainers to manage intermediate schemas that had never shipped" | No PR has added two drafts since. The release cut keeps racing with merges (below) |
| 6. Reader floor + draft copies | Oct 20 → | `reader_floor` per ledger row, computed at the cut; draft builds run on `~/.lf/copies/<digest>` | #1608, #1612 | "a store migrated by a newer lf locked out every older lf" | Floors only rise: 5 of the 6 migration-bearing releases since (0.14.12, .15, .16, .17, .19) raised them. The first copy costs 265 s on an 8.8 GB store, and #1727 cleans up copies a killed draft build left behind |

## Churn, quantified

- **Ordinal collisions or renumbers (eras 2–3):** 2 duplicate prefixes merged,
  4 renumber commits (#792, f5c9ba6e, #940, 3e2c3d5c/2fd5ebbf), and 1 PR shipped
  with a known pending renumber (#935). All of it came from a serial number
  chosen in the PR.
- **Central registry:** in era 3, every schema PR edited both `migrations.rs`
  and the next ordinal (38 edits in 4 days). Since era 4, only release PRs
  touch `migration_catalog.rs`; there have been 2 non-release edits in total
  (#1344, #1603).
- **Release-cut races (eras 4–5):** a draft merged while a release was being
  prepared left the merged source "unprepared". The fixes were #1408, #1451
  (v0.13.1 skipped: "Preparation used an older snapshot; integration changed
  its migration set"), #1661 and #1662. 6 of about 40 batches are repair cuts
  (`0.13.9.002`, `0.13.12.002/.003`, `0.14.15.002`, `0.14.16.002`,
  `0.14.17.002`). There were 87 release PRs for 70 versions, with v0.13.9 taking
  5 PRs.
- **Semantic collisions between drafts:** 3 PRs edited another Task's
  unreleased draft (#1520 → `agent_process`, #1521 → `local_planning`,
  #1709 → `scope_tasks_capacity`). #1709 is the "valid alone, broken together"
  case. #1704's draft deleted all RunPod rows on the premise that "no
  published release could rent from RunPod"; #1707 made that false. It had to
  "land before the next release cut turns the draft into a permanent schema
  batch".
- **Ordering by hand:** 39 of the 109 drafts added on main declared
  `depends_on`, many on other Tasks' drafts (`rental_spot → scope_tasks_capacity`,
  `agent_process → optional_task_pr`). Name-order tie-breaking means an
  undeclared edge silently runs in alphabetical order.
- **"One draft per Task" violations since #1385:** none on main.

## Live-store disruptions

- **Jul 15, the migration fire (#940, #960 W2-218, #962):** a branch-local
  ordering of `0.11.009–012` "reached a production ledger before main
  established its canonical sequence". The fix needed ledger-rewrite
  convergence code that still ships ("Known leading migration names under
  branch-local ordinals"), and made main the authority.
- **Jul 17 (#1086):** "a published candidate at `target/release/lf` migrated
  the shared store during a normal open and stranded the installed binary."
  After that, only `lf install promote` advances the shared store.
- **Jul 19 (#1108, #1123):** a post-0.12 binary published an unreadable 0.11
  migration under the 0.12.1 id. A clean-main build replaced the tagged CLI
  "even though its runtime queried draft schema absent from its embedded
  registry". Rolling back exposed nanosecond timestamps.
- **Jul 20 (#1132):** a local promotion applied a *test-materialized*
  `0.12.4` batch "whose canonical source never landed". It was recovered
  from the installed binary's bytes, matched by checksum.
- **Oct 20 (#1608):** any newer release locked out every older lf, so side-by-side
  installs or a rollback meant deleting the store.

Each incident came from an id or a set of SQL bytes that existed on a store
before it was fixed on main.

## Hot spots today

A schema PR touches `drafts/<name>.sql` and optionally
`tests/fixtures/migrations/<name>_released.sql` (34 fixtures since Oct 10).
Neither file is shared. The release PR rewrites `migration_catalog.rs`,
renames drafts into the batch, and writes `release/unreleased/BREAKING.md`.
The schema reference is already generated at build time (`build.rs`,
#1344), and CI materializes drafts in a disposable checkout
(`materialize_rust_tests.py`). No schema dump is committed.

## Constraints for the next design

1. **The PR never chooses a sequence number.** Eras 1–3 each produced
   collisions, renumbers or z-prefix hacks. Assign the id after merge,
   mechanically, from main's history.
2. **No hand-edited central registry in a feature PR.** The 38 edits in 4
   days during era 3 were all rebase bait. Discover drafts from the directory,
   or generate the registry at build time.
3. **An id and a floor exist only for bytes that are on origin/main.** All
   five live-store incidents involved branch-, test- or candidate-only SQL
   reaching a real store. Ids assigned at merge must be reproducible by any
   build of that commit.
4. **Immutability starts at origin/main.** Once a store may have applied a
   migration, a cross-Task fix like #1709 has to be a new forward migration.
   That boundary moves from the release cut to merge, so pre-merge review must
   catch what #1709 caught after merge.
5. **Order comes from merge order or declared edges, never names.** Today
   ties are broken alphabetically and 36% of drafts carry manual edges.
6. **No step that snapshots the draft set and races integration.** The repair
   batches, #1408, #1451 and #1661 all came from preparation reading main
   before a migration merged. Assignment has to happen on main itself, or be
   idempotent against what merged.
7. **The floor is computed from the SQL, not declared.** Keep #1608's
   proof-by-application. Expand steps must keep the floor, and a contract step
   raises it no earlier than its expand has been on main and released for the
   wait period. Today floors only ever rise to the release being cut.
8. **Never reorder or rewrite a ledger.** The convergence code from #940/#960
   is debt; no new scheme should need it.
9. **Keep the existing safety rails.** That means one migration file per
   Task, upgrade tests seeded from released shapes, the per-migration
   foreign-key and JSON checks, and promotion as the only path that advances
   the shared store.
