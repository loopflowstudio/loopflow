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

Only a **Move**-class issue runs this sequence (an issue on the legacy team that
this code is about to move). Per such issue:

1. **Traceability comment (before the move), existence-checked *against this exact
   migration*.** The comment's critical payload — the old id — is known *before* the
   move, because the issue is still `W2-N`. Post
   `Reteamed by loopflow: was {W2-N}; moving onto team {target_key}. The issue id
   (UUID) is unchanged; Linear reassigns the number on the move.` **only if** the
   issue does not already carry a comment matching *this* migration — a body
   containing `was {W2-N}` onto team `{target_key}`, not any generic
   "Reteamed by loopflow" text. The specificity matters: an issue reteamed in a
   prior migration carries a `was {A-5}` comment; matching a generic marker would
   false-skip *this* migration's record. The check reads existing comments via the
   client's existing `observe_issue(id).comments` — no new query. (`W2-N` is stable
   across Move-class reruns because the issue stays `W2-N` until it moves, so the
   match key is stable.)
2. **`move_item_to_team`** → returns `ENG-M`.
3. **`rebind_task_issue_identifier`** (only when a Session exists), keyed on the
   stable issue **UUID**. Idempotent: its `WHERE issue_identifier = {old}` matches
   nothing once already rebound.

**Ordering invariant:** step 1 must succeed before step 2 runs, and any per-issue
error aborts the apply (the loop stops, apply returns the error). So a durable
`ENG-M` (move done) *implies* this migration's traceability comment already exists.

**`Already` never comments and never moves.** An `Already`-class issue (target-team
prefix) is *not* evidence this code moved it — the 22 existing `ENG-*` issues
include directly-created items with no migration comment and no prior `W2-N`. So
`Already` does exactly one thing: reconcile a **stale Session** by stable UUID
(`rebind` when `session.identifier != item.identifier`), and nothing when the
Session is absent or already current. It posts no comment — comment-before-move
already guarantees that any move *this code* performed recorded the old id, so
there is nothing to recover here.

**Recovery across every crash boundary** (rerun re-derives, never hand-waves at a
clean dry-run):

| Crash point | State left behind | Next run |
|-------------|-------------------|----------|
| after comment, before move | issue still `W2-N`; this-migration marker present | `Move`; existence-check matches `was {W2-N}` → skips re-post; move; rebind. No duplicate. |
| after move, before rebind | issue `ENG-M`; marker present (posted pre-move); Session cache still `W2-N` | `Already`; rebinds `W2-N → ENG-M` by UUID. No comment (the old id already lives in the pre-move comment). |
| after rebind | issue `ENG-M`; cache `ENG-M` | `Already`; Session already current → no-op. |

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
| Does a rerun double-comment or re-move? | The move is skipped by `Already` classification; the pre-move comment is existence-checked against *this migration's* `was {W2-N}`/target marker (not a generic one), so it posts at most once. `Already` posts no comment at all. | Reruns neither duplicate a comment nor re-move; a partially-migrated issue completes idempotently, and directly-created `ENG-*` issues never get a spurious migration comment. |
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

- **In scope:** `classify_reteam_item` (drop `Historical`), splitting
  `pm_reteam_async` into `resolve_reteam_context` + `apply_or_plan_reteam` (invert
  order, comment-before-move with this-migration existence check, `Already` =
  rebind-only, gate-before-mutation, per-moving-Project target-team assertion), a
  comment existence check reusing `observe_issue`, `PmReteamResult` (drop
  `historical`), `print_pm_reteam_result` (drop the historical line), and the reteam
  tests.
- **Out of scope:** active Task worktrees and live provider state (the
  implementation body must not run `reteam --apply` itself — the orchestrator does
  that after merge). No schema change. No `pm init` / binding changes. No other
  `lf pm` verb.

## Done when

- `classify_reteam_item` returns `Move` (not `Historical`) for a completed issue
  with no live writing body, and `Defer` for a completed issue whose Session still
  holds a lease.
- A behavioral test drives the **real** apply path (`apply_or_plan_reteam` with a
  `test_server`-backed client + temp store) and asserts every issue team-move
  precedes every Project `teamIds` replacement, and that a completed issue produces
  a move mutation. Restoring Projects-first or deleting the issue-move loop turns it
  red (sabotage-proof).
- Failure-injection tests drive that same real path across both partial-success
  boundaries: (a) comment ok then `issueUpdate` errors → rerun posts **no** second
  comment, then moves and rebinds; (b) store seeded moved-but-not-rebound → the
  `Already` path rebinds by UUID and posts **no** comment. Exactly one
  `Reteamed by loopflow` comment survives per issue and the store ends on the new
  identifier.
- A test proves `Already` is not treated as migration: a directly-created `ENG-*`
  issue records no `commentCreate` and no `issueUpdate`; only a stale-Session
  `ENG-*` triggers a rebind.
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

**Tests drive the real apply path, never a provider-only slice.** A test of a
provider-only helper stays green if `pm_reteam_async` omits or misorders the call,
and cannot exercise the store rebind — so it proves nothing about production. The
seam is instead *dependency* extraction: split `pm_reteam_async` into
`resolve_reteam_context` (wave/provider/token/client/store — thin wiring shared with
every other `lf pm` verb) and `apply_or_plan_reteam(&ctx, &store, repo, wave,
team_id, team_key, apply)` which holds the **entire** decision surface —
classification, both safety gates, the ordered provider mutations, *and* store
reconciliation — and returns `PmReteamResult`. `build_client` today hardcodes the
real Linear URL, so this is what makes an end-to-end test possible at all: the test
builds a `PmContext` from a `test_server`-backed `LinearClient` (`with_base_url`,
per `linear_test_ctx`) plus a temp `Store`, and drives `apply_or_plan_reteam`
directly. Everything a sabotage could break lives inside the tested unit.

- **Order test (sabotage-proof).** Drive `apply_or_plan_reteam` with `apply=true`
  against a mock serving the full sequence (list items already provided via `ctx`
  path, `observe_issue` reads, `commentCreate`, `issueUpdate` per move,
  `projectUpdate` per Project, snapshot refresh) and a temp store holding a stale
  Session. Assert from `test_server`'s recorded requests that every
  `issueUpdate`(`teamId`) precedes every `projectUpdate`(`teamIds`), and that the
  temp store's Session ends on the new identifier. **Sabotage:** restoring
  Projects-first flips the recorded order → red; deleting the issue-move loop leaves
  the Session unreconciled and no `issueUpdate` recorded → red.
- **Completed-migration test (end-to-end).** A completed `W2` issue in the mock
  produces an `issueUpdate` (it moves), proving `Historical` is gone. Plus the
  classification-level cases: completed + no session and completed + terminal session
  → `Move`; completed + active lease → `Defer` (update
  `reteam_classifies_move_defer_leave_and_skip`, which currently asserts
  `Historical`).
- **Failure-injection, both boundaries, on the real path.**
  (a) *comment ok, move fails*: queue an error at the `issueUpdate`; the first
  `apply_or_plan_reteam` returns `Err` with the comment already recorded. Re-drive
  against a mock whose `observe_issue` now returns the `was {W2-N}` comment and whose
  issue is still `W2-N`; assert the second run issues **no** second `commentCreate`,
  then moves and rebinds.
  (b) *move ok, rebind not done*: seed the temp store directly in the moved-but-not-
  rebound state (mock issue on `ENG-M` carrying the pre-move comment; Session cache
  still `W2-N`), drive `apply_or_plan_reteam`; assert the `Already` path rebinds the
  Session `W2-N → ENG-M` by UUID and issues **zero** `commentCreate`. Across both
  runs of (a), exactly one `Reteamed by loopflow` comment exists.
- **Preflight test (end-to-end).** With a moving issue whose Project resolves to
  `team_ids == None` or a set lacking the target team, `apply_or_plan_reteam` with
  `apply=true` returns `Err` naming the Project **before** any `issueUpdate` is
  recorded by `test_server`; with the target team present it proceeds.
- **`Already`-is-not-migration test.** A directly-created `ENG-*` issue with a
  current Session (or none) drives `apply_or_plan_reteam` and records **no**
  `commentCreate` and no `issueUpdate` — only a stale-Session `ENG-*` triggers a
  rebind, never a comment.
- Store reconciliation internals are already covered by
  `rebind_task_issue_identifier` tests in `store/mod.rs`; don't duplicate them.
