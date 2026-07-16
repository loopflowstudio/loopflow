# W2-231 — Make ci-fix infrastructure failures block the Task actionably

## Outcome

When a ci-fix body cannot repair a PR because infrastructure is unavailable,
the Task turns red (Blocked) with an actionable reason instead of quietly
returning to Waiting. No silent loss, no false progress.

## Root cause (today)

A "ci-fix" run is the Task body running its iterate flow with a ci-fix
directive — there is no distinct ci-fix flow. After each completed turn the
runner's status decision block (`task/runner.rs:601-700`) picks the status.
For an open PR it always picks **Waiting** "pull request #N is open for review"
(`runner.rs:653-664`), ignoring `ci_observation` entirely. So:

- **No-change agent report** (agent completed, CI still Failing on the same
  head) → Waiting. Indistinguishable from a healthy PR. This is the silent
  false-progress bug.
- **GitHub observation failure** (`gh pr checks`/`gh pr list` errored) →
  `required_check_state`/`observe_required_checks` return `None`, overloaded
  with "no required checks configured" and "gh not installed". So an
  observation failure looks identical to a healthy PR → Waiting (silent), or
  `gh pr list` error propagates → `record_unhandled_failure` → Failed with a
  raw error string (red, but not actionable and not "Blocked").
- **Provider outage** → `Lifecycle::Failed` → `finish_failed` → Failed with
  "provider turn failed" (red, raw, not actionable, not "Blocked").

The only no-progress signal the runner has is the worktree fingerprint
(`task_state_fingerprint`), used only for the no-PR "would spin" path. The
directive forbids relying on worktree churn for the ci-fix distinction.

## Design

Replace the open-PR → Waiting decision with a CI-aware decision, and route
infrastructure failures to Blocked. The non-churn signals are GitHub CI state
on the PR head + provider turn lifecycle + GitHub command results — never the
worktree fingerprint, never agent free-text.

### Three ci-fix outcomes (derived from observable signals)

- **Repaired / progress** — open PR, CI Passing or Pending on the head, OR the
  PR head advanced during the iteration (agent pushed a fix; CI is in flight).
  → **Waiting** "pull request #N is open for review" (healthy observation /
  human-review boundary). Preserves the auto ci-fix retry loop.
- **InfraBlocked(capability)** — provider turn `Lifecycle::Failed` (capability
  `provider`), or GitHub observation failed (capability `github-observation`).
  → **Blocked** "ci-fix blocked by <capability>: <detail>. <next action>. PR
  #N stays attached."
- **NotRepaired (no-change)** — open PR, CI Failing on the head, AND the head
  did NOT advance during the iteration (agent pushed nothing). → **Blocked**
  "CI failing on pull request #N; the Task body did not repair the head. Needs
  a new directive or human review. PR #N stays attached."

The no-change signal is **PR head non-advancement** (GitHub-observable), not
worktree churn. Captured as `iteration_start_head` at the iteration boundaries
where `state_fingerprint` is (re)set.

### Changes

1. `ops/pr.rs` — `required_check_state` returns `RequiredCheckReading`
   (`Observed(RequiredChecks)` | `NoRequiredChecks` | `Unavailable` |
   `ObservationFailed(String)`), splitting the overloaded `None`. Pure
   classification helper `classify_required_check_reading` for testability.

2. `ops/task.rs` — `observe_required_checks` returns `ObserveOutcome`
   (`Observed(Option<CiObservation>)` | `InfraFailed(String)`). Mapping:
   `Unavailable`/`NoRequiredChecks`/no-head → `Observed(None)` (healthy
   unknown); `ObservationFailed` → `InfraFailed(detail)`; `Observed(c)` →
   `Observed(Some(CiObservation))`.

3. `ops/task.rs` — `reconcile_task_pr_with_authority` returns `ReconciledPr {
   pr, github_observation_failed }`. A `current_or_merged_pr_for_branch` Err
   (gh present but failed) becomes the infra signal instead of propagating;
   the last-known PR row is kept so the runner can block with it attached.
   Open-PR status is decided by the shared pure `decide_open_pr_status`
   (used both in reconcile for the inactive-body path and in the runner
   decision block). Wrappers: lease wrapper returns `ReconciledPr`; no-lease
   wrapper + `ensure_working_pr`/`pr_next` preserve old bail-on-gh-error
   behavior (return Err when `github_observation_failed` is Some).

4. `task/runner.rs` — capture `iteration_start_head` alongside
   `state_fingerprint` at the iteration boundaries. Replace the open-PR
   Waiting branch with `decide_open_pr_status(pr, github_observation_failed,
   head_advanced)`. Add `finish_infra_blocked` (Blocked + StatusChanged +
   finish process + `Ok(())`). Provider `Lifecycle::Failed` with an active PR
   → `finish_infra_blocked(provider)`; without a PR → `finish_failed` (kept).

5. `lf/commands/waves.rs` — `next_move_for_task`: a Blocked task with an Open
   PR routes to `NextMoveOwner::Project` (not Ci), so a failing ci-fix stops
   silently re-looping and surfaces for a directive/human review. Waiting +
   Open PR keeps CI-derived routing (auto-loop intact).

6. `engine/builtins/build/skill/ci-fix.md` — note that infra blockers now
   transition the Task to Blocked with the failing capability; the runner
   detects provider/GitHub outages independently, so the agent should report
   the failing capability and safe next action (it no longer has to fall back
   to Waiting).

### What stays

- The no-PR "would spin" fingerprint path (`runner.rs:665-699`) is untouched —
  not a ci-fix scenario.
- `gh` not installed → `Unavailable` → healthy unknown → Waiting (envs without
  gh are not penalized).
- PR detach/settle only on merge/close; Blocked never detaches (KR3).
- Blocked is non-terminal → `lf task resume` works after infra recovers (KR3).

## Tests

- `provider outage` — completed turn `Lifecycle::Failed` + active open PR →
  Blocked, capability `provider`, PR still attached.
- `github observation failure` — `required_check_state`/`gh pr list` error +
  active open PR → Blocked, capability `github-observation`, PR attached.
- `no-change agent report` — completed turn, CI Failing, head unchanged →
  Blocked (not Repaired); companion: head advanced + CI Pending → Waiting
  (progress, no false block).
- `decide_open_pr_status` pure unit tests for all branches.
- `classify_required_check_reading` pure unit tests (gh error vs no checks vs
  unavailable).
- `next_move_for_task`: Blocked + Open PR → Project; Waiting + Open + Failing
  → Ci (auto-loop preserved).

## Open questions

- None blocking. Edge case "agent pushed but new head's CI already Failing"
  → treated as Waiting (head advanced = progress, let CI resolve); acceptable
  and conservative.
