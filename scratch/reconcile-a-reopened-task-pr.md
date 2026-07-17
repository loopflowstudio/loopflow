# Reconcile a reopened Task PR before rotating

## Problem

A Task PR row carries `abandoned_at`. Once it is stamped, loopflow can never
look at that PR again — and nothing in production ever clears it.

Observed on ENG-20 (PR #1026): the PR was closed, then reopened on GitHub at the
same head. Loopflow kept `abandoned_at`, so `lf ci` omitted the live failure, the
Task resumed by minting an empty serial branch at current `main` (sequence 2),
and the gate was asked to review a PR with identical base and head. The published
PR stayed open and red the whole time, unobserved.

The mechanism is a one-way door, confirmed end to end:

1. `TaskPr::phase()` (`task/mod.rs:478`) checks `abandoned_at` **first**, so an
   abandoned row reports `Abandoned` no matter what GitHub says.
2. `active_task_pr` (`store/sqlite/child_sessions.rs:828`) filters
   `merge_commit IS NULL AND abandoned_at IS NULL`.
3. `reconcile_task_pr_with_authority` (`ops/task.rs:2679`) *begins* at
   `active_task_pr` and returns `None` for an abandoned row — so it never issues
   the GitHub read that would reveal the reopen.
4. `ensure_working_pr_with_authority` (`ops/task.rs:3248`) treats "no active PR"
   as rotation eligibility. `is_settled()` is true for Merged and Abandoned
   alike, so sequence N+1 is minted off an abandoned N exactly as off a merged N.

`abandoned_at` is written in exactly two places, and **cleared in none**:
`abandon_task_pr` (`ops/task.rs:1721`) and reconcile's `"closed"` arm
(`ops/task.rs:2844`).

The reduction this unlocks: **both writers mean the same thing.**
`abandon_task_pr` runs `gh pr close` (`ops/task.rs:1716`) before stamping the
field. So on a *published* PR, `abandoned_at` is not an independent decision — it
is a cached claim that GitHub has the PR closed. GitHub is authoritative for that
claim, and a human reopening the PR overrides both writers identically. There is
no operator-abandon-vs-observed-close distinction to preserve, and therefore no
second lifecycle to build.

Who benefits: any Task whose PR is closed and reopened — a routine review action
that today silently forks the Task onto a branch nobody asked for.

## The demo

Take a Task whose red PR was closed, and reopen it on GitHub:

```
$ gh pr reopen 1026
$ lf ci
  REPO             PR     CHECK      STATE    OUTCOME
  loopflow         #1026  rust-test  failing  open        # the live failure is back
$ lf task resume W2-286
  Task W2-286 resumed on jack-heart/reconcile-a-reopened-task-pr (pull request #1026, sequence 1)
$ lf task show W2-286 --json | jq '[.prs[] | {sequence, phase}]'
  [ { "sequence": 1, "phase": "open" } ]                   # no sequence 2, ever
```

The reopened PR is active again, red again, and woken again — on the same branch
and the same PR number.

## Approach

Make GitHub authoritative for a published PR's closed state, in the code that
already reconciles every other GitHub fact. Two changes, both subtractive.

### 1. Let reconcile see a settled-but-published PR

`reconcile_task_pr_with_authority` resolves its subject as: the active PR, else
the latest published, non-merged row. Compose it from the existing
`store.task_prs()` (already ordered; a Task has a handful of PRs) rather than
adding SQL — no new store method, no new index.

Merged stays terminal: GitHub cannot unmerge, and
`CHECK (merge_commit IS NULL OR abandoned_at IS NULL)` already encodes it.

### 2. Clear `abandoned_at` when GitHub reports the PR open

The `_` (open/draft) arm of the state match (`ops/task.rs:2854`) already refreshes
`publication.github`, re-reads the required checks, mints the CI incident, and
emits `PrOpened`. It needs one more line:

```rust
pr.abandoned_at = None;
```

A no-op on the normal path; the whole reopen fix on the abandoned path. Nothing
else moves: `pr.is_settled()` is then false, so the existing persistence branch
routes to `update_task_pr_with_authority` — a plain UPDATE, not
`settle_task_pr_on`, which would have refused with "already settled differently".
The row returns to `Open`, re-enters `active_task_pr`, and every downstream
consumer works again with no knowledge of reopening.

Once the row is active, the rest is free:

- **`lf ci` sees it.** `ci_incidents_since` never joins `task_prs` — it reads
  `ci_incidents`. The live failure was omitted because reconcile never ran, so
  `observe_ci_incident` never minted the row. Restoring the read restores the
  report. No change to `lf ci`.
- **The wake arms.** `queue_ci_fix_command` → `ci_fix_wake_kind` →
  `current_ci_incident` all funnel through the PR the reconcile returned.
- **The wake stays bounded.** `ensure_child_ci_fix_command` dedups on incident
  identity (`repo:number:head:failure-set`) and deliberately ignores command
  state, terminal included. A reopen at the same head with the same failures
  mints the *same* identity and therefore no second wake. That is the bound, and
  it is correct: re-arming is the job of a moved head or a changed failure set.
  This design does not touch it.

### 3. Refuse to rotate past a predecessor GitHub could not confirm

Change 1 prevents the ENG-20 rotation on its own — reconcile runs first inside
`ensure_working_pr_with_authority`, clears `abandoned_at`, and the existing
`if let Some(active) = active_task_pr { return }` gate stops the mint. But it only
works when the read *succeeded*. A degraded read (quota, outage, no `gh`) leaves
the stale `abandoned_at` standing and rotation proceeds on unverified state —
the same silent fork, one outage away.

So at the single mint point (`ops/task.rs:3286`), before `settled.sequence + 1`:
refuse when the settled predecessor is published, abandoned, and the observation
backing that claim is `Degraded`. Reconcile just read that exact row, so
`session.observation` already describes it — the guard reads it rather than
issuing a read of its own.

This is the seed's "or explicitly stop for an operator": name the PR, say the
read failed, and stop. Bounded by the existing 5-minute degraded circuit.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Can a reopened PR be tested at all, or does this need real GitHub? | Yes. `task/runner/ci_fix_lifecycle_tests.rs` already drives the whole observation path against a fake `gh` on `PATH` (`FAKE_GH`, :175), with `FakeGh::set_pr(number, state, head_sha)` (:202) writing the exact REST body reconcile parses. `set_pr(n, "closed", sha)` then `set_pr(n, "open", sha)` *is* the reopen. | The proof lives in that module — same state machine, same fake. No new harness, no production reshaping. |
| Does the test harness actually exercise the reopen, or pin a fixture? | **It would silently pin the fixture.** `expire_read_cache` (:499) resolves via `active_task_pr` and `let Some(pr) = … else { return }` — for an abandoned row it returns early, the read cache never expires, and reconcile serves the cached reading without ever spawning the fake `gh`. The test would report on a stale observation. | Fix `expire_read_cache` to resolve the same row reconcile does. Then sabotage-check: revert the one-line `abandoned_at = None` and confirm each new test *fails*. |
| Does the DB allow a reopened row to go back to active? | Only if no successor is active: `CREATE UNIQUE INDEX idx_task_prs_open ON task_prs(task_session_id) WHERE merge_commit IS NULL AND abandoned_at IS NULL`. | Not reachable by construction — the fallback is only entered when `active_task_pr` returned `None`. The index stands as the backstop that makes "one active PR" a real invariant rather than a convention. |
| Does clearing the field need a new store method to bypass `settle_task_pr_on`'s "already settled differently"? | No. That guard sits on the *settle* path. A cleared row is `!is_settled()`, so reconcile's existing branch routes to `update_task_pr_with_authority`. | Zero store surface added. Verify at build that the free `update_task_pr` (:3739) writes `abandoned_at` in its column list. |
| Is "identity/head still match" a real check, or an assumption? | A PR's head ref is immutable on GitHub, and the row records the number at publish time. `observe_pr_by_number` reads `repos/{nwo}/pulls/{number}` — the number *is* the identity. `GhRestHead` parses only `sha`. | **Do not** add a `head.ref == pr.branch` check. It can never fire, and a field parsed only to satisfy an impossible condition is exactly the redundancy to prune. Identity = the recorded PR number. Head SHA is refreshed, not matched — a reopened PR that got a new push is still the same PR, and `fresh_ci` already handles the moved head. |
| Does reconcile returning `Some(abandoned_pr)` where it returned `None` break callers? | Audited, and it found one. `runner.rs`'s `needs_rotation` stays correct (a reopened PR is no longer settled, so it does not rotate). But `settled_completing_pr` keyed on `is_settled()` while the branch it guards reports the PR **merged** and waits on its review gate — true of a merge, false of an abandoned PR. The lie already existed on the turn that closed the PR; keeping a published row readable would have made it reachable on every later turn. | Fixed in place: `merged_completing_pr`, keyed on `phase() == Merged`. Merged behaviour is byte-identical; the abandoned-completing case keeps its pre-change path and stops claiming a merge. |
| Can the degraded-rotation test model an outage by deleting the fake `gh`? | **No — and this is why the test asserts the message, not `is_err()`.** `AmbientGuard` *prepends* its bin dir to the real `PATH`, so deleting the fake falls through to the host's installed `gh`, which reads real GitHub for `test/repo`, 404s, and lands in `NotFound` — leaving freshness `Fresh`. The guard never fired, rotation ran on, and it died on an unrelated git error that a bare `is_err()` would have accepted while minting the very branch the test forbids. | The harness models the outage by *replacing* the script with one that fails like an exhausted quota (`classify_pr_read_failure` → `Degraded`). Deterministic on a host with or without gh. Note: the pre-existing `a_gh_outage_degrades_the_read_without_inventing_a_reading` uses the delete idiom and is host-dependent for the same reason — it still passes, weakly. Filed, not touched. |
| Should the reopen re-mint a ci-fix wake for a failure already woken before the close? | No. The identity dedup ignores state "terminal included" — deliberate, documented, and the thing that makes the wake bounded. | Out of scope, and load-bearing. If a wake was spent before the close, the reopen does not buy a second one; a moved head or changed failure set does. |
| `abandon_task_pr` discards `gh pr close`'s exit status (`let _ =`). Does that need fixing here? | It stamps `abandoned_at` even when the remote close failed — a lie. But change 1 makes it self-healing: the next reconcile sees the PR open and clears the field. | Out of scope; the consequence is already covered. Filed below. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| A `reopened_at` column / explicit reopen lifecycle | Explicit state, greppable | Two fields, one truth — the drift this bug already is. The directive forbids a parallel reopen lifecycle, and it would be the third writer to a question GitHub already answers. |
| Store `phase` as a column instead of deriving it | Reopen becomes a state transition | A migration and a second source of truth for a value `phase()` derives correctly today. The bug is a stale *input*, not a bad derivation. |
| Only guard at rotation; never reconcile abandoned rows | Smallest diff; stops the empty successor | Fails the seed. The PR stays invisible to `lf ci` and never receives its wake — it just strands quietly instead of forking loudly. |
| Stop stamping `abandoned_at` from observation; keep it operator-only | Restores the field to one meaning | Loses closed-PR detection entirely, and the operator path closes on GitHub too — the two are the same claim, not two meanings. |
| Detect and repair a pre-existing abandoned-open + active-successor pair | Heals ENG-20's exact row | Prevention makes it unreachable, and the unique index blocks new ones. ENG-20's successor is already abandoned in reality. Repairing a one-off data mess is not worth a permanent code path. |

## Key decisions

- **`abandoned_at` on a published PR is a cache of GitHub's closed state, not a
  decision.** This is the whole design. It licenses one writer to clear it and
  needs no new vocabulary. On an *unpublished* PR (no number) it stays purely
  local and terminal — there is nothing to reconcile against, and reconcile
  already returns `NotRequired` there.
- **The fix is one line in the arm that already handles "open."** Everything else
  — active-PR restoration, `lf ci` visibility, the wake, the event — falls out of
  code that already exists and was only ever starved of the read.
- **Identity is the PR number.** No head-ref check; it cannot fire.
- **The wake bound is untouched.** Identity dedup stays terminal-blind.
- **Degraded ≠ closed.** The one genuinely new rule: never rotate on a claim
  GitHub declined to confirm. Stop and name the PR.

## Scope

In scope:
- Reconcile resolves the latest published non-merged PR when none is active.
- The open arm clears `abandoned_at`.
- The mint point refuses to rotate past a published abandoned predecessor whose
  observation is `Degraded`.
- Fix `expire_read_cache` in the ci-fix harness so it can age an abandoned row's
  read cache.
- Tests (below), each sabotage-checked.
- Audit `reconcile_task_pr*` callers for `Some`-means-active assumptions — this
  found `settled_completing_pr` reporting an abandoned PR as merged; fixed as
  `merged_completing_pr`.

Out of scope (found here, filed as follow-ups):
- `abandon_task_pr` discarding `gh pr close`'s exit status. Self-healing under
  this change; a real hole, separately.
- A genuinely abandoned PR's unresolved `ci_incidents` linger as `open` forever
  with no `blocked_at`, dragging `summary.unresolved` and the median green/merge
  metrics — `ci_incidents_since` never joins `task_prs`. Metric hygiene, not
  control flow.
- `lf pr next`'s "current PR is not merged yet" message overstating its check (it
  fires only when a PR is active, so it rotates off an abandoned PR happily).
  Covered in effect by the guard; the wording is a separate cleanup.

## Done when

**Done.** `cargo test -p loopflow` green: 1515 lib + all integration targets, run
to completion. Two proofs in `task/runner/ci_fix_lifecycle_tests.rs`, compressed
to the minimum behaviours rather than one test per assertion — matching the
module's existing one-pass style:

- `a_reopened_pr_is_restored_red_woken_once_and_mints_no_successor` — drives the
  real writer of `abandoned_at` (GitHub reports closed) rather than stamping the
  field by hand, then reopens: same `id`/number/branch/sequence, `phase == Open`,
  the red head visible as exactly one incident to `ci_incidents_since`, exactly
  one wake minted (a repeat observation mints none), and `ensure_working_pr`
  leaves `task_prs().len() == 1`.
- `a_degraded_read_refuses_to_rotate_past_an_abandoned_pr` — the refusal names the
  PR and no successor is minted.

**Sabotage-verified** — each production change was reverted in turn, and only the
test naming it failed:

| Sabotage | Result |
|---|---|
| drop `pr.abandoned_at = None` | reopen test fails, degraded test unaffected |
| `PrPhase::Abandoned` → `Merged` in the rotation guard | degraded test fails only |
| `reconcile_subject` fallback → `Ok(None)` | both fail |
| `expire_read_cache` → `active_task_pr` (harness) | both fail — the fixture-pinning trap was real |

Remaining: the dogfood demo — close and reopen this PR, confirm `lf ci` shows its
head and the Task stays on sequence 1.

## Measure

Not a latency change. The observable is a count: **empty successor branches
minted while a published PR is open — currently reachable, target zero**, and
GitHub reads per reconcile, which must stay at one (the guard reads
`session.observation`, it does not add a call). Serves Developer Efficiency's
"a week of normal development requires zero manual git surgery" — deleting the
orphan sequence-2 branch is exactly that surgery — and "avoidable human-in-the-loop
repair steps fall to zero."
