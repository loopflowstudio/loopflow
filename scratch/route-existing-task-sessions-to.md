# W2-243 — Route existing Task Sessions to the successor Project Session

## Directive (v1, the contract)

When a Project Session is abandoned and replaced append-only, existing Task
Sessions under that Linear Project must route later observations, wakeups,
review requests, and status-triggered reconciliation to the **current
successor** rather than the terminal predecessor. Preserve historical
`project_session_id` provenance without making it the live routing key.

Done when:
1. deterministic predecessor+Task+successor tests prove only the successor wakes;
2. reviews assigned before replacement remain discoverable/completable;
3. `lf task status` emits no terminal-session wake warning;
4. stale/missing successor chains fail actionably;
5. CLI and DB evidence expose historical owner and current routing target.

## User-visible outcome

A Task created under a Project Session that was later abandoned (and replaced by
a successor for the same Linear Project) keeps running. Its new observations wake
and are consumed by the **successor** Project; its gate reviews are conducted by
the successor; `lf task status` shows the historical owning session *and* the
live routing target with no "terminal wake" warning. If the chain is broken (the
recorded Project Session is terminal and no live successor exists), routing
fails with one actionable error naming the project and the dead session.

## Source of truth

- **Historical owner** (provenance): `task_sessions.project_session_id` — the
  session the Task was born under. Never rewritten. Not the live routing key.
- **Live routing target**: resolved at read time from the `project_sessions`
  chain for the Task's Linear `project_id` (`task.launch.project.id`). The
  current successor is the non-terminal session with that `project_id`; the
  terminal predecessor stays in the table as history (the
  `0.11.002_project_session_successors` partial unique index already keeps one
  current successor per project).
- **Observation outbox** (`observation_outbox`): `recipient_id` stays the
  historical `project_session_id` (provenance). Routing is resolved by the
  successor consuming the whole project chain, not by rewriting the recipient.

## Routing resolver

New `ops::project::route::resolve_task_project_route(store, task) ->
TaskProjectRoute`:

```
TaskProjectRoute {
    historical: ProjectSessionId,   // task.project_session_id
    current:    ProjectSessionId,   // live routing target
    current_status: ProjectSessionStatus,
    succeeded:  bool,               // historical != current
}
```

Logic:
1. Load `historical = get_project_session(task.project_session_id)`.
   - Missing → actionable error: "Task's Project Session {historical} is not
     registered; cannot route."
2. If `historical` is non-terminal → `current = historical`, `succeeded = false`.
3. If `historical` is terminal → query the chain by Linear project id:
   `get_project_session_by_project(task.launch.project.id.as_str())` returns the
   latest session for the project.
   - Latest is non-terminal and differs from `historical` → `current = latest`,
     `succeeded = true`.
   - Latest is `historical` itself, or latest is terminal, or absent →
     **actionable failure**: "Task's Project Session {historical} is
     {terminal-status}; no live successor exists for project {id} ({slug}).
     Resume or restart the Project (`lf project run {slug}`)."

The resolver is the single routing key. Every surface below calls it; none reads
`task.project_session_id` as a live target.

## Affected surfaces and consumers

### 1. Observation wake → successor (criterion 1)

`store::child_sessions::append_task_event` and `append_task_event_for_lease`
(`store/child_sessions.rs:694,750`) currently call
`wake_project_session(&session.project_session_id)`, which silently no-ops on a
terminal predecessor via `supervisor_restart_bar`.

Replace with `ops::project::wake_task_project_route(store, &session)`:
- resolve the route;
- wake `route.current` (the successor);
- on actionable failure (no live successor), log at `warn!` with the resolver
  reason and return `Ok(())` — the observation is already enqueued to the
  historical recipient and will be drained by a successor once one exists
  (chain consumption below). The wake is best-effort; the error is surfaced in
  logs, not swallowed.

The terminal predecessor is never woken. Only the successor wakes.

### 2. Observation consumption → chain query

`project_session::runner::consume_task_observations` (`runner.rs:862`) currently
reads `pending_observations(&Project{session.id})` — only observations addressed
to the successor's own id. Observations from Tasks born under the predecessor
are addressed to the predecessor id and never seen.

New store method `pending_project_observations_for_chain(successor)`:
`WHERE recipient_kind='project' AND recipient_id IN
 (SELECT id FROM project_sessions WHERE project_id = ?successor.project_id)
 AND delivered_at IS NULL ORDER BY id`. The successor consumes the whole chain.

`consume_task_observation_for_project_with_lease` (`sqlite/child_sessions.rs:2360`)
currently requires `recipient_id == project_session_id`. Relax to: the recipient
is a Project session whose `project_id` equals the consuming successor's
`project_id`. The `TaskObserved` event is recorded under the **successor** id
(unchanged: `session_id = project_session_id` = successor); the outbox row keeps
its historical `recipient_id` and is marked delivered.

Pending-observation **counts** that drive status/snapshot
(`ops::project::project_snapshot:664`, `lf::commands::waves::snapshot_project_runtime:1066`)
switch to the chain count for a project recipient, so the successor's pending
count includes predecessor-addressed observations. The terminal predecessor's
own-id count drains to 0 as the successor delivers them.

### 3. Status-triggered reconciliation → successor (criterion, "reconciliation")

`project_session::runner::inspect_outcome` (`runner.rs:764-767`) currently
filters Tasks by `task.project_session_id == session.id`, so the successor never
supervises Tasks born under the predecessor. Change the filter to
`task.launch.project.id == session.launch.project.id` — the successor
reconciles every Task for the Linear Project. Only the successor runs
`inspect_outcome` (the predecessor is terminal and never runs), so there is no
double-supervision.

### 4. Review requests → successor; predecessor reviews stay completable (criterion 2)

The review row keeps its historical `project_session_id` and `reviewer`
(`InteractionReviewer::Project(<historical>)`) as provenance — symmetric with the
observation outbox. Routing is resolved at the authority boundary, not by
rewriting the assignment:

- **Completable by the successor**: the live successor (holding the Project write
  lease) may conduct a review whose `project_session_id` is in its project chain.
  The authority checks were relaxed from "acting == review.project_session_id" to
  "acting is non-terminal and shares the review's Linear `project_id`":
  - `ops::project::project_review_message` and `project_review_complete` (the
    ambient-successor check, and they now pass the ambient successor — not the
    review's session — to the store as the acting lease holder);
  - `store::sqlite::interaction_reviews::complete_project_interaction_review` and
    `require_open_project_review` (via a new `project_review_chain_ok` helper).
  A Project outside the chain is still rejected.
- **Discoverable**: `list_interaction_reviews` is wave-scoped, so
  predecessor-assigned reviews already appear in `lf reviews`; the successor's
  pending-review view is driven by the same chain reads, so a review assigned to
  a predecessor surfaces as actionable for the live successor.

### 5. Status exposure → historical owner + current routing target, no terminal wake warning (criteria 3, 5)

- Add to `ops::task::TaskSessionSnapshot`: `routing_project_session_id: String`
  and `project_route_succeeded: bool`, computed by resolving the route in
  `task_snapshot`. `project_session_id` stays (historical owner).
- Add to `lf::commands::waves::TaskRuntimeSnapshot`: `routing_project_session_id`
  for the app/terminal runtime view, resolved in `snapshot_task_runtime`.
- `print_task_session` (`bin/lf.rs:588`) prints a `project:` line only when the
  route succeeded (`project: {historical} → routes to {current}`), stating the
  historical owner and current target as fact — **not** a "terminal session
  wake" warning. When the route is the historical owner itself, print nothing
  extra (no noise for the common case).
- No code path emits a "terminal-session wake warning" on the status surface.
  The wake log stays a `tracing::warn!` for genuinely broken chains, never a
  status string.

## Absent and error states

- **Recorded Project Session missing from registry**: resolver returns an
  actionable error naming the missing session id.
- **Recorded session terminal, no live successor** (stale/missing chain): resolver
  returns an actionable error naming the terminal session, its status, the
  project id/slug, and the recovery command. Surfaces: wake (warn log), review
  creation (error to the Task runner), status (`routing_project_session_id` =
  historical, `project_route_succeeded = false`, and a `routes to <dead>` note
  only when the chain is broken — stated, not warned).
- **No observations**: chain query returns empty; successor proceeds; counts 0.
- **Review for a task whose project has no live successor**: review creation
  fails actionably rather than assigning to a terminal session.

## Operational boundary

All routing resolution is in-process, in-memory reads against the local
registry DB (no network, no subprocess beyond the existing wake which already
launches a tmux body). The resolver adds one indexed `project_sessions` lookup
per Task observation/review/status read — negligible. No migration: the
`project_sessions` schema and `observation_outbox` shape are unchanged; routing
is resolved from existing columns (`project_id`, `status`, `created_at`).

## Exclusions

- Rewriting `task_sessions.project_session_id` (provenance is preserved).
- Rewriting `observation_outbox.recipient_id` (provenance is preserved; routing
  is read-time chain resolution).
- Changing the `0.11.002` successor index or the append-only replacement model.
- Project→Wave observation routing (unchanged).
- Cross-project or cross-wave routing (out of scope; routing is per Linear
  project id).

## End-to-end proof

One deterministic store-level scenario crosses the source of truth and every
affected consumer:

1. Create wave W, project session P0 (Running) for Linear project `L`.
2. Create Task T under P0 (`project_session_id = P0`).
3. Abandon P0; create successor P1 (Created→Running) for the same `L`, newer
   `created_at`.
4. Append a project-observable Task event on T.
   - **Wake**: `wake_task_project_route` resolves `current = P1`; only P1 is
     woken. Assert P0 is not woken (criterion 1).
5. Run the successor's `consume_task_observations` for P1.
   - It drains the observation addressed to P0 (chain query), records
     `TaskObserved` under P1, marks the outbox row delivered. Assert P0's
     own-id pending count is 0 and P1 saw the event (criterion, observation
     routing).
6. `inspect_outcome` for P1 includes T in its Task set (criterion,
   reconciliation to successor).
7. Open a `Defer` review for T (assigned to P0, the historical owner). Assert
   the P1 body (holding P1's lease) completes it (criterion 2); assert a Project
   outside the chain cannot.
8. `task_snapshot(T)` exposes `project_session_id = P0` and
   `routing_project_session_id = P1`, `project_route_succeeded = true`; the text
   status states the route with no "terminal wake" warning (criteria 3, 5).
9. Broken chain: abandon P1 (no further successor). Resolve the route for T →
   actionable error naming P0/P1, project L, and the recovery command. Assert
   review creation for T fails actionably (criterion 4).

Commands that prove it: `cargo test -p loopflow route_successor` (unit/store) and
`cargo test -p loopflow task_routes_to_successor` (integration). `lf task status
<W2-243-issue>` after the fix shows the historical owner and current routing
target with no terminal-session wake warning.
