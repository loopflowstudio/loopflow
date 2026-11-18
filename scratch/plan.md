# LOO-979: a Session read finds its inputs through the live agents

## Outcome

`session_projection_inputs_in` (`rust/loopflow-storage/src/sqlite/sessions.rs`)
reads only Sessions whose agent has not ended, plus the unended Graph runs.
On this Machine's store that is 37 Sessions and 6 runs instead of 20,732
activity rows and 803 open Sessions twice over. Nobody sees a different
Session list; `lf server run` stops spending its busy time here.

Done when the all-Sessions inputs read costs tens of microseconds on a store
this size, the three statements appear in a query-plan test that fails on a
scan, and every Session summary reads the same as before.

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

**Identities** for the all-read take the same index: add `agent_process_id IS
NOT NULL AND agent_ended_at IS NULL` to the predicate. 37 rows, 29 µs.
An ended agent is Dead from its own row (`RecordedProcess::evidence_by`
returns Dead on `ended` before any OS lookup), so the 735 ended agents were
read only to say what the row already says. `SessionProjection::evidence`
defaults to Unknown for an agent missing from the map, and
`agent_process_evidence` is on the `SessionSummary` wire, so the summary
query selects `a.agent_ended_at IS NOT NULL` and `read_summary` reports Dead
for it before consulting the map. Holders of ended agents are never read
(`terminal_id` checks the holder only when the agent is Live).

One reading changes, made for review: a completed Session with an ended agent
reads Dead in an all-Sessions summary where it read Unknown (it was outside
`completed_at IS NULL`). The single-Session read already said Dead, so the two
now agree. Passed over: keeping Unknown there with a `completed_at` test in
`read_summary`.

**Graph runs** for the all-read are every unended run:
`WHERE g.ended_at IS NULL`, through `graph_processes_unended`. 6 rows, 3 µs.
This is a superset of "unended runs whose step captured an open Session's
input"; an extra run costs one OS observation and `graph_result` is looked up
by run id, so no summary changes. The single-Session statement stays.

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
- `store::query_plans::tests::` gains a test that traces
  `session_projection_inputs` for all Sessions and for one, each followed by
  `pin`, over the `retirement` fixture with activity recorded, and asserts
  `growing_scans` is empty. With today's statement it fails on
  `SCAN session_activity`.
- Adding the table to `GROWING` puts every other traced path's
  `session_activity` statements under the same check. A scan it finds
  elsewhere is fixed here when it is a one-line predicate, otherwise filed.
- `identity_acquisition_is_batched_and_indexes_out_completed_history`
  (`rust/loopflow-execution/src/store/session_projection.rs`) keeps its count
  of 6 SELECTs: three statements, twice.

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

No migration, no new index, no change to `store_revisions` triggers, to
`available_sql`, or to the 120 s quiet rule. The summary read itself
(`summary_query`) is not re-planned here.

## What goes

- The `observed_at + ?1 > ?2` statement.
- The all-read's list subquery over every open Session for Graph runs.
- Evidence and holder map entries for ended agents.

What survives: the deadlines a subscription wakes on, `pin` returning `None`
when an identity changed between observation and projection
(`a_changed_identity_restarts_acquisition_before_projection`), Dead for an
ended agent in every summary.

## Slices

1. **The failing plan test, then deadlines.** Add `session_activity` to
   `GROWING` and `MEASURED`, write the test, watch it fail on the scan, replace
   the deadline statement. Shows whether adding the table flags statements in
   other traced paths, which is the one thing that could widen this change.
2. **Identities through the unended-agent index**, with Dead read from the row
   in `read_summary`. Run `session_projection::` and the Session CLI tests that
   assert `agent_process_evidence`
   (`grep -rn agent_process_evidence rust/loopflow/tests`).
3. **Graph runs as every unended run.**
4. Re-measure the three statements on a copy of the store and put before and
   after in the PR body.

## Demo

`sample` of `lf server run` under the same load no longer shows
`session_projection_inputs_in` near the top; headless, the timing script's
all-read total drops from about 6,200 µs to about 50 µs.
