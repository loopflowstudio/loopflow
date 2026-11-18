# LOO-938 — Send every planning change through one queue: what remains

## Where this starts

LOO-925 (#1957, merged 2026-11-18) already did most of this Task's outcome for
Linear and Asana: every outgoing change is one `tracker_actions` row with a
kind, one bounded drain (`ops/tracker/drain.rs`), one rule for conflicts
(`planning::rules::decide`), one status view (`TrackerActionStatus`, `lf repo
plan`). Served writes sync after their reply (`journal::request::after_reply`,
`planning_sync::sync_after_save`). LOO-941 owns dropping the receipt tables it
replaced (`task_changes`, `project_changes`, `task_state_deliveries`,
`task_comment_deliveries`, the `export_*` columns). None of that is reopened
here.

Steer, 2026-11-18: "Start by checking what of this outcome remains after
LOO-925 (planning_peer_changes/observed/routes, delivery after the Machine
answers) and scope to that." From LOO-925's concept review, Jack Heart: "3a
for now, possibly a follow up task?" (3a: Git is a tracker on surfaces only.)

What remains is the Git side, and it has two real costs:

1. **Every planning write in every repository journals itself for Git.**
   `planning_write::write_record` calls `capture` for every `Local` and
   `Tracker` write, whatever the repository plans through. Live store
   (`~/.lf/loopflow.db`, 2026-11-17): `planning_peer_changes` 43,995 rows,
   `planning_peer_observed` 32,753, `planning_peer_routes` 6,665; the three
   tables and indexes take 30 MB. 26,084 changes belong to
   `/Users/jack/src/loopflow`, a Linear repository with no Git destination;
   37,452 changes have no route at all. Only `etude` plans through Git today
   (manabot, kata and etude-studio were Git and now say `provider: linear`).
   Each Linear acceptance and each local save pays one journal insert per
   changed field plus the head lookups, under the shared write lock.
2. **A foreground session on a Git repository fetches the remote every
   second.** `PlanningSync` runs `publish_repository` every 1 s, and
   `publish_destination` always acquires (a `git fetch`) before deciding
   there is nothing to publish; acquisition also runs every 5 s. An idle
   Session makes about 4,300 fetches an hour. That is the Git twin of the
   pending-export drain that stalled Linear.

## Decision: Git stays a replica, not a queue (made for review)

Passed over: moving Git exchange onto `tracker_actions` (`tracker='git'`).
The Git document is a state-based replica: a causal journal of every
mutation, merged with the remote and pushed whole. `tracker_actions` is a
request queue to an API that answers each request. Putting Git's outgoing
side in the queue keeps the journal anyway (merge, causality, losing values
all need it) and adds a second row per save. LOO-925 already put Git's lost
values in the same `TrackerActionStatus` view, so a person sees one view
today. The work that pays is making the journal exist only where Git planning
is, and making Git publish only when there is something to publish. If Git
later needs "N open" lines, they come from unrouted journal changes, not a
queue row.

## Done when

- A save or a tracker event in a Linear or Asana repository writes no
  `planning_peer_*` row.
- A Git repository journals every save as today; switching a repository to
  Git publishes its whole current plan on the first exchange.
- A foreground Session on a Git repository makes at most one fetch per 5 s
  tick, whatever other repositories on the Machine are doing; it publishes on
  a tick only when its destination has an unexported change or an
  unconfirmed push. A save still publishes right after the command answers.
- A foreground Session in a Linear or Asana repository starts no Git
  exchange thread.
- The live store loses the journal rows of repositories with no Git
  destination (about 26k changes and their observed rows).

**Demo.** In a Linear repository, `lf task edit <id> --title X`, then
`sqlite3 ~/.lf/loopflow.db "select count(*) from planning_peer_changes"`
stays the same. In a Git repository, the same edit adds rows and a second
clone's `lf task show <id>` sees the title after one exchange.

**Acceptance check at gate.** An integration test in
`rust/loopflow/tests/work/local_planning.rs`: one Machine, two repositories,
one `provider: linear` (fixture Linear) and one on shared Git with a bare
remote. Edit a Task title in each. The Linear repository's Task has no
journal rows and one queued `tracker_actions` row; the Git repository's Task
has a journal change routed to its destination after `lf task sync`, and a
second store importing that remote shows the new title. Then switch the
Linear repository's config to Git, run `lf task sync`: the remote document
holds every one of its Tasks' current titles.

## Source of truth

The repository's `.lf/config.yaml` `planning` stays the only selection. The
store learns it where the config is already read: every Git exchange runs
`configured_destination` → `bind_peer_planning`, and every Linear or Asana
save runs `sync_saved` → `drain_repository`. Each records the repository's
tracker on the store, `repository_plans.planning_tracker` (`git`, `linear`,
`asana`, NULL = not yet seen; additive column, this Task's one migration).
The writer journals a record iff its repository's tracker is `git`.

A recorded tracker can be stale only between a config switch and the next
exchange. Linear → Git: the first Git exchange records `git` and, in the same
transaction, captures every record the repository owns (`capture` is
idempotent against observed heads), so nothing saved before the switch is
missing from the first publication. Git → Linear: journaling continues until
the next drain records `linear`; harmless. Both recordings are conditional
writes (`WHERE planning_tracker IS NOT ?`): the drain runs on every save and
tick, and must not take the write lock to restate a value. The value is a
`TrackerKind`.

Seeding happens in the bind transaction, before the first acquisition, and a
switched repository keeps its journal. Git → Linear → Git depends on both:
the kept observed heads are what was last exchanged, so the seed's captures
name them as parents and the Linear-era values win causally, while a peer's
change made in between still surfaces as a lost line. A pruned journal
would make every changed field a lost line, and seeding after the import
would let the remote's stale values overwrite Linear's silently.

This is a stored copy of one config fact. The rule in `docs/dev/planning.md`
("there is no stored selection") is about the Git destination; the
destination is still derived from config. The doc line gets updated to say
what is stored and why.

## Slices

1. **Journal only Git repositories** (teaches the most: proves the writer
   can learn the tracker without a config read, and that seeding at the
   switch keeps the first publication whole). Migration adds
   `repository_plans.planning_tracker`; `bind_peer_planning` records `git`
   and seeds on a change; the drain records `linear`/`asana`;
   `write_record`'s capture checks the record's repository. Tests: the
   acceptance check above; `planning_write` unit test that a Linear-tracked
   repository's local and tracker writes leave the journal unchanged.
   Check `EXPLAIN QUERY PLAN` for the per-write repository lookup
   (tasks → `repository_plans` by id; projects → waves.repo; comments →
   task) against a 10k-Task store.
2. **One exchange loop, gated by Git's own outbox.** Git's outbox is the
   journal changes owned by the repository with no `planning_peer_routes`
   row for the active destination (`unrouted_changes`). `PlanningSync`
   keeps one loop, at 5 s. Each tick publishes (which acquires first, as
   today) when the destination has an unrouted owned change or its
   `publication_state` is not `confirmed`, and otherwise only acquires.
   Delete the 1 s publication loop. Saves keep publishing after the reply
   (`sync_after_save`), so the loop is only the retry path: a failed
   publication waits at most 5 s. `start_for_directory` reads the planning
   config once and starts no thread unless it is Git. Passed over: gating on
   the store's `planning` revision. It is one revision for the whole store,
   and any Linear save or import in another repository moves it, so a Git
   Session beside a busy Linear repository would still fetch every second.
   Add the outbox check as a storage read (`EXISTS` over `unrouted_changes`'s
   join, `LIMIT 1`) and check its `EXPLAIN QUERY PLAN` against a 10k-Task
   store. Test: a foreground connection on an unchanged Git repository
   records no publication over several ticks, while saves in a second,
   Linear repository on the same store go on; then a save whose push fails
   is published by the next tick.
3. **Delete the rows nothing reads.** The same migration deletes
   `planning_peer_routes`, `planning_peer_observed` and `planning_peer_changes`
   rows whose record belongs to a repository with no `planning_destinations`
   row, and orphaned rows (no live record) only when they are unrouted.
   Routed orphans are history a peer holds; the routes table keeps them on
   purpose. Rows of switched repositories (manabot, kata,
   etude-studio) stay: SQL cannot read their config, and they stop growing
   once their tracker is recorded. Pruning them would break a switch back to
   Git (see Source of truth). Delete `planning_write::seed` from the
   migration runner if only a released migration's hook calls it and the
   hook can call `capture` per record instead. Report before/after.

## Docs written ahead

- `docs/work.md` (Share the plan: 5 s check, push only on change, switch publishes whole plan)
- `docs/dev/planning.md` (Git-only journal, stored `planning_tracker` and seed, outbox-gated 5 s loop)

## Affected surfaces

- `rust/loopflow-storage/src/sqlite/planning_write.rs` (capture gate),
  `planning_peers.rs` (record + seed on bind), `tracker_actions.rs` or
  `repositories.rs` (record the tracker), new migration via
  `scripts/new_migration.py planning_tracker`.
- `rust/loopflow-execution/src/ops/planning_sync.rs` (one loop, outbox gate,
  Git-only start), `loopflow-storage/src/sqlite/planning_peers.rs` (outbox
  read),
  `ops/tracker/drain.rs` (record linear/asana).
- `docs/dev/planning.md`: the journal is Git-only; publication is
  change-driven; the stored tracker. `rust/loopflow-storage/SCHEMA.md`
  regenerated.
- Unchanged wire: the Git document shape, `TrackerActionStatus`, Desktop.

## Absent and error states

- Tracker never recorded (fresh repository, no save yet): no journal; the
  first exchange seeds.
- Config unreadable: `planning_transport` treats it as no tracker; nothing
  is recorded, the stored value stays.
- Seed fails mid-way: it shares the bind transaction, so the tracker stays
  unrecorded and the next exchange retries.

## Operational boundary

- Per write: one indexed lookup for the repository's tracker. No new write.
- Seed: once per switch to Git, O(records) inside one transaction; loopflow's
  size (about 3k Tasks, 12k comments) is a few seconds under the write lock,
  once. Measure in slice 1; if it exceeds 2 s, seed per Wave in separate
  transactions before recording `git`.

## Exclusions

- The receipt tables and `export_*` columns: LOO-941.
- Git onto `tracker_actions`, and "open" Git lines in `lf repo plan`.
- Folding `planning_peer_observed` into a column, or dropping
  `planning_peer_routes`: they are the replica's own bookkeeping and shrink
  to Git repositories with this change.
- Linear-only fields in the Git document (`task_linear_id`, `task_url`,
  `task_team`, `project_initiatives`…): released peers read the document;
  noted in `scratch/questions.md`.
- Sharing the 5 s tick across processes, the way Linear's ten-second team
  stamp is shared.

## What goes

- Unconditional `capture` in `write_record` for `Local`/`Tracker` writes.
- The 1 s publication loop in `PlanningSync`, and its thread in
  non-Git repositories.
- About 26k journal rows and their observed/route rows in the live store.

## Inventory: what the product sends

| Change | Linear/Asana (`tracker_actions` kind) | Git journal | Verdict |
|---|---|---|---|
| Task/Project creation | `create` | every field of the record | keep |
| Title, description, assignee, Project membership | `field` | field mutation | keep |
| Project name, summary, description, status, KRs, metric targets | `field` (Asana: unsupported after creation) | field mutation | keep |
| Task order in a Project | `order` | `task_rank` | keep |
| State/disposition | `state` | `disposition` | keep |
| Comment | `comment` | `comment_task` + `comment_content` | keep |
| Deletion | `delete` | `task_deleted_at` | keep |
| Initiative, follow-up relation, PR link | `link` | — | keep |
| Linear ids, URL, team, branch, Initiatives/Teams | never sent (tracker facts) | journaled | Linear repos stop journaling them here; Git document shape unchanged (questions) |
| Any write in a Linear/Asana repository | its kind above | journaled today | stop journaling (slice 1) |

## Before (for the gate report)

- `SCHEMA.md` tables: 90. `pub fn` in `loopflow-storage/src`: 900. Files
  outside `loopflow-storage` with inline SQL: 99.
- Live store: 1.02 GB; `planning_peer_*` 30 MB; 43,995 / 32,753 / 6,665 rows.
