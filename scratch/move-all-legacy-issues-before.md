# Move all legacy issues before narrowing Linear Project teams

## Problem

`lf pm reteam` completes the per-wave Linear team migration: each wave's Projects
and issues move from the shared legacy team (`W2`) onto the wave's own team
(`ENG` / `SCI` / `PRD`). New-issue creation already cut over in PR #1072. What is
left is the **existing items** — and the current implementation cannot finish
them.

Two design promises that were made together cannot both hold:

1. *Completed issues stay historical in the shared team* (`ReteamClass::Historical`).
2. *Every Project ends on exactly one team* (`project_needs_reteam`).

Linear refuses to remove a team from a Project while any issue owned by that team
remains in the Project:

```
lf pm reteam --wave intelligence --apply
→ loopflowstudio/loopflow cannot be removed from the project as they own
  issues that are part of the project.
```

The current apply order makes this fatal on the first step: it narrows the
**Project first**, then moves issues. Since completed issues are deliberately
left on `W2`, the Project can *never* be narrowed. No issue moves at all, because
Projects are updated before issues.

Who benefits: every wave's PM surface becomes truthful (`pm doctor` clean, one
team per Project, one prefix per issue), and `lf pm task done` stops failing on
the ENG-vs-W2 team split that the multi-team state causes.

## The demo

```
lf pm reteam --wave intelligence --apply
  ... moving W2-NNN into team SCI ...
  ... narrowing Project "Intelligence — …" onto team SCI ...
lf pm reteam --wave intelligence          # second run
  will move: none          # completed + open all on SCI, Projects on [SCI]
```

The migration that used to abort on its first mutation now runs to completion,
and re-running it is a clean no-op. `lf pm doctor --wave intelligence` reports no
stranded-team diagnostics.

## Approach

Three changes to `rust/loopflow/src/ops/pm.rs` (plus its CLI print + result
struct), all inside the `reteam` surface. No schema, no new noun, no touch to
active Task worktrees.

### 1. Completed issues migrate like any other issue

Delete `ReteamClass::Historical` and the `if item.completed { … }` early return in
`classify_reteam_item`. Completed issues then flow through the normal
`Move` / `Already` / `Defer` logic. A completed issue's Task Session is terminal,
so it carries no writing lease and is never protected — it simply `Move`s, gets a
traceability comment, and has its cached identifier reconciled, exactly like an
open issue with no live body.

This is strictly more correct: protection now keys on *session state*, not on
Linear completion. A "completed" issue whose Session somehow still holds a lease
would still `Defer` (it can no longer slip through as `Historical`).

### 2. Move issues before narrowing Projects

Invert the apply order. Today:

```rust
for pm in &project_moves { move_project_to_team(pm) }   // narrows first — fails
for mv in &mut moves     { move_item_to_team(mv) }
```

Becomes:

```rust
for mv in &mut moves     { migrate_issue(mv) }        // comment → move (see §4)
for pm in &project_moves { move_project_to_team(pm) } // then narrow
```

(Per-issue steps and their order are detailed in §4.) Global order ("all issues,
then all Projects") is sufficient: narrowing any
Project only requires *that Project's* `W2` issues to be gone, and every `W2`
issue across every Project is moved before the first narrow. No per-Project
interleaving needed.

### 3. Fail closed before the first provider mutation

Two independent safety gates, both evaluated **after** classification and
**before** any store or provider write:

- `ensure_reteam_apply_safe(&deferrals)` — unchanged in spirit, but the `Already`
  reconciliation (currently done inline during the classification loop, i.e.
  *before* this gate) moves to a post-gate pass, so a refused apply performs zero
  mutations of any kind.
- `ensure_move_projects_carry_target_team(...)` — new. Issues-first is only safe
  if the Project **that contains a moving issue** already lists the target team,
  so the moved issue *stays in the Project* under its new team. The gate is keyed
  on **every Project holding ≥1 planned issue move**, not on `project_moves`
  (Projects being narrowed). Those two sets differ: `project_needs_reteam(None)`
  returns `false`, so a Project whose `team_ids` did not resolve is **skipped for
  narrowing** yet can still hold issues we would move — narrowing only what we
  could resolve while mutating issues under an unresolved Project is exactly the
  half-migration to refuse. So the gate treats `team_ids == None` as **unsafe**
  (refuse), and requires `Some(set)` containing the target team for each Project
  with a planned move. Refuse with a diagnostic naming the offending Projects and
  their resolved team sets.

Any protected live writer (`deferrals` non-empty) refuses the **whole** apply.
This is load-bearing for Infrastructure: `W2-319`/`W2-320` are live, so a partial
move would leave their `W2` issues in the Project and the narrow would fail
mid-run. Fail-closed means Infrastructure simply waits until those bodies are
terminal, then applies cleanly.

### 4. Idempotent per-issue order — lossless across the irreversible move

`move_item_to_team` is the one irreversible step: Linear renumbers `W2-N → ENG-M`
and, after it, **nothing but our own record knows the prior number `W2-N`**. The
old order (`move → comment → rebind`) loses `W2-N` if the run dies after the move:
the next run sees the `ENG-M` prefix, classifies `Already`, and — in the current
proposal — neither re-posts the comment nor re-derives the old id, so the prior
identifier is gone. The store cache is not a reliable fallback either: issues with
no Task Session (most plain `W2` issues) have no cached identifier to recover from.

Fix: **record the prior identifier before the irreversible move, and make every
step idempotent.** Per moving issue:

1. **Traceability comment (before the move), existence-checked.** The comment's
   critical payload — the old id — is known *before* the move. Post
   `Reteamed by loopflow: was {old_id}; moving onto team {target}. The issue id
   (UUID) is unchanged; Linear reassigns the number on the move.` **only if** the
   issue does not already carry a `Reteamed by loopflow` comment. Idempotency reads
   the issue's existing comments via the client's existing
   `observe_issue(id).comments` — no new query. (The live number after the move is
   the issue's own current number, so the comment need not be updated with `ENG-M`;
   the only irreplaceable datum, `W2-N`, is now durable.)
2. **`move_item_to_team`** → returns `ENG-M`. Naturally idempotent under
   classification: on a rerun the issue is `Already`, so the move is skipped.
3. **`rebind_task_issue_identifier`** (only when a Session exists), keyed on the
   stable issue **UUID**. Idempotent: its `WHERE issue_identifier = {old}` matches
   nothing once already rebound.

**Ordering invariant:** step 1 must succeed before step 2 runs, and any per-issue
error aborts that issue (the loop stops, apply returns the error). So a durable
`ENG-M` (move done) *implies* the traceability comment already exists.

**Recovery across every crash boundary** (rerun re-derives, never hand-waves at a
clean dry-run):

| Crash point | State left behind | Next run |
|-------------|-------------------|----------|
| after comment, before move | issue still `W2-N`; marker present | `Move`; comment existence-check skips re-post; move; rebind. No duplicate. |
| after move, before rebind | issue `ENG-M`; marker present; Session cache still `W2-N` | `Already`; recovery re-runs the *idempotent comment* (marker present → skip) and rebinds `W2-N → ENG-M`. Old id was preserved in the comment written pre-move. |
| after rebind | issue `ENG-M`; marker present; cache `ENG-M` | `Already`, nothing to do. By the ordering invariant, `ENG-M` present ⇒ comment present, so skipping is safe. |

The `Already` branch therefore runs the **same** recovery as `Move` minus the move
itself: idempotent comment + rebind. A no-Session `Already` issue needs no rebind,
and its comment is guaranteed present by the invariant (move done ⇒ comment done).

Cached Session identifiers reconcile via `rebind_task_issue_identifier` keyed on
the stable UUID (`get_task_session_by_issue` matches `issue_id OR issue_identifier`),
independent of the number change.

### 5. Remove the historical surface

Delete `PmReteamResult.historical`, the `historical` counter, and the
`print_pm_reteam_result` block that prints "left as historical". reteam has no
`--json`, so this touches only the struct, the counter, the print fn, and tests.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Why does `reteam --apply` abort on step one? | Linear: a team cannot be removed from a Project while it owns issues in that Project. Current code narrows Projects before moving issues, and completed issues are never moved off `W2`, so the narrow can never succeed. | Invert order (issues first) and stop retaining completed issues. |
| Is the target team already on each Project (so moved issues stay in the Project)? | **Yes**, verified via live dry-run: `infrastructure` "Developer Efficiency" = `[ENG, W2]`; task report gives `intelligence` = `[SCI, W2]`. The binding step already added the wave team alongside `W2`. | 2-phase (move issues → narrow) is sufficient; no explicit "attach team" step needed. Assert the precondition per **Project holding a planned move** rather than depend on undocumented Linear move-time auto-association. |
| Can the preflight miss a Project whose issues still mutate? | Yes, if keyed on `project_moves`: `project_needs_reteam(None)` is `false`, so an unresolved-team Project is skipped for narrowing while its issues are still classified and moved. | Key the gate on every Project with ≥1 planned move, and treat `team_ids == None` as **unsafe** (refuse). An unresolved Project never has its issues moved. |
| Can a partial run lose the prior identifier `W2-N`? | Yes on `move → comment → rebind`: a crash after the irreversible move leaves an `Already`-classified issue whose comment was never written; no-Session issues have no store fallback. | Post the traceability comment **before** the move (old id is known then), existence-checked so reruns don't duplicate. Move done ⇒ comment present. |
| Can a *completed* issue move teams, preserving completion? | Linear remaps an issue's workflow state to the same *type* in the destination team on a team move; standard teams (`ENG`/`SCI`/`PRD`) have completed states, so a completed issue maps to the destination completed state. Medium confidence — not tested live. | The task's own rollout de-risks it: Intelligence applies first (16 completed) as the canary before Infrastructure (64 completed). If a completed move errors, it surfaces on the small batch. |
| Does Session reconciliation find the right row after renumber? | `get_task_session_by_issue` queries `WHERE issue_id = ?1 OR issue_identifier = ?1` and reteam passes the stable issue **UUID** (`item.id`). `rebind_task_issue_identifier` also keys on the UUID. So reconciliation is immune to the number change. | Reconciliation for both moved and already-moved issues is sound; kept. |
| Does a rerun double-comment or re-move? | The move is skipped by `Already` classification; the comment is existence-checked against `observe_issue(id).comments`, so it posts at most once. | Reruns neither duplicate a comment nor re-move; a partially-migrated issue is completed idempotently. |
| Rate limits at Infrastructure scale (64 completed + 20 open)? | ~170 sequential provider calls (move + comment per issue, plus reconciliation reads). Well inside Linear's per-hour budget. | No batching or backoff needed; note it and keep the loop sequential. |
| Does narrowing a Project drop it from its Initiative? | Initiative association is independent of team association in Linear. | Projects stay under the wave Initiative after the narrow. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep completed issues historical; find another way to narrow Projects | Preserves shipped `W2-N` references verbatim | Impossible: Linear won't narrow a Project while completed `W2` issues sit in it. The two promises are contradictory; the task chooses one-team Projects. Traceability comment preserves the old number instead. |
| 3-phase: attach target team → move issues → narrow | Correct even if a Project lacks the target team | Needs a new "add team, preserve others" mutation. The target team is already present on every real Project, so the attach never fires. Replaced by a cheap pre-flight assertion; attach is the documented future extension if a single-legacy-team Project ever appears. |
| Per-Project interleave (move a Project's issues, then narrow it) | Localizes each narrow | No correctness gain over global order, more bookkeeping, and worse under a mid-run interrupt (some Projects narrowed, others not). Global "all issues, then all Projects" matches the existing two-loop shape. |
| Leave `Already` reconciliation before the safety gate (as today) | Smaller diff | Violates fail-closed: a refused apply would still mutate the store. Moving it after the gate makes "no mutation on refusal" literally true. |

## Key decisions

- **Record the old id before the irreversible step.** The comment carrying `W2-N`
  is written *before* `move_item_to_team`, existence-checked so it posts once. The
  UUID (and every Session/PR/comment link) is stable; the only datum Linear
  destroys on the move is the old number, and it is durable before the move fires.
  Completed issues get the same comment.
- **Fail-closed is the whole safety model, and it is keyed on what actually
  mutates.** No live writer, and every Project *holding a moving issue* carries the
  target team (unresolved `team_ids` = refuse) — both proven before the first
  mutation. Infrastructure legitimately can't apply yet (`W2-319`/`W2-320` live);
  that's a wait, not a failure.
- **No attach step.** Verified unnecessary; a pre-flight assertion guards the
  precondition instead of adding a never-exercised mutation path.
- **Provider apply is orchestrator-only.** This Task body never runs
  `reteam --apply` against live Linear; all tests drive a mock. The orchestrator
  applies post-merge.

## Scope

- **In scope:** `classify_reteam_item` (drop `Historical`), the `pm_reteam_async`
  apply block (invert order, comment-before-move + existence check,
  gate-before-mutation, per-moving-Project target-team assertion), a comment
  existence check reusing `observe_issue`, `PmReteamResult` (drop `historical`),
  `print_pm_reteam_result` (drop the historical line), and the reteam tests.
- **Out of scope:** active Task worktrees and live provider state (the
  implementation body must not run `reteam --apply` itself — the orchestrator does
  that after merge). No schema change. No `pm init` / binding changes. No other
  `lf pm` verb.

## Done when

- `classify_reteam_item` returns `Move` (not `Historical`) for a completed issue
  with no live writing body, and `Defer` for a completed issue whose Session still
  holds a lease.
- A behavioral test drives the apply against a mock Linear (`test_server`,
  recorded request order) and asserts every issue team-move precedes every Project
  `teamIds` replacement, and that a completed issue in the plan produces a move
  mutation.
- Failure-injection tests cover the two partial-success boundaries: (a) comment
  succeeds then move fails → the rerun re-moves and posts **no** second comment;
  (b) move succeeds then rebind fails → the rerun rebinds and posts **no** second
  comment, and the traceability comment written pre-move still names `W2-N`. Each
  asserts exactly one `Reteamed by loopflow` comment survives and the store ends on
  the new identifier.
- `ensure_move_projects_carry_target_team` refuses apply (before any mutation) when
  any Project holding a planned move has `team_ids == None` or a set lacking the
  target team.
- `PmReteamResult.historical` and the "left as historical" output are gone; no
  remaining reference to `Historical` in the tree.
- `cargo fmt`, `cargo clippy -- -D warnings`, and the reteam unit + behavioral
  tests pass.
- Then (orchestrator, post-merge, **not** this body): Intelligence apply succeeds;
  a second Intelligence dry-run is a no-op; Product and Infrastructure dry-runs
  still name their protected live writers.

## Measure

Baseline (today, dry-run): `lf pm reteam --wave infrastructure` reports
`20 will move`, `2 deferred`, `22 already`, `64 left as historical`, and
`--apply` for `intelligence` aborts on the first Project update with zero issues
moved.

After: `intelligence --apply` moves every legacy-team issue (completed included),
narrows both Projects to `[SCI]`, reconciles cached identifiers, and a second
dry-run reports `will move: none`. Infrastructure's `64 left as historical` line
disappears — those 64 become part of `will move` until applied.

## Testing notes for the implementer

- **Mutation-order test.** Extract the provider execution into a small helper —
  `run_reteam_provider_apply(client, team_id, target_key, &mut moves, &project_moves)`
  — that performs *only* the provider work in order (per move: comment
  existence-check → conditional `comment` → `move_item_to_team` filling
  `new_identifier`; then per Project: `move_project_to_team`). `pm_reteam_async`
  calls it, then does store reconciliation. This separation is a genuine production
  clarity win (compute plan / execute provider / reconcile store), not a test-only
  seam, and it lets the order test run against a `test_server`-backed `LinearClient`
  with **no store or env**. Queue responses positionally (per move: an
  `observe_issue` read, a `commentCreate`, an `issueUpdate`; then `projectUpdate`
  per Project) and assert every recorded `projectUpdate`/`teamIds` index is greater
  than every `issueUpdate`/`teamId` index.
- **Failure-injection tests** live at this helper too: queue an error `QueuedResponse`
  at the `issueUpdate` (boundary a) or have the store rebind fail (boundary b),
  then re-drive the helper with fresh responses whose `observe_issue` now returns
  the marker comment, and assert the second run issues **no** `commentCreate` and
  completes the move/rebind. `test_server` records every request, so "exactly one
  `commentCreate` for this issue across both runs" is a direct assertion. Boundary
  (b) is drivable by pointing rebind at a temp store made to error, or by asserting
  at the `pm_reteam_async` level that an `Already`+stale-Session issue re-posts no
  comment and reconciles.
- **Completed-migration test.** At the classification level: a completed issue
  with no session, and a completed issue with a terminal session, both classify
  `Move`; a completed issue with an active-lease session classifies `Defer`.
  Update `reteam_classifies_move_defer_leave_and_skip` (it currently asserts
  `Historical` for a completed issue).
- **Preflight test.** `ensure_move_projects_carry_target_team` refuses (with a
  Project-naming message) when a moving issue's Project has `team_ids == None` or a
  set lacking the target team, and passes when the target team is present.
- Store reconciliation itself is already covered by
  `rebind_task_issue_identifier` tests in `store/mod.rs`; don't duplicate them.
