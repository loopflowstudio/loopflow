# Make existing Tasks honor `lf task run --headless`

## Problem

`lf task run <issue> --headless` mutates an existing idle `TaskSession` in
memory, calls `update_task_session`, prints status, and exits successfully. The
control UPDATE writes `updated_at` but omits the lifecycle configuration, so the
persisted policies remain `require|defer|require`. Future interactive steps keep
routing to a human who is absent in an autonomous run.

The persistence omission covers six existing columns, not three:

```
iterate_flow                    ?35
iterate_interaction_policy      ?36
kickoff_flow                    ?45
kickoff_interaction_policy      ?46
gate_flow                       ?47
gate_interaction_policy         ?48
```

`task_session_params` already supplies all six. `TASK_SESSION_UPDATE` stops at
`?34`, and `task_session_control_params` truncates the vector to 34, making the
omission internally consistent and therefore silent.

A second boundary exists only at the Task's current waitpoint. Changing future
policy cannot reassign a nonterminal Human review already minted there. The
command must refuse before mutation and name that review; it must not search
historical or unrelated reviews.

This directly advances Developer Efficiency's KR that avoidable human repair
steps found in agent runs fall to zero: an idle existing Task becomes headless
with the documented command instead of requiring relaunch or store surgery.

## The demo

```
$ lf task run W2-YYY --headless
$ sqlite3 ~/.lf/loopflow.db "SELECT kickoff_interaction_policy,
    iterate_interaction_policy, gate_interaction_policy
    FROM task_sessions WHERE issue_identifier='W2-YYY';"
defer|defer|defer
```

At a current nonterminal Human review, the same command exits nonzero and names
the review id; the three stored policies remain unchanged.

## Approach

Keep the existing conversion path and repair only the persistence boundary.

1. Leave `_defer_task_interactions(&mut TaskSession)` synchronous. It retains
   the existing terminal/body guards, calls `defer_all_interactions`, and
   updates the in-memory timestamp.
2. Before calling it for a policy change, read exactly one row with
   `interaction_review_at(session.id, phase_epoch, phase_iteration,
   phase_cursor)`. If that current review is nonterminal and Human, refuse with
   its id. Completed or parent-owned reviews do not block conversion.
3. Keep the caller's one existing `update_task_session` call.
4. Add the six lifecycle flow/policy assignments to `TASK_SESSION_UPDATE` and
   extend the control parameter slice to `?48`. Parameters `?37..?44` are bound
   because they are interleaved in the shared vector, but remain unreferenced by
   the control statement, so lease-owned execution state is not written.
5. Prove only the two observable behaviors through the existing-Task
   `task_run` branch itself. Each fixture executes
   `lf task run INF-123 --headless` against a temporary registry, then reopens
   that registry and reads the same Session. Calling private helpers directly
   does not count as proof.

No migration is needed. The columns already exist, INSERT already writes them,
and reads already reconstruct the lifecycle plan.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Did `_defer_task_interactions` fail to run? | No. `updated_at` moves while policy does not. | Repair the store mapping, not the mutation helper. |
| Is the omission limited to policy? | No. All three phase flows are dropped by the same `truncate(34)` boundary. | Persist all six lifecycle configuration fields. |
| Does extending to 48 overwrite lease state? | No. SQLite binds interleaved `?37..?44`, but the control SQL does not reference them; the existing position round-trip test keeps phase position unchanged. | Keep the shared positional vector and document the highest control parameter. |
| Should conversion inspect every review in the Wave or Task history? | No. Only the review at the current lifecycle coordinates can still park this conversion path. Historical completed reviews are evidence for other lifecycle rules. | Use `interaction_review_at`, not `list_interaction_reviews`. |
| Can the current Human review be rewritten as Project-owned? | No supported supersession path exists, and reviewer kind is coupled to policy. Rewriting would broaden review lifecycle and authority. | Refuse before mutation and preserve #1034's authority boundary. |
| Is runtime post-write verification required? | No. Directive v2 keeps the command on the established single-write path and asks the focused round-trip test to own regression detection. | Do not add a readback query or new state. |
| Is a generalized struct-to-column classifier warranted? | No. This Task owns one measured omission, not a persistence framework. | No exhaustive destructuring test, macro, derive, or new store API. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Add only the three policy assignments | Smallest SQL diff | Leaves the identical flow omission behind and keeps lifecycle configuration partly unwritable. |
| Query all reviews for the Wave and filter locally | Reuses an existing broad read | Reads unrelated history and makes conversion policy broader than the current waitpoint. |
| Supersede the Human review | Could convert a parked Task in place | Requires a new review lifecycle or destructive evidence rewrite and risks weakening authority. |
| Re-read after the UPDATE and fail if policy did not stick | Permanently detects future store omissions at runtime | Adds a second store read and duplicates the focused persistence proof; directive v2 rejects it. |
| Add an exhaustive `TaskSession` destructuring classifier | Forces classification when fields are added | Generalized ceremony disproportionate to this defect; directive v2 rejects it. |
| Make existing-Task `--headless` always fail | Avoids silent success | Deletes a documented recovery path whose underlying write is straightforward to repair. |

## Key decisions

- Lifecycle flows and interaction policies are control-owned configuration.
  Lease writes remain unchanged and cannot clobber them.
- The conversion stays a read-modify-single-write operation. No new store
  method, transaction state, verification state, or compatibility path is
  introduced.
- Review refusal is coordinate-scoped: `(task_session_id, phase_epoch,
  phase_iteration, phase_cursor)`. A nonterminal Human row there is the only
  review blocker this Task recognizes.
- Completed reviews are never rewritten or treated as conversion blockers.
  The changes-requested completion deadlock belongs to W2-297.
- Success means future lifecycle policy is durably Defer. It does not mean an
  already-open Human review was reassigned.
- Both regression tests enter through `task_run`. This couples the proof to the
  command path that performs the coordinate lookup, calls the synchronous
  mutation helper, and owns the single control UPDATE; removing any one of
  those steps must make a test fail.

Wild success is boring: operators rerun the documented command on an idle Task,
see `defer|defer|defer`, and the next interactive step routes to its Project.
Wild failure would be a broad review scan refusing on stale history, or a new
write path drifting from the existing control UPDATE. The coordinate lookup and
single-write constraint rule both out.

## Scope

In scope:

- Six lifecycle configuration assignments in `TASK_SESSION_UPDATE`.
- The matching control parameter boundary at `?48`.
- One current-waitpoint review lookup before mutation.
- Refusal naming a current nonterminal Human review.
- Two focused behavior tests.

Out of scope:

- Review supersession, cancellation, or evidence rewriting.
- W2-297's changes-requested terminality and completion-gate history.
- Any change to `TASK_SESSION_LEASE_UPDATE`.
- Runtime post-write verification.
- Generalized persistence classification or code generation.
- New schema, store methods, or durable state.

## Done when

1. `task_run_headless_existing_task_persists_all_policies` constructs an idle
   standard-policy Task in a temporary registry, invokes the existing-Task
   `task_run` path with `headless: true`, then re-reads that Task from the store.
   Kickoff, iterate, and gate policies are all Defer:

   ```bash
   cargo test -p loopflow --test task_headless_conversion_tests \
     task_run_headless_existing_task_persists_all_policies
   ```

2. `task_run_headless_existing_task_refuses_current_human_review` constructs an
   idle standard-policy Task with a nonterminal Human review at its current
   lifecycle coordinates, invokes the same existing-Task `task_run` path with
   `headless: true`, and asserts the returned error contains the review id. A
   store re-read shows all six lifecycle configuration values — three flows and
   three policies — byte-for-byte unchanged:

   ```bash
   cargo test -p loopflow --test task_headless_conversion_tests \
     task_run_headless_existing_task_refuses_current_human_review
   ```

3. Removing the current-review query from `task_run` makes the second test fail;
   removing the defer call or six SQL assignments makes the first test fail.
4. `cargo fmt --all -- --check`, `cargo clippy -p loopflow --all-targets -- -D
   warnings`, and `cargo test -p loopflow --lib` pass.

## Measure

Before: an idle conversion exits 0 and persists `require|defer|require`.

After: it persists `defer|defer|defer`, or the current Human review causes a
nonzero refusal naming its id. The six lifecycle configuration columns writable
by no UPDATE fall from six to zero.
