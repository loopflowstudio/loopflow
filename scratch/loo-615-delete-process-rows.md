# LOO-615 — Process rows live while running, kept only when referenced

Status: **implemented; gate remains** (2026-10-26). Remaining:
`store_contention` measured on a quiet Machine at gate (slice 5 "Open findings"). Product decision
accepted by Jack Heart in LOO-602 decision 5 (2026-10-26): write a Process row
for every lf process so the live tree works; delete it at exit unless a child,
a Session event or a Graph step references it, or an agent issued it. This
replaces `LfCli::records_process` and `PROCESS_RETENTION_SECONDS`.

## Problem

`processes` holds 945k rows (434 MB + 0.31 GB indexes). Two mechanisms keep it
in check today and both are wrong in shape:

- `Cli::records_process` (`lf/mod.rs:217`) is a denylist of commands that never
  get a row, so they are also invisible in the live tree while running. Every
  new poll or hook has to remember to join it.
- `expire_processes` deletes unreferenced finished rows after 7 days, and
  `forget_unrecorded` re-parses every stored argv through Clap to apply the
  denylist retroactively.

Measured on Jack's store (read-only, 2026-10-26):

| class | rows |
|---|---|
| agent Processes (`kind='agent'`) | 6,497 |
| agent-issued lf Processes (`via_agent=1`) | 82,475 |
| lf Processes something references | 12,682 |
| unfinished, unreferenced (mostly crash leftovers) | 3,081 |
| **finished, unreferenced, not agent-issued** | **840,668 (89%)** |

After this change the store keeps ~105k rows. Growth is then dominated by
agent-issued commands: ~1k/day excluding hooks, ~150 MB/year at today's row
size. That is above LOO-602's 50–60 MB/year estimate, and LOO-602 decision 5
keeps those rows on purpose.

## Demo

```
lf task status LOO-615 &  lf ps            # the running status read appears in the live tree
lf task status LOO-615; lf history processes --search 'task status' --limit 1
                                            # → nothing new: it finished unreferenced
lf -b skill commit …                        # agent-issued children stay listed under the Session
lf disk clean                               # first pass on a clone of the live store: processes 945k → ~105k
```

Clone probe: `lf clone`/`cp -c ~/.lf/loopflow.db` into a temp `LF_HOME`, run the
branch `lf disk clean`, then `select count(*) from processes` drops by about
840k, and `pragma foreign_key_check` stays empty.

## Chosen approach

### One rule, one SQL predicate

A finished `lf` Process is **forgettable** when all of these hold:

1. `kind='lf'`, `completed_at IS NOT NULL`, `via_agent IS NOT 1`;
2. its parent is absent or finished (`parent_lf_process_id` is NULL or names
   no unfinished row), because a running parent may still read its child's
   result;
3. no row references it: no child (`processes.parent_lf_process_id`), no
   attached AgentProcess (`attached_lf_process_id`), and none of
   `session_events.lf_process_id`, `graph_processes.lf_process_id`,
   `graph_process_steps.{lf_process_id,graph_lf_process_id}`,
   `task_workflows.lf_process_id`, `task_workflow_moves.lf_process_id`,
   `waves.project_activation_lf_process_id`, `ci_incidents.repair_lf_process_id`,
   `tasks.ship_lf_process_id`, `agent_sessions.transport_lf_process_id`,
   `relocated_task_processes.process_id`.
   (`pr_landings.supervisor_process_id` is an OS PID `INTEGER`, not a Process
   reference; implement dropped it from the union.)

This is today's `expire_processes` reference union without the age cutoff,
plus rule 2. Write it once in `store/sqlite/processes.rs` as a `NOT EXISTS`
fragment over alias `p` (`FORGETTABLE`). The exit path and the prune share it.
Agent-side references (`steer_*`, `session_activity`,
`agent_sessions.agent_process_id`, `caller_agent_process_id`) point only at
`kind='agent'` rows, and those rows are never forgotten.

### At exit (`SqliteStore::record_process`)

When the recorded row carries `completed_at`, the same transaction that writes
the terminal upsert also:

1. deletes this Process's finished children that are now forgettable (it was
   the running parent that kept them);
2. deletes this Process if it is forgettable.

`ledger_insert` keeps calling `record_process_within`, and receipt removal stays
as it is. Interrupt cleanup (`record_process_interruption`) goes through the
same writer.

### Exit-only observation writes only what it may keep

Commands that never admit (help, rejected args, previews, early failures) are
observed only at exit (`journal::with_process`). Nothing can reference a
Process that had no row while it ran, so such a row survives only when an agent
issued it or a running lf parent may still read it (rule 2: a Graph step that
fails at argument parsing). Exit observation therefore runs only when
`LF_AGENT_CALLER` or `LF_PROCESS_ID` is inherited. Explicit early observation
(`lf open`, installation bypass) is unchanged: it stands in for admission.

### Harness callbacks are not agent-issued

Provider hooks inherit the provider's `LF_AGENT_CALLER` and the launching lf's
`LF_PROCESS_ID`. Today 3k of 5k agent-attributed rows in two days are
`__context-block`. Implement found that the harness callbacks are admitted
(general admission precedes `execute_command`), so the drop happens right after
parsing, not at the dispatch arm: `Cli::harness_callback` (the three hidden
callbacks plus stale `__` hooks) → `journal::harness_issued()` removes both
variables. A hook is then a root Process, listed while it runs and forgotten at
exit. None of them reads either variable.

### Prune backstop (`retention.rs`)

`expire_processes` + `forget_unrecorded` + `records` + the `processes` cursor
become one `forget_processes(conn)`: batched `DELETE … WHERE id IN (SELECT p.id
FROM processes p WHERE <FORGETTABLE> LIMIT 10000)` until a batch deletes
nothing (a parent becomes forgettable once its last child goes). It runs after `close_dead_processes`, so crash-closed rows go the same
way. It also collects rows left behind when a child outlived its parent. The
first pass on Jack's store deletes ~840k rows. `RetentionReport.processes`
keeps its name and counts forgotten rows.

### Indexes (one draft migration)

A `NOT EXISTS` per candidate needs an index on every referencing column, or the
first bulk pass costs about 840k × 20k row reads. Add the missing indexes in
this Task's draft (`uv run python scripts/new_migration.py forget_processes`):
`agent_sessions(agent_process_id)`, `task_workflows(lf_process_id)`,
`task_workflow_moves(lf_process_id)`, `waves(project_activation_lf_process_id)`,
`ci_incidents(repair_lf_process_id)`, `tasks(ship_lf_process_id)`, each partial
`WHERE … IS NOT NULL` (written). `graph_process_steps.lf_process_id` is already
`UNIQUE`. `agent_sessions_transport` was released in 0.14.16; do not recreate it. Already indexed:
`processes_parent`, `agent_process_attachment`, `session_events_process`,
`graph_processes` PK, `relocated_process_tasks`. The `agent_sessions` index also
removes the parent-side FK probe on every exit delete.

### Desktop Task-content reads run outside the checkout (consumer cutover)

A Task's work is every Process in its checkout (`task_open_processes` →
`execution_blockers`). Desktop runs `task diff/files/file` and saves with
`cwd` = the Task checkout: up to 79k rows per checkout historically. Once they
are recorded, each poll is an open Process in the checkout for its lifetime. A
`lf land` or completion check that overlaps one then refuses with "live or
unresolved execution". The denylist was hiding this.

These commands address the Task by issue and work from any cwd (probe:
`lf task file LOO-615 AGENTS.md --json` and `lf task diff … --files` from
`/tmp` both succeed). In `swift/Loopflow/Services/RegistryQuery.swift`, the Task
content calls (`taskChanges`, `taskDiff`, `taskFiles`, `taskFile`,
`saveTaskFile`) pass `nil` cwd, as `watch-files` and `location` already do.
Their callers stop passing the checkout. Their rows then sit in the live tree
and are not Task work.

Audited the other Desktop Task reads (kickoff, 2026-10-26). `taskComments`,
`taskHistory` and `taskContext` run from `WaveOrigin.resolve(repo)`, the
repository origin. `taskGraphProcesses` (Debug in `TaskWorkView`) now also runs
with `nil` cwd; `graph list --processes --for-task` names the Task.

## Risk findings

- **Graph step race (resolved by rule 2).** `RunningGraph::spawn` finds its step
  by polling `child_process_after` every 10 ms and once after exit. A step that
  finishes first would delete itself before `record_graph_step` names it, and
  the Graph would lose its result. `lf task run` attempt registration (50 ms)
  has the same shape. Rule 2 keeps the finished child while the Graph or
  `task run` process is still running. A focused test must cover an operation
  step that exits before the first poll.
- **New Process references.** `FORGETTABLE` is a hand-kept inventory; a
  migration that adds a `processes` reference without extending it makes that
  writer's exit fail (FK) or forget a referenced row. Gate greps the store
  schema for `REFERENCES processes` / `_process_id` columns against it.
- **A reference written after its Process exited.** The exit forgets the row
  first, so the later reference fails its FK (`agent_sessions`, `relocated_task_processes`)
  or names a missing row (the rest have no FK). `lf-world` did exactly this and
  failed. Live writers that record `task_workflows`, `graph_processes`,
  `ci_incidents.repair_lf_process_id` or `session_events` from a process other
  than the referenced one, after it exits, have not been audited.
- **Detached child after parent exit.** A parent that exits before its detached
  child registers is forgotten first. `parent_is_recorded` then drops the
  child's parent link. The child's own row survives, and only the edge is lost.
  `task run` waits for registration, so no current caller depends on that edge.
  Accepted.
- **Children of long-lived parents accumulate** until the parent exits
  (`__transport-serve`, `repo ci watch`). Bounded by the parent's lifetime. Gate
  measures `select count(*) from processes c join processes p on
  c.parent_lf_process_id=p.id where c.completed_at is not null and p.completed_at is null`
  on the clone.
- **Revision churn.** Insert and delete each bump `store_revisions.processes`,
  and every Work-watch part except PeerPlanning recomputes on it
  (`work_watch.rs:245`). Recorded Desktop polls add two bumps each. Gate checks
  that `lf work watch` emits no new frame for an unchanged Task while
  `lf task diff` runs 20 times from outside the checkout. If it does, the fix
  is in watch-frame dedup, not a new denylist.
- **Exit latency.** One extra `DELETE` with indexed `NOT EXISTS` probes inside
  the existing terminal transaction stays inside `receipt_wait`.
  `tests/store_contention.rs` keeps passing unchanged.

## Alternatives considered

- *Keep a smaller denylist for hooks.* Rejected: it is the mechanism being
  deleted, and exit-only observation plus harness provenance get the same
  result from facts.
- *Age-based expiry without rule 2.* Rejected: a 7-day window is what hid the
  Graph step race, and it leaves 89% of rows in place for a week.
- *Make `execution_blockers` ignore read commands.* Rejected: that moves the
  denylist into the blocker. Desktop does not work in the checkout, so its
  Processes should not run there.

## Current → target

| | Before | After |
|---|---|---|
| Which lf processes get a row | all except `records_process` denylist | every admitted process, while running |
| When rows go | 7 days after finish, or at any age when denylisted (prune) | at exit when forgettable, plus prune backstop |
| Hooks | denylisted | harness-issued roots: listed while running, forgotten at exit |
| Desktop Task reads | denylisted, cwd = checkout | recorded briefly, cwd outside checkout |
| Reference inventory | temp table in `expire_processes` | `FORGETTABLE` fragment, used at exit and in prune |

## Delete — do not maintain

- `Cli::records_process` (`lf/mod.rs:212–246`) and both call sites in
  `bin/lf.rs` (remote path `1546`, local path `1585`). `mark_effect_free_read`
  stays for its real uses (machine validation, unsupported Desktop, clone).
  Narrow its doc to "touches no store".
- `retention.rs`: `PROCESS_RETENTION_SECONDS`, `expire_processes`,
  `forget_unrecorded`, `records`, the `processes` cursor file and the
  `clap::{CommandFactory, FromArgMatches}` import. Rework the unit tests that
  cover them (`prune_keeps_facts_…` process half, the `__context-block`
  `forget_unrecorded` test near line 734) to cover the rule.
- `doctor.rs`: `ledger_gap_days`, `format_gap_summary`, `format_gap_dates`, and
  their tests. Once inspection rows vanish at exit, a day without rows no
  longer means the Machine was off. The cron-receipt continuity check stays.
- `docs/troubleshooting.md:145–149`: the "expires … older than seven days" and
  "never recorded as Processes" sentences. Replace them with the rule.
  `docs/architecture-reference.md` row "One Process per actual lf process" gets
  one sentence on retention.

Must survive: Process ancestry for retained rows, `close_dead_processes`, Graph
step results, agent-issued history (`lf history processes --caller`), every
referenced row (FK check stays empty).

## Forbidden outcomes

- Any per-command list deciding whether a process is recorded or kept.
- An age-based expiry alongside the rule.
- Two copies of the reference inventory (exit and prune must share `FORGETTABLE`).
- Exit-only observations written and then deleted (revision churn for nothing).
- Desktop Task reads counted as Task execution.

## Internal slices

One PR; the slices are an order of work.

1. **This slice:** `FORGETTABLE` + exit deletion in `record_process` + draft
   indexes. Focused test in `store/sqlite/processes.rs`: a finished human row is
   gone; agent-issued, parented-by-running, child-holding, session-event- and
   graph-step-referenced rows stay; finishing the parent removes its finished
   unreferenced child.
2. Delete `records_process`; exit-only observation writes only agent-issued;
   harness callbacks drop the caller. Journal test: an exit-observed process
   without a caller leaves no row, and one with a caller leaves one.
3. Prune: `forget_processes` replaces the expiry. Retention test: a
   crash-closed unreferenced row at any age goes; a finished child of a running
   parent stays.
4. Desktop cutover in `RegistryQuery.swift` and its callers.
5. Doctor gap deletion, docs, and the integration-test sweep (done). Tests
   that read a finished top-level command's row either assert absence or run it
   under a running fixture parent (an unfinished `processes` row exported as
   `LF_PROCESS_ID`/`LF_TRACE_ID`), which rule 2 keeps. `lf-world` now records
   each Process running, writes its references, then finishes it, as live
   commands do. Before that it hit FK failures after the exit forgot
   `task run`/Graph/mover rows. `journal::tests::a_detached_body_keeps_its_trace_after_its_launcher_is_forgotten`
   now asserts the accepted edge loss described under "Detached child after
   parent exit". The fast-step Graph test is
   `graph_tests.rs::a_step_that_finishes_before_the_graph_looks_keeps_its_result`:
   a step rejected at argument parsing first appears at exit, and the Graph
   still records it and stops on its failure.

### Open findings

- **`store_contention` under machine load.** It failed twice inside the full
  integration run (40 and 54 of 1020 first writes `database is locked`, load
  average 28–65 from other worktrees' suites) and passed alone in 28.5 s.
  `record_process` now takes an `IMMEDIATE` transaction and runs two
  `FORGETTABLE` deletes. Base has not been run under the same load, so this
  regression is unconfirmed; gate runs base and branch back to back. Its fixture
  is now agent-issued, so the row count still proves exactly-once while every
  write still pays for the delete probe.
- Flaked once under the same load and passed on rerun:
  `auth_tests::banked_resets…`, `context_launch_tests::codex_startup_failures…`,
  `release_lock_tests::surviving_release_auto_merge_child…`,
  `land_tests::waited_task_landing_completes_at_workflow_end…`.

Resolved at kickoff (2026-10-26):

- *Session-connect diagnostic.* The failed-launch evidence lives in the
  `spawn_session_process` error, and stderr carries all of it, run id included
  (`Session input {artifact_key}`). The test now asserts it on stderr. Passes.
- *Journal tests reading a forgotten root.* They run under
  `adopt_by_running_parent`, which rule 2 keeps. `--lib journal` passes (30).
- *A forgotten Process answers `Unknown`.* Kept: a missing row proves nothing
  about exit. Every durably held id is in `FORGETTABLE`'s inventory and stays.
  The only in-memory holders are running parents: the `pr_landing` repair
  launcher polling its `__transport-serve` owner, Graph step polls, `task run`.
  Each child inherits `LF_PROCESS_ID`, so rule 2 keeps its finished row, and
  its `Dead` answer, until the parent exits.

## Done when

- A non-agent `lf task status` appears in `lf ps` while it runs and leaves no
  row after it exits; an agent-issued command's row stays under its Session.
- A Graph whose operation step exits within milliseconds still records that step
  and routes on its exit.
- `lf disk clean` on a clone of Jack's store leaves ~105k Process rows with an
  empty `foreign_key_check`.
- `lf land` in a Task checkout is not refused by Desktop's file and diff reads.

## Acceptance at gate (headless)

With inherited `LF_*` unset and `< /dev/null`:

- `cargo test -p loopflow --lib -- processes retention journal doctor graph_tests task_graph_launch local_planning work_watch`
  — rule, exit deletion, exit-only observation, prune backstop, fast-step Graph
  keeps its step. (`tests/linear/*` compile into the lib target via `#[path]`,
  not `--test integration`.)
- `cargo test -p loopflow --test integration store_contention` on a quiet
  Machine, against base and branch — contention budget unchanged.
- `swift test --filter RegistryQuery` (or the Desktop target per TESTING.md):
  Task content calls carry no checkout cwd.
- Clone probe above: row count and FK check as stated, plus the
  long-lived-parent count recorded in one line.

Realign (2026-10-26): v0.14.17 (main 8ad349a841, merged) added
`tasks.ship_lf_process_id REFERENCES processes(id)`; it was missing from
`FORGETTABLE`, so a ship process's exit delete would fail its FK. Added the probe,
its index in the draft, and a shipped-Task case in `forget_tests`. The draft now
builds on the 0.14.17 release batch. Any later column that references
`processes` must join `FORGETTABLE` in the same change.
Realign (2026-10-27): origin/main has nothing past base 66ad233139; every
`processes` reference in the migrations (renamed `flow_lf_process_id` →
`graph_lf_process_id` included) is in `FORGETTABLE`. Plan and code agree.

Compress (2026-10-27): journal tests reuse `RunningParent`; clippy clean, `--lib journal` 30 ok.
Check (2026-10-27, after merge of main): `--lib -- processes retention` 26 ok, `linear_on_demand_native_hooks_share_freshness_across_processes` hit its 5 s deadline at load 66 and passed alone.
Check (2026-10-26): `swift test --filter 'RegistryQueryTests|TaskFilesTests'` 43 ok;
`--lib journal` 30 ok. Compress (2026-10-26): running-parent fixture shared as `loopflow_test_support::RunningParent`; doctor's test-only `audit` dropped; clippy clean, `--lib doctor failed_placement` 13 ok, `process_ownership_tests` 12 ok, touched integration tests 8 ok. Earlier sweep: everything else touched passes except
`store_contention` under load. The clone probe and the quiet contention
comparison are deferred to gate.
