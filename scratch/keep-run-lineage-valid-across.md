# Keep run lineage valid across trace boundaries and retention

## Problem

`lf doctor --json` reports `lineage=fail`: 7 parent process ids are "missing or
belong to another trace". The check has been red since 2026-07-15 and was
reproduced by the infrastructure `telemetry-daily` cron on 2026-07-16. A
permanently red check is a check nobody reads, and lineage is the column every
`lf runs` tree and every cost rollup walks.

## Evidence (live ledger, `~/.lf/loopflow.db`, 2026-07-16)

All seven are one shape, not two:

| parent process | child rows | child traces | parent found in *any* store |
|---|---|---|---|
| `c95a8ae5…` | 136 | 1 (`247ad4d0…`) | no |
| `cb6f9ff1…` | 2 | 1 (`83411fe5…`) | no |
| `d6991842…` | 82 | 1 (`c5326aba…`) | no |
| `713d8ca6…` | 4 | 1 (`4b51780f…`) | no |
| `0a04f377…` | 32 | 1 (`1bef97ac…`) | no |
| `041f7b99…` | 12 | 1 (`1bef97ac…`) | no |
| `706b8ce5…` | 9 | 1 (`8fc4c4e3…`) | no |

Findings that reshape the task:

1. **Zero of the seven are cross-trace.** Every child names a parent that
   appears in no store on this machine — not `~/.lf`, not `~/.lf-dev`, not any
   of the 20+ per-worktree dev stores. The "belongs to another trace" half of
   the check never fired. The writer already prevents it
   (`ensure_run_context` drops an inherited `LF_PROCESS_ID` when it mints a
   fresh run id), and the evidence says that guard works.
2. **There is no retention on `run_events`.** No sweeper, no window, no
   `DELETE` outside migration `0.11.025`'s one-off junk purge (which post-dates
   the failure). Nothing ages out, so a dangling pointer written once fails the
   check forever.
3. **The ledger is younger than the processes that write to it.** `~/.lf/loopflow.db`
   was created 2026-07-14 18:23:25; `0.10.001_initial` applied at 18:23:25 and
   the first row landed at 18:23:41. Every store before that is gone.

### Root cause

`ensure_run_context` (`rust/loopflow/src/journal/mod.rs:591-599`) exports
`LF_RUN_ID` and `LF_PROCESS_ID` into the process environment **before, and
regardless of whether, the row reaches the ledger**. `ledger_insert` is
best-effort by design and swallows every failure into one `warn!` and then
`debug!`. Two live paths reach it:

- **A refused or unavailable store.** `guard_development_database`
  (`store/mod.rs:227`) returns `PermissionDenied` when a development build
  resolves the production database; a locked or migrating store errors the
  same way. The process still exports its identity, and any child `lf` — a
  *release* binary resolved from `PATH`, which is allowed to write production —
  inherits it and writes rows naming a parent that recorded nothing.
- **A store replaced under a long-lived process.** `lf wave` listeners and
  detached tmux bodies outlive the store. `start_lf_session_with_env`
  (`engine/process.rs:405`) deliberately copies `LF_RUN_ID`/`LF_PROCESS_ID`
  into a detached session, so an id minted against the pre-2026-07-14 store
  keeps stamping children in the new one. Trace `8fc4c4e3…` is exactly this:
  five `lf __resident rules` respawns across four hours, all naming one parent
  that never wrote.

Both are the same defect at the same ownership boundary: **a process exports a
lineage pointer the ledger does not hold.** The child then writes a pointer to
a ghost, and the reader — correctly — calls it corruption.

## The demo

`lf doctor` on the real machine store prints `ok lineage — every parent
process resolves`, having been red for two days; and a new test proves that a
child inheriting a parent id the ledger never recorded writes `parent_process_id
= NULL` instead of a dangling pointer, while a genuinely missing parent inside
a recorded trace still fails the check.

## Approach

Fix it where the existing doctrine already lives. `ensure_run_context` already
carries this comment:

> A parent process id only means "my parent within this trace." … Drop it so
> the violation is unspellable at write time.

That rule is right and its scope is one case short. Extend it from "a parent
from another trace" to "a parent this ledger does not hold", which subsumes
both live paths without guessing:

1. **Writer.** At `run/started`, an inherited `LF_PROCESS_ID` is honored only
   when the ledger holds a row for it. Otherwise the parent is dropped and the
   inherited run id is kept — the child is a root of its trace, which is the
   truth: "these ran under one trace; their parent is not in this ledger."
   One indexed lookup (`idx_run_events_process`) on a path that opens the
   store microseconds later anyway.
2. **Reader.** `check_lineage` stays strict — a parent must resolve inside its
   own trace — but splits its count into the two classes it always conflated
   (absent from the ledger vs. present under another trace) and names the
   offending ids. "7 parent process id(s) are missing or belong to another
   trace" cost hours of archaeology to turn into "all 7 are absent"; the
   check should say which failure it found.
3. **History.** Migration `0.11.026` applies the writer's new invariant to the
   rows already written: null `parent_process_id` where no row in this ledger
   carries that process id. It removes a pointer that names nothing; every run
   and every token stays.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Are any of the 7 cross-trace? | No. All 7 parents are absent from every store on the machine (checked `~/.lf`, `~/.lf-dev`, and every per-worktree dev store). | Dropped the cross-trace repair from scope; the existing writer guard already holds. Kept the reader's detection of it. |
| Is retention pruning `run_events`? | No retention exists at all. Only `0.11.025` deletes rows (a one-off tmp-repo purge, merged 2026-07-16, after the failure). | "Retention" in the task title is really *store replacement*. Designed for that, not for a sweeper that doesn't exist. |
| Then why is every parent absent? | `ensure_run_context` exports identity before `ledger_insert`, which swallows all failures (`journal/mod.rs:599` vs `:459`). A refused dev-build write (`guard_development_database`) or a store replaced under a long-lived process both produce it. | Fix at the writer's ownership boundary, not the reader's tolerance. |
| Could the lookup falsely drop a legitimate parent (parent's row not yet inserted when the child starts)? | No. The parent sets env and inserts its `run/started` row in the same `try_emit` call, before it does any work; a child cannot exist until after. Detached sessions start later still. | The lookup is safe without a retry or a grace window. |
| Should the writer instead refuse to export identity it failed to record? | Considered and rejected — see Alternatives. A transient busy store would fragment a healthy trace into orphan runs. | Kept the export unconditional; validate at the child, where the evidence is. |
| Can the boundary be dated (pre-ledger parent vs. genuinely lost)? | No. The parent's rows are gone, so nothing on the machine can date them. Any classifier would be a guess. | Refused to infer a "pruned" class from timestamps. The writer records the boundary as it happens instead. |
| Is the migration ordinal free? | `0.11.025` is the tip on main (merged today, #1022). Rechecked open PRs for `run_events` migrations before landing. | `0.11.026_lineage_boundary`. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Export `LF_RUN_ID`/`LF_PROCESS_ID` only after the ledger accepts the row | Kills the lie at its source | A momentarily locked store would stop the export and fragment one healthy run into unrelated traces — trading a rare pointer defect for routine trace loss. The journal is best-effort on purpose; this couples trace identity to store availability. The child-side check catches the same case with no such cost. |
| Add a typed `parent_boundary` column (`external`/`pruned`) | Preserves "there was a parent" | A wire-visible DTO field, a migration, and three language mirrors to carry a value no reader renders — `lf runs` cannot draw a parent it does not have. The task's "explicit typed boundary" is satisfied by the writer refusing to write the ghost. Revisit if a reader ever needs it. |
| Relax `check_lineage` to warn when the parent is simply absent | Green today, zero code | Exactly the "retention masquerading as corruption" inversion the task forbids, pointed the other way: it would blind the check to the real defect it just caught. |
| Backfill by deleting the orphaned children | Green today | Destroys 277 rows of real runs and their spend to fix a pointer. |
| Leave history dangling, fix only the writer | Least invasive | `run_events` has no retention, so the 7 never age out; `lf doctor` stays red forever and the cron keeps crying wolf. |

## Key decisions

- **The parent, not the child, is validated.** A dangling pointer is not the
  child's error; it is the parent's unrecorded existence. But the child is the
  only party still running when the truth is knowable, so the check lives there.
- **Keep the inherited run id when dropping the parent.** Minting a fresh trace
  would be equally truthful and strictly less useful: trace `247ad4d0`'s 68
  processes really did run under one `lf wave` session, and `lf runs` should
  still group them.
- **Null the ghost pointers in history rather than deleting rows or relaxing
  the check.** The rows are evidence of real runs; only the pointer is false.
- **The reader keeps failing on a genuinely missing parent inside a recorded
  trace.** That is the case the check exists for, and the regression test pins
  it.

## Scope

- In scope: the `ensure_run_context` parent rule; a `process_is_recorded`
  store query; `check_lineage`'s two-class reporting; migration
  `0.11.026_lineage_boundary`; regression tests for nested runs, cross-trace
  handoff, an unrecorded parent, and the migration.
- Out of scope: the `capture` check (a separate red signal, per the directive);
  building retention for `run_events`; the swallowed-`ledger_insert` warning
  policy; `guard_development_database`'s refusal itself (correct as designed).

## Done when

- `cargo test -p loopflow journal:: store:: doctor::` passes, including the new
  cases.
- `lf doctor --json | jq '.checks[] | select(.name=="lineage")'` reports
  `"status": "ok"` against the real machine store.
- A test proves an unrecorded inherited parent is dropped at write time, and a
  test proves a missing parent inside a recorded trace still fails.

## Measure

Baseline: `lineage` = fail, 7 dangling parents across 277 child rows
(2026-07-14 → 2026-07-16). After: 0 dangling parents, and no new ones appear
in a week of real runs — re-check with the query in "Evidence".
