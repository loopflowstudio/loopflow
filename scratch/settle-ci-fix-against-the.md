# Settle CI-fix against the authoritative repaired head

## Problem

A CI-fix body is one bounded turn: it wakes for a failed PR head, pushes a fix
to the same branch, and parks. `settle_ci_fix_turn` then judges whether the head
moved — if it did, the Task waits for CI on the new head; if it did not, the
repair did not happen and the incident is **Blocked** ("the Task body did not
repair the head. Needs a new directive or human review").

That judgment reads a PR observation that can be stale. The settlement call site
(`runner.rs:688`) reconciles with `reconcile_task_pr_for_lease`, which routes
through `cached_github_observation` — a 60-second TTL over the last GitHub read.
When the body pushed only seconds ago and the cache is still warm, reconcile
returns **without re-reading GitHub**, so `pr.head_sha()` is still the pre-turn
head. `head_advanced` computes `old == old → false`, and settlement blocks a PR
the body already repaired.

### Live evidence: W2-311 / PR #1065

System command `cc_25913faa6aa94ba29a664c5588553cc2` woke Codex generation 3 for
failed head `709046f6…`. The body diagnosed a Rust stack overflow and pushed
`02527e29…`; every substantive hosted check passed on the new head (only
structural `scratch-clear` + its `tests-result` roll-up stayed red). Settlement
read the pre-turn observation at `709046f`, marked the wake failed, blocked the
incident, and proposed "did not repair" — while GitHub and the Task PR
publication both held `02527e29`. A settlement observation race, not an agent
failure.

### Why every test misses it

`ci_fix_lifecycle_tests.rs`'s `reconcile()` helper calls `expire_read_cache()`
*first* — it back-dates `github_observation.checked_at` past the TTL so reconcile
really talks to the fake `gh`. Production settlement has no such step. The suite
green-lights the exact path that fails in the fleet: with a warm cache, the
authoritative head is never read.

## The demo

A CI-fix body pushes a real fix to a red PR. Its pre-turn observation is still
cache-fresh at the old head. `settle_ci_fix_turn` reads the authoritative remote
head, sees it advanced, leaves the Task **Waiting** on the new head, re-arms CI
observation there, and records the repaired head on the incident — no
"did-not-repair" block, no second repair body. Reproduced deterministically by a
regression that keeps the pre-turn cache warm while the fake body pushes a new
remote head.

## Approach

Force settlement to read the **authoritative remote head**, bypassing both cache
layers, before it judges head advancement. Everything downstream of a correct
`head_advanced` already works.

### 1. A `Fresh` reconcile freshness, used only at CI-fix settlement

`reconcile_task_pr_with_authority` gains a freshness argument:

- `Cached` (today's behavior) — honor `cached_github_observation`; the default
  for control commands and passive reconciles that must not hammer GitHub.
- `Fresh` — skip `cached_github_observation` entirely and read the remote PR now.

`reconcile_task_pr_for_lease` stays `Cached`. A new
`reconcile_task_pr_fresh_for_lease` passes `Fresh`, and the settlement call site
(`runner.rs:688`) uses it. This is not a new cache — it is a bypass of the
existing one, on the one path that must see the truth.

### 2. Bypass `gh`'s own 60s cache on the fresh read

`observe_pr_by_number` currently always passes `gh api --cache 60s`. That is a
second staleness layer for the head SHA. It gains a `PrReadFreshness` argument:
`Fresh` omits `--cache`, so the settlement read hits GitHub's live REST state.
(`gh pr checks`, which feeds `observe_required_checks`, already runs uncached, so
the re-armed CI reading is live for free.)

### 3. Head advanced → record the response against the new head, re-arm, never block

A full fresh reconcile already refreshes `pr.head_sha()` *and* `pr.ci_observation`
for the new head, and its "open" arm already:

- marks the incident **green** if the new head passes, or
- `observe_ci_incident` for the new head if it fails (a *new* identity → the
  natural re-arm; the old identity is spent and wakes no second body).

So `settle_ci_fix_turn`'s existing `head_advanced` logic, fed an authoritative
`pr`, yields `Waiting` (no block) when the head moved — exactly the intended
outcome. `set_and_record_status` only marks incidents blocked on a `Blocked`
status, so a `Waiting` settlement never false-blocks.

### 4. Durable repaired-head attribution

The incident already ties old head (`failed_head_sha`), system command
(`trigger_command_id`, whose row carries the claiming generation), and
`responded_at` (stamped at body birth). The one missing datum is the **repaired
head**. Add `repaired_head_sha TEXT` to `ci_incidents` and a
`mark_ci_incident_repaired(identity, head, at)` store call, invoked from
settlement when the head advanced. `CiFixWake` carries the `incident_identity`
forward (already on the `CiFix` command kind at arm time) so settlement names the
incident it is completing without re-deriving the identity. Result: one row says
"incident X, woken by command C at generation G, failed at head A, repaired to
head B" — attribution stays automatic.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Where does the stale head come from? | `cached_github_observation` (`ops/task.rs`, 60s TTL) short-circuits reconcile and returns the cached PR without touching `pr.head_sha()`. Confirmed by the test helper `expire_read_cache()` existing solely to defeat it. | Fix is a freshness bypass at the settlement reconcile, nothing more. |
| Is there a second stale layer? | Yes: `observe_pr_by_number` passes `gh api --cache 60s`. `gh pr checks` (checks read) is already uncached. | `Fresh` also omits `--cache` for the head read; checks need no change. |
| Does a fresh reconcile re-arm CI on the new head? | Yes — the reconcile "open" arm calls `observe_required_checks` for the fresh head and either marks green or `observe_ci_incident`s a new identity. | Re-arm is free; no new mechanism. Old identity is spent (accepted), so no second body. |
| Could forcing fresh cause a false block on a genuine outage? | A degraded fresh read sets `session.observation = Degraded`; `decide_open_pr_status` returns `Blocked` with the **github-observation** reason (resume-when-recovered), *not* "did not repair". | "Did-not-repair" fires only when an authoritative read shows the same head still failing — Done-when bullet 3 holds. |
| Is `head_before_turn` the incident head? | `iteration_start_head` = `pr_head_for_session` at runner start = the failing head the wake armed for. The ci-fix path returns before the non-ci-fix `iteration_start_head` update at `runner.rs:876`, so it is stable across the bounded turn. | Existing param wiring is correct; no change. |
| Does the fix keep exactly-once? | Only the read freshness at settlement changes. Wake dedup (`ensure_child_ci_fix_command` on incident identity) and `arm_ci_fix_wake` selection are untouched. | Duplicate/stale/concurrent/restart still launch ≤1 body. Regression asserts it. |
| Migration ordinal race? | `0.11.027` is the current frontier; `0.11.028` is free *now* but siblings race. | Choose the ordinal at land time by scanning **open PRs** for a `ci_incidents` migration, not just the on-disk max (wave MEMORY). The core settlement fix is independent of the column if review defers it. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Read the true remote head via `git ls-remote origin <branch>` | Git refs update atomically on push — zero eventual-consistency window, most authoritative. | The fake body in the test model rewrites `pr.json`, not a git remote; would force real bare-remote push infra into every test. And the CI re-arm still needs the gh read, so we'd run both. A forced-fresh gh reconcile reads head **and** re-arms checks in one path the tests already exercise. Revisit only if gh REST lag proves real (it did not for W2-311: checks had already run on the new head). |
| Compare `head_before_turn` to the worktree's local `git rev-parse HEAD` | Cheap, no network. | The local head advancing does not prove the **remote** moved — a committed-but-unpushed fix would falsely read as repaired, and CI never runs on it. Done-when demands the *remote* head. |
| Always force-fresh on every reconcile | Uniform. | Defeats the read-coalescing the cache exists for; hammers GitHub on every `status`/`follow-up` burst. The staleness only matters at the settlement instant. |
| Widen the observation TTL / add a "post-push" cache | — | Boundary forbids another PR observation cache; a shorter TTL still races a fast body. |

## Key decisions

- **Bypass, don't cache.** The fix reads live at exactly one moment (settlement);
  it adds no state and no second cache.
- **Reuse the reconcile, not a parallel read.** A forced-fresh full reconcile
  refreshes head, CI observation, incident green/observe, and the PR row through
  the one authority (`reconcile_task_pr_with_authority`) — so head-advance,
  re-arm, and settlement can never disagree.
- **Freshness is a caller choice, not a global flip.** Only CI-fix settlement
  asks for `Fresh`. Control commands keep the coalescing cache.
- **Repaired head lives on the incident.** Attribution is a property of the
  incident record, queryable in one row, not reconstructed by joining the PR's
  later observations.

## Scope

- **In scope:**
  - `PrReadFreshness` on `observe_pr_by_number`; `Fresh` omits `--cache`.
  - Freshness argument on `reconcile_task_pr_with_authority`;
    `reconcile_task_pr_fresh_for_lease` for the settlement call site.
  - `CiFixWake.incident_identity`; `mark_ci_incident_repaired`;
    `repaired_head_sha` column + migration.
  - Settlement records the repaired head on head-advance.
  - Deterministic regression: warm pre-turn cache at old head + fake body pushes
    new remote head → `Waiting`, no blocked incident, `repaired_head_sha` = new
    head, exactly one ci-fix command.
- **Out of scope:**
  - `git ls-remote` remote-head reads (alternative, deferred).
  - Any change to wake dedup, arm selection, or the parent lifecycle loop.
  - A generic gate body or a second observation cache (boundary).
  - ENG-19 / W2-311 store surgery — both are terminal; do not reopen, resume, or
    re-work them. This Task owns only the settlement race.
  - W2-319 (stays filed, starts after this settles).

## Done when

- `cargo test -p loopflow --lib runner::ci_fix` passes, including a new
  regression that:
  1. arms one ci-fix wake for failed head `h1`,
  2. runs a fake body that "pushes" `h2` (sets the remote PR head to `h2`) while
     leaving the pre-turn `github_observation` cache-**fresh** at `h1`,
  3. settles, and asserts: Task status `Waiting` (not Blocked), the incident is
     **not** blocked, `repaired_head_sha == h2`, and exactly one `CiFix` command
     exists (no second body), and its state is `Accepted`.
- Sabotage check: reverting the settlement reconcile to `Cached` (or restoring
  `--cache` on the head read) turns the regression red with the "did not repair"
  block — proving the test guards the freshness, not the shape.
- The existing lifecycle test (`a_failed_head_wakes_exactly_one_ci_fix_body_and_
  rearms_until_green`) still passes; its `expire_read_cache()` becomes redundant
  for the settlement step but harmless.
- `cargo clippy -p loopflow --lib --tests -- -D warnings` and `cargo fmt` clean.
- No DTO ripple: `CiIncident` has no `tests/fixtures/dto/` fixture and is not
  emitted by any `bin/` `--json` surface (verified), so the new column adds a
  struct field + SQLite column mapping only.

## Measure

- **Before:** on W2-311/#1065, a body that pushed a green fix was recorded
  "did-not-repair" and the incident blocked. Baseline: 1 known false block.
- **After:** settlement reads the authoritative remote head; a pushed fix settles
  to `Waiting` with the incident carrying `repaired_head_sha`. Target on the
  Developer Efficiency KR "No Task strands on a dead body": zero CI-fix
  settlements that block an incident whose remote head already advanced, over a
  week of real runs.
