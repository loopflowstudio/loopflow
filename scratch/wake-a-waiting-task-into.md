# W2-156 — Wake a waiting Task into `ci-fix` when its PR fails

## User-visible outcome

Today a Task that opens a PR settles into `Waiting` with no live process (correct:
it must not burn a provider body while checks run). But when a required check then
**fails** on the current PR head, nothing happens — the developer has to notice the
red X and hand-run `lf task resume`. After this Task holds, Loopflow itself wakes the
same durable Task into one bounded `ci-fix` turn: it repairs the existing branch,
pushes to the same PR, and goes back to sleep watching the new head. The developer
observes the transition through `lf status` (`waiting on CI` → `fixing CI` → back to
`awaiting review`) and never rescues the worker from its worktree.

## Source of truth

The **Task PR record** (`TaskPr` + its `GithubPr` receipt, `task/mod.rs`) is the
authoritative persisted record. It gains two pieces of durable state:

1. **`head_sha`** on `GithubPr` (today it is only `{ number, url }`). Populated from
   GitHub during PR reconcile. CI evidence is authoritative *only* for this head.
2. A **CI observation** on the PR publication recording the last head SHA and the
   failing-check set that already triggered a wake — the dedup key. No new store,
   no CI-specific table (operational boundary): it rides the existing PR row.

GitHub check-runs are read live (never trusted stale); everything derived — `lf status`
next-move owner, the ci-fix wake decision — reads from this record + a fresh read for
the current head only.

## Mechanism (the four transitions)

1. **Pending is healthy waiting.** `reconcile_task_pr_with_authority` (`ops/task.rs:1316`)
   already reads open/merged/closed. Extend it to also read the current head SHA
   (`gh pr list … --json …,headRefOid`, extending the query at `ops/pr.rs:312`) and the
   **required** check conclusions for that head (reuse the `lf wt ci` pattern —
   `gh pr checks <branch> --required` / `statusCheckRollup`, `lf/commands/ops/mod.rs:1498`).
   Pending checks persist head SHA + `pending` state and run **no** provider body.

2. **A required check failing on the current head wakes exactly one `ci-fix` turn.**
   When the reconcile sees ≥1 required check `FAILURE` at the current head *and* that
   `(head_sha, failure_set)` differs from the last-woken record, it enqueues one
   `ChildCommandKind::CiFix { pr_number, url, branch, head_sha, failing_checks, log_urls }`
   on the **existing** child-command path, then stamps the dedup key. The wake mints a
   generation through the existing lease CAS (`reserve_task_process`), so the
   exactly-one-body / write-lease invariant is unchanged — a second wake attempt while a
   ci-fix generation is live cannot reserve a second body.

3. **The woken generation runs the builtin `ci-fix` skill, not the task flow.** The
   Task runner (`task/runner.rs:97`) hardcodes `QueuedInvocation::load(worktree, "task")`.
   When the startup command drain finds a `CiFix` command, it loads a new single-step
   builtin flow `ci-fix.yaml` (step: the existing `ci-fix` skill) and seeds the body with
   the canonical metadata from the command, so the skill resolves the exact failure
   without re-deriving it. After the body completes and pushes (head SHA advances), the
   runner reconciles, returns to `Waiting`, and rearms observation for the **new** head.

4. **Green stays waiting; a new failure may wake one new turn.** After the push, the new
   head starts a fresh check cycle. Green → `Waiting`, owner = the configured
   review/merge action (no body). A new required failure at the new head is a new
   `(head_sha, failure_set)` → one new ci-fix turn.

## `LaunchIntent::CiFix` — the third intent

`supervisor_restart_bar` (`task/mod.rs:368`) correctly bars a **supervisor** from
restarting a `Waiting`-on-`Open`-PR Task (the W2-129 incident: a blind wake re-ran
`task_clarify` over already-submitted work). `ExplicitResume` (a human `lf task resume`)
bypasses the bar. The ci-fix wake is neither: add `LaunchIntent::CiFix` (`ops/child.rs:443`)
that is allowed past the open-PR bar **only when accompanied by fresh required-failure
evidence for the current head** — carried by the `CiFix` command. It is not a blind
restart (it never re-opens the flow at `task_clarify`; it runs only the ci-fix body) and
it is not human — it is an evidence-gated automated wake. The bar for supervisor and the
override for the operator both stay exactly as they are.

## Manual resume dominates (W2-144 generation 7)

The atomic in-loop boundary `claim_task_commands_or_stop_for_lease`
(`store/sqlite/child_sessions.rs:922`) already drains queued commands before committing
the open-PR `Waiting` settle. The discard window is the **out-of-loop** settle path
`reconcile_process_liveness` (`ops/task.rs:1271-1296`): it sets `Waiting` on an open PR
with no queue consultation and no relaunch, so a `lf task resume` enqueued around that
transition is orphaned. Fix: before settling a dead-process open-PR Task to `Waiting`,
consult the command queue — if a `Resume` (or `CiFix`) is queued, relaunch and let the
new generation drain it instead of settling. This makes "manual resume is consumed before
the default open-PR waiting transition" true, and the ci-fix wake reuses the same bridge
(one relaunch path, two producers).

## Affected surfaces and consumers

- **`ops/pr.rs`** — add `headRefOid` to `current_or_merged_pr_for_branch`; `PrInfo` gains
  `head_sha`.
- **`task/mod.rs`** — `GithubPr` gains `head_sha`; PR publication gains the CI observation
  (last head + woken failure set); new `ChildCommandKind::CiFix { … }` (`child_session.rs:339`).
- **`ops/task.rs`** — extend `reconcile_task_pr_with_authority` with the required-check read
  + wake decision + dedup stamp; bridge `reconcile_process_liveness` to the queue.
- **`ops/child.rs`** — `LaunchIntent::CiFix`; wake enqueues via `queue_command`.
- **`task/runner.rs`** — startup drain recognizes `CiFix`, loads the ci-fix flow, seeds the
  body, settles back to `Waiting` after; carries an infra-blocker reason into `Blocked`.
- **New builtin `engine/builtins/build/flow/ci-fix.yaml`** — single step `ci-fix` (skill
  already exists at `build/skill/ci-fix.md`; it already consumes injected PR#/branch/head
  SHA/logs).
- **`lf/commands/waves.rs`** — wire the already-declared `NextMoveOwner::Ci` producer:
  pending → `Ci` ("waiting on CI"); failed + ci-fix queued/running → `Task` ("fixing CI");
  infra-blocked → `NeedsAttention`; green → `Review` (or configured owner). This surfaces
  pending/fixing/blocked/green truthfully.
- **Wire DTOs** — `GithubPrSnapshot` (`waves.rs`) mirrored in Swift `WaveWorkMap.swift`
  and DTO fixtures `roadmap_snapshot.json` / `wave_detail.json`. Any exposed head-SHA / CI
  field follows the no-defaults DTO rule and adds to the fixture + each language mirror.
  (Internal `TaskPr.head_sha` persistence is required; exposing it on the wire is a display
  choice — expose only what `lf status` renders.)

## Absent and error states

- **No PR / gh unavailable** — reconcile returns without CI state (as `gh_available()`
  already guards). No wake, no crash.
- **Head moved between read and wake** — dedup key is per-head; a wake computed against a
  stale head is dropped when the current head differs. Stale failures never wake work.
- **Checks still pending** — healthy waiting, no body.
- **Infra / credential blocker** — the ci-fix body reports the blocker and makes no
  worktree change; the runner settles `Blocked` (matching the existing
  "completed without a PR or any worktree change" path, `runner.rs:454`) with the blocker
  reason visible in `lf status`. Tests are never weakened to go green.
- **Duplicate poll / webhook delivery** — same `(head_sha, failure_set)` → dedup skip. No
  second command, no second body.
- **Coalesced simultaneous failures** — the failure_set is the union of required failures
  at one head; one wake carries all failing check names.

## Operational boundary

One Task, one worktree, one provider history, one active PR sequence. No second CI worker
identity, no duplicate Task, no new worktree/transcript, no CI-specific monitor store.
CI evidence authoritative only for the PR current head. The wake rides the existing
child-command path; execution reuses the existing lease/generation machinery.

**Cadence driver (decision):** CI reconciliation is folded into the *existing* Task-PR
reconcile, which the supervising loop already calls each pass (project loop
`inspect_outcome` → `reconcile_task_pr`, `project_session/runner.rs:768`; also `lf status`).
Because the reconcile is idempotent and deduped by `(head_sha, failure_set)`, the driver is
pluggable — project pass, `lf status`, or a reconcile cron all produce at most one wake. The
integration harness drives the reconcile directly. Guaranteeing a bounded wake *latency* for
an unsupervised waiting Task (a periodic reconcile tick over waiting-on-PR Tasks) is the one
follow-on named in Exclusions.

## Exclusions

- **Webhook receiver.** Poll/reconcile is the baseline; a GitHub webhook is a latency
  optimization, out of scope. (The dedup key makes a future webhook safe to add.)
- **Bounded-latency reconcile cron** over unsupervised waiting Tasks — separate wiring;
  correctness proven here, cadence deferred.
- **Retry caps / non-convergence records** for repeated ci-fix churn on the same PR beyond
  what the failure-set dedup already gives — the KR-level "streak/non-convergence" concern
  lives with the broader task-loop-trust bet, not this Task.
- **Non-required check failures** never wake work.

## End-to-end proof

A deterministic integration harness (mock `gh` / check-run reads, in-memory store, scripted
harness like `task/runner.rs` tests) opens a PR and asserts:

1. Pending checks → `Waiting`, no body reserved (lease generation unchanged).
2. One required failure at the current head → exactly one `CiFix` command, one new
   generation, a ci-fix body seeded with `{pr#, url, branch, head_sha, failing_checks,
   log_urls}`.
3. Duplicate delivery and a multi-check failure at the same head coalesce to one wake
   (dedup by `(head_sha, failure_set)`); no second body.
4. The fix pushes → head SHA advances → Task returns to `Waiting`, dedup key rearmed for the
   new head; a failure at the new head wakes one new turn.
5. Green CI → `Waiting`, owner = review/merge; no body.
6. An infra failure → `Blocked` with an actionable blocker reason and zero worktree change.
7. **W2-144 gen 7**: a queued `lf task resume` on an open-PR Task is consumed once by a
   relaunched generation — never discarded because the PR is open — and takes precedence over
   the default open-PR waiting settle.

`lf status` reflects `waiting on CI` / `fixing CI` / `blocked` / `awaiting review` through
the `NextMoveOwner` producer across these transitions.

## Serial PR plan (this Task, ordered branches)

- **PR 1 — truthful CI observation.** Persist `head_sha` + CI observation on `TaskPr`;
  extend the reconcile to read required-check state for the current head; wire
  `NextMoveOwner::Ci` + fixing/blocked/green in `lf status`; DTO/fixture/Swift updates.
  No wake yet. Proof: reconcile records head + check state; `lf status` shows the four
  CI-derived owners.
- **PR 2 — the wake + ci-fix turn.** `LaunchIntent::CiFix` past the open-PR bar on fresh
  evidence; `ChildCommandKind::CiFix` + dedup; `ci-fix.yaml` flow and the runner's ci-fix
  turn + rearm; bridge `reconcile_process_liveness` to the queue (W2-144 fix). Proof: the
  full integration harness above.
</content>
</invoke>
