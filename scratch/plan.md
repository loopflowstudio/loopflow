# LOO-979: a Session read finds its inputs through the live agents

## Outcome

`session_projection_inputs_in` (`rust/loopflow-storage/src/sqlite/sessions.rs`)
reads only Sessions whose agent has not ended, plus the unended Graph runs.
Its three lists then mean one thing each: the agents the OS must be asked
about, the Graph runs the OS must be asked about, and the times an answer
changes with no write.
On this Machine's store that is 37 Sessions and 6 runs instead of 20,732
activity rows and 803 open Sessions twice over. Nobody sees a different
Session list; `lf server run` stops spending its busy time here.

Done when the all-Sessions inputs read costs tens of microseconds on a store
this size, the three statements appear in a query-plan test that fails on a
scan, and every Session summary reads the same as before, but for the one
reading named under Identities.

## What the store showed (2026-11-18, `~/.lf/loopflow.db`, read-only, SQLite 3.51)

22,643 Sessions, 803 open, 772 open with a started agent, **37 with an unended
agent**; 20,732 `session_activity` rows (one per Session: `session_id` is its
primary key), 13 observed in the last 120 s; 2,843 Graph runs, 6 unended.

All-Sessions read, per statement, mean of 300 warm runs:

| statement | rows | time | plan |
|---|---|---|---|
| deadlines | 13 | 2,443 µs | `SCAN session_activity` + temp B-tree for DISTINCT |
| identities | 772 | 1,630 µs | `SEARCH agent_sessions USING INDEX session_inventory (ANY(interactive) AND completed_at=?)` + temp B-tree for ORDER BY |
| Graph runs | 6 | 2,093 µs | `SEARCH g` by primary key from a list subquery over every open Session (`session_inventory`), two correlated lookups each, bloom filter |

About 6.2 ms a call, run twice per projected read (`pin` → `is_current`).
The deadline scan is 40% of it; the other two statements cost as much
together, so fixing only the scan would leave 60% in place. None of the
three shows as a growing-table scan to `growing_scans`: `session_activity` is
not in `GROWING`, and the other two are index searches that happen to visit
every open Session.

Single-Session read: identities 2 µs, Graph runs 4 µs, both by primary key.
Only deadlines (the same 2.4 ms scan) is wrong there.

Moving the bound across (`observed_at > ?2 - ?1`) alone changes nothing: with
no index on `observed_at` the plan is the same scan, 2,578 µs.

## Design

One driver for the all-Sessions read: the partial index
`agent_sessions_unended_agent (agent_process_id) WHERE agent_process_id IS NOT
NULL AND agent_ended_at IS NULL`, which `MEASURED` counts at 17 rows and this
store at 37. It is bounded by how many agents run at once.

**Deadlines** join from that index to `session_activity` by primary key, and
filter by Session when the read is for one:

```sql
SELECT DISTINCT act.observed_at + ?1 FROM agent_sessions s
JOIN session_activity act ON act.session_id=s.id
WHERE <s.id=?3 | s.agent_process_id IS NOT NULL>
  AND s.agent_ended_at IS NULL AND s.completed_at IS NULL
  AND act.agent_process_id IS s.agent_process_id
  AND act.observed_at > ?2 - ?1 ORDER BY 1
```

The guard that makes an activity row count (`act.session_id=s.id`, open
Session, the Session's current agent, agent not ended) is the same one
`available_sql` opens with. It is written once, as a function of the Session
alias beside `available_sql`, and both statements use it: the rule and the
time it next changes its answer cannot drift apart. The other three copies of
the join (`TURN_LIVE`, `summary_query`, `session_reports_status`) each lack one
of its clauses on purpose and stay.

17 µs for all (7 rows), 2 µs for one. The meaning holds: `available_sql`
reads quiet time only from an activity row of the Session's current agent, on
an open Session whose agent has not ended, and `read_summary` reports
`available` only when that agent is Live. A row outside this join cannot
change any Session's availability, so it owes nobody a wakeup. The 13 → 7 drop
is activity rows of ended agents.

Passed over: `CREATE INDEX … ON session_activity(observed_at)`. It serves the
all-read (not the single one) and it is rewritten on every activity reading,
every few seconds per streaming agent, under the Machine's one write lock.
The join needs no migration.

**Identities** are the agents that have not ended, in both reads: add
`agent_ended_at IS NULL` to the statement, and `agent_process_id IS NOT NULL`
to the all-read's predicate so it takes the same index. 37 rows, 29 µs.

An ended agent is Dead because its row says so, and no OS answer can change
that. Today that fact travels out of SQL as
`SessionProcessIdentity.completed_at` (which holds `agent_ended_at`), through
`RecordedProcess { ended }`, into the evidence map and back into
`read_summary`, for 735 of the 772 identities. After: the summary query
selects `a.agent_ended_at IS NOT NULL` and `read_summary` reports Dead for it
before consulting the map, for the all-read and the single read alike.
`SessionProcessIdentity.completed_at` is deleted and `Reading::acquire` passes
`ended: false`. The evidence map then holds only what the OS was asked.
`SessionProjection::evidence` still defaults to Unknown for an unended agent
missing from the map. Holders of ended agents are never read (`terminal_id`
checks the holder only when the agent is Live).

Chosen for review, over the kickoff plan's narrower change: the single-Session
read stops returning ended agents too. Passed over: narrowing only the
all-read, which leaves two ways to learn that an ended agent is Dead (the row
in one read, `RecordedProcess.ended` in the other).

One reading changes: a completed Session with an ended agent reads Dead in an
all-Sessions summary where it read Unknown (it was outside
`completed_at IS NULL`). The single-Session read already said Dead, so the two
now agree. Passed over: keeping Unknown there with a `completed_at` test in
`read_summary`.

**Graph runs** for the all-read are every unended run:
`WHERE g.ended_at IS NULL`, through `graph_processes_unended`. 6 rows, 3 µs.
This is a superset of "unended runs whose step captured an open Session's
input"; an extra run costs one OS observation and `graph_result` is looked up
by run id, so no summary changes. The single-Session statement stays: one
Session's read should not ask the OS about every running Graph. The field's
doc comment says which runs each read carries.

After: about 50 µs for the all-read, both runs of it under 0.1 ms.

**`pin`'s currency check stays as it is**: rerun the inputs, compare. A
cheaper comparison would be the `sessions` and `graphs` rows of
`store_revisions`, but `store_revision_agent_sessions_update` fires only
`WHEN OLD.fence_locator IS NOT NULL`, so an identity write can pass without a
revision; using it means changing triggers. With the rerun at ~50 µs there is
nothing left worth that. The PR says so.

## Query-plan test

- `session_activity` joins `GROWING` in `rust/loopflow-storage/src/query_plans.rs`
  (one row per Session, forever). Its `sqlite_autoindex_session_activity_1`
  row goes into `MEASURED` as `'20732 1'` with the table in the `DELETE` list,
  and the doc comment says where the number came from.
- No new test in `store::query_plans::tests::`.
  `identity_acquisition_is_batched_and_indexes_out_completed_history`
  (`rust/loopflow-execution/src/store/session_projection.rs`) already traces
  the three statements and `pin`, for all Sessions and for one, through
  `growing_scans`. With the old deadline statement restored it fails on
  `SCAN session_activity` (run 2026-11-18), so a second trace of the same
  statements was deleted in compress.
- Adding the table to `GROWING` puts every other traced path's
  `session_activity` statements under the same check. A scan it finds
  elsewhere is fixed here when it is a one-line predicate, otherwise filed.
- A deadline test beside `quiet_deadline_repages_sessions_and_task_selections_without_a_write`:
  two Sessions with fresh activity, one whose agent has ended. The all-read
  carries one deadline, the live Session's; a single read of the ended one
  carries none; a single read of the live one carries its own.
- That test keeps its count of 6 SELECTs: three statements, twice.

`graph_processes` is left out of `GROWING`: the all-read walks
`graph_processes_unended` in full by design, and `growing_scans` would need
that index measured and every other Graph statement audited. Noted in
`scratch/questions.md`.

## Acceptance check at gate

```
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow-storage --lib query_plans::tests:: < /dev/null
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow-execution --lib session_projection:: < /dev/null
```

Both pass. And on a copy of a Machine store, `EXPLAIN QUERY PLAN` of the three
statements names `agent_sessions_unended_agent`, `graph_processes_unended` and
`sqlite_autoindex_session_activity_1`, with no bare `SCAN`; the PR body carries
the before and after plans and times from this file.

## Absent and error states

- Session with no activity row, or one from an earlier agent: no deadline, as now.
- Session with no agent yet (`agent_process_id` NULL): no identity, evidence
  Unknown, as now.
- Unknown single Session id: empty inputs, as now.

## Exclusions

No migration, no new index, no change to `store_revisions` triggers, to what
`available_sql` answers, or to the 120 s quiet rule. The summary read itself
(`summary_query`) is not re-planned here.

## What goes

- The `observed_at + ?1 > ?2` statement.
- The all-read's list subquery over every open Session for Graph runs.
- Evidence and holder map entries for ended agents, in both reads.
- `SessionProcessIdentity.completed_at`.

What survives: the deadlines a subscription wakes on, `pin` returning `None`
when an identity changed between observation and projection
(`a_changed_identity_restarts_acquisition_before_projection`), Dead for an
ended agent in every summary.

## Slices

All three statements, Dead from the row and both tests are built. The two
commands under "Acceptance check at gate" ran here after compress: 13 and 5
tests, all passing. Left:

- Gate runs the suites and clippy; the PR body carries the plans and times below.
- `binary(session_cli_tests)` and `test(performance_tests::)` were not run
  here; they read `agent_process_evidence` through the CLI.

What the build showed (2026-11-18):

- Adding `session_activity` to `GROWING` flagged only the deadline statement.
  Every other `query_plans::tests::` path passed unchanged, so the change did
  not widen.
- The all-read statements carry `INDEXED BY agent_sessions_unended_agent`:
  `completed_at IS NULL` in the deadline guard would otherwise let the planner
  choose `open_sessions`.
- The all-read's identities drop `completed_at IS NULL` as well as gaining the
  two agent clauses, so "the agents that have not ended" is the whole rule.
- The summary query no longer selects `a.attached_pid` and
  `a.attached_os_started_at`; nothing read them.
- The deadline test is `quiet_deadlines_come_only_from_agents_that_have_not_ended`
  in `store/session_projection.rs`, next to the other reads of the inputs,
  not in `observe.rs`.
- `~/.lf/loopflow.db`, read-only, mean of 300 warm runs, all-Sessions read:

  | statement | before rows | before | after rows | after | after plan |
  |---|---|---|---|---|---|
  | identities | 771 | 1,278 µs | 36 | 27 µs | `SEARCH s USING INDEX agent_sessions_unended_agent (agent_process_id>?)` |
  | Graph runs | 5 | 2,034 µs | 5 | 3 µs | `SCAN g USING INDEX graph_processes_unended` + temp B-tree for ORDER BY |
  | deadlines | 11 | 2,221 µs | 7 | 16 µs | `SEARCH s … agent_sessions_unended_agent`, `SEARCH act USING INDEX sqlite_autoindex_session_activity_1 (session_id=?)`, temp B-tree for DISTINCT |
  | total | | 5,533 µs | | 46 µs | |

- Under plain `cargo test` with several filters,
  `ops::human_session::tests::resuming_an_ended_conversation_launches_its_provider_as_the_session`
  failed once with "Native launch has no admitted invocation" and passed
  twice alone. It sets the process `PATH`; whether it shares state with
  another test in one process was not looked into. Nextest runs each test in
  its own process.

## Demo

`sample` of `lf server run` under the same load no longer shows
`session_projection_inputs_in` near the top; headless, the timing script's
all-read total drops from about 6,200 µs to about 50 µs.

## Left for later, larger than this change

`scratch/concept-review.html` shows these against the product concepts.

- The 120 s quiet rule is written three times: `available_sql`, the deadline
  statement, and a literal `120` in `store_revision_session_activity_update`.
  The first two share `AVAILABILITY_QUIET_SECONDS`; the trigger is in a merged
  migration and changing the rule means a new one.
- `pin` reads the inputs twice because `store_revisions` cannot vouch for
  them (see the currency check above).
- No wakeup comes from a narrower deadline list by accident: a view that
  relied on an ended agent's activity row to re-read would now wait for a
  write or a process death. None is known; `process_death_invalidates_session_pages_and_task_selections_without_a_write`
  and the quiet-deadline test cover the two real wake sources.

## Docs written ahead of the code

- `docs/dev/reference-models.md`, "Conversation and attachment lifetime": an
  agent whose end is recorded is dead by its row in every read, finished
  Sessions included; the OS is asked only about unended agents.
- `docs/dev/data.md`, after the measured-statistics paragraph: a
  one-row-per-Session table is growing; read across Sessions from the live
  subset and join by primary key. Plan text there was run against
  `~/.lf/loopflow.db` on 2026-11-18.

No user guide, README or command reference changes: no command, flag or output
field does.
