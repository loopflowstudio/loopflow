# LOO-584: landing and release agree on who removed a PR's merge request

Status: draft (kickoff 2026-10-25). Stacks on LOO-583 (PR #1679). Source:
LOO-583's 5-whys, Prevention 1–3 (git `4ea55cba4a~1:scratch/loo-583-merge-queue-eviction.md`).

## Problem

When a PR's auto-merge or queue entry disappears, landing and release each decide
on their own whether a person withdrew intent. They use opposite defaults:

- `release.rs` `wait_for_pr_merge` re-arms on any removal except
  `manual_merge_removal_since(armed_at)`. It also skips while
  `release_repair_running`, and `finish_release_pr` resets `armed_at` after a repair.
- `pr_landing.rs` `classify_github_observation` maps "no request" to `Unarmed`,
  which blocks unless `repair_running` or `merge_queue_eviction` finds an eviction.

Each incident taught one path one more cause (#1668, #1673, LOO-583). Release
also re-arms an evicted PR immediately and its repair gate reads only PR-head
checks. A real merge-group failure (green head, red merge result) therefore
re-queues forever and never reaches ci-fix. That is inferred from code, not observed.

## Demo

1. Replay the real PR #1676 eviction and #1654 dequeue timelines through the table
   test. Both callers print the same decision for each fixture.
2. Release integration test: a release PR evicted with `failed_checks` and a green
   head reaches ci-fix with the merge-group run URL in its prompt. Today it loops on
   `pr merge --auto`.
3. A person's dequeue blocks a landing with the person and time in the reason, which
   shows in `lf task status`: `jacklionheart removed #N from the merge queue at
   <time>; run lf arm to resume landing`. Today the reason is "pull request has no
   auto-merge request".

## Risk findings (2026-10-25)

- **GitHub cannot tell Loopflow from a person.** The last 100 loopflowstudio PRs
  show these removals:

  | Count | Event | Reason | Actor |
  |---|---|---|---|
  | 89 | `RemovedFromMergeQueueEvent` | `merged` | — |
  | 61 | `AutoMergeDisabledEvent` | `manually_disabled` | jacklionheart |
  | 9 | `RemovedFromMergeQueueEvent` | `merge_conflict` | — |
  | 9 | `RemovedFromMergeQueueEvent` | `failed_checks` | — |
  | 4 | `RemovedFromMergeQueueEvent` | `manual` | jacklionheart |
  | 3 | `AutoMergeDisabledEvent` | `closed` | — |
  | 1 | `RemovedFromMergeQueueEvent` | `checks_timed_out` | — |

  Most of the 61 `manually_disabled` events are Loopflow's own disarms:
  `enable_auto_merge`'s replace, repair's `clear_task_pr_merge_before_head_mutation`,
  and `land.rs`'s pre-push dequeue. All of them run `gh` with Jack's token. So "by
  Loopflow" has to come from Loopflow's records. Actor and reason cannot decide it.
- **Loopflow already records the needed evidence.** `ci_incidents.repair_lf_process_id`
  plus `processes.started_at`/`completed_at` and `repair_finished_at` give each
  repair's lifetime. A manual-shaped removal inside a repair's lifetime is that
  repair's disarm. No schema change is needed. Only the latest attempt per incident
  is retained, which is enough because only the latest removal event is classified.
- **Loopflow disarms outside a repair act for the person running the command**
  (`lf sync`/`lf commit` on an armed Task, mode → User). They count as Manual, which
  matches today's blocking behavior.
- **Run lookup can hand ci-fix the wrong run.** `failed_merge_group_run` takes the
  newest non-success run for `pr-N`. After a `failed_checks` eviction, a later
  `merge_conflict` eviction, which has no run, reports the old run. A busy queue can
  also push the run out of the 100-run page. GitHub's runs API accepts
  `created=<from>..<to>`, and `AddedToMergeQueueEvent` gives the start of the window.
- **Item 3 mostly holds already.** `ci_incident` digests each check's name and URL.
  A distinct run URL is already a distinct incident. The gap is the run lookup
  above, not the dedupe.
- **LOO-583 broke a release test.** It renamed the timeline query to
  `LoopflowPrMergeRequestEvents`. `tests/release_tests.rs` `write_gh_merge_wait_script`
  still matches `*LoopflowPrMergeRemovals*`, so
  `release_run_waits_out_a_repair_disarm_but_stops_on_a_persons` fails locally
  (verified 2026-10-25). PR #1679 CI was pending at kickoff. If #1679 lands with the
  fix, rebase over it. Otherwise this branch rewrites that fixture anyway.

## Chosen approach

**One reader, one decision.** `pr.rs` owns the removal reader. `pr_landing.rs` owns
the single decision function over (PR state, request, removal, head checks).
Release reuses that decision instead of keeping its own.

```rust
// pr.rs
pub(crate) enum MergeRequestRemoval {
    /// A person dequeued the PR or disabled auto-merge.
    Manual { at, actor: Option<String>, queued: bool },
    /// A Loopflow CI repair disarmed it while repairing this PR.
    ByLoopflow,
    /// GitHub's queue dropped it; `run_url` is the merge-group run for this entry.
    Evicted { reason: String, run_url: Option<String> },
    /// The queue removed it because it merged; the PR read has not caught up.
    Merged,
    /// No timeline event explains the missing request.
    Unknown,
}
pub(crate) fn merge_request_removal(repo, number, repairs: &[RepairWindow]) -> OpsResult<MergeRequestRemoval>
```

The latest merge-request timeline event decides. If it is an arm (`AutoMergeEnabled`
or `AddedToMergeQueue`), or there are no events, the result is `Unknown`. A
manual-shaped event inside a repair window is `ByLoopflow`. `RepairWindow` is
`[process started_at, repair_finished_at or completed_at]`. The window stays open
only while `repair_live`. A dead repair with no end record has no window.

Decision when the PR is open and has no request (`classify_github_observation`):

| Removal | Decision | Landing | Release |
|---|---|---|---|
| Manual | Withdrawn{removal} | block, naming actor and time | `Err(ManualMergeRemoval)` |
| Merged | Pending | re-observe | keep polling |
| Evicted | Failing `merge-queue (<reason>)` + run URL | ci-fix | NeedsRepair → ci-fix |
| ByLoopflow / Unknown | read head checks: failing → Failing(real checks); pending → Pending; green → Failing `merge-request (<cause>)` | as decided | failing → NeedsRepair; green → NeedsRepair |

A live repair already makes the landing's `Failing` arm wait (`repair_running`
→ `Ok(())`). That one check replaces both special cases, the landing's `Unarmed`
check and release's `release_repair_running`. Release's NeedsRepair path then
calls `reconcile_armed_pr` → the same classifier → the same incident, and waits in
`wait_for_release_repair`.

**Release stops classifying and re-arming on its own.** `wait_for_pr_merge` keeps
its merged, closed, integration and timeout handling. Everything else maps from the
shared decision:

- Withdrawn → `ManualMergeRemoval`
- Failing → `NeedsRepair`
- Pending / Passing → keep polling
- Merged → `Merged`

ci-fix re-arms with `lf arm` when the removal turns out to be harmless, so the
incident dedupe bounds every loop: one repair per (head, evidence).

**Run lookup bound to the queue entry.** Search
`actions/runs?event=merge_group&created=<added_at>..<removed_at>` for `pr-N-`.
The window runs from the latest `AddedToMergeQueueEvent` before the eviction to the
eviction itself. No queue-add event means no window and no URL. Each eviction then
carries its own run or none, and the existing digest makes the second eviction new
evidence.

## Alternatives rejected

- **Release keeps re-arming directly on non-manual removals.** It is cheap, but it
  has no bound. That is the forever-requeue this Task exists to remove.
- **Record Loopflow's own disarms in a new table, or keep timeline event ids.** Exact,
  but it needs a migration and a write at four disarm sites. Repair windows already
  cover the one case where Loopflow's disarm must not stop work. The other sites
  act for a person.
- **Put the eviction time in the synthetic check name to separate evictions.**
  Redundant once the run lookup is exact. It would also churn the identities of
  `merge_conflict` evictions that have no run.

## Delete — do not maintain

- `pr.rs`: `ManualMergeRemoval`, `manual_merge_removal_since`, `latest_manual_removal`,
  `MergeQueueEviction`, `merge_queue_eviction`, `latest_queue_eviction`.
  Their tests are `only_a_manual_removal_after_the_run_armed_stops_a_release` and
  `only_the_queues_own_latest_removal_reads_as_an_eviction`. Fold their fixtures
  into the table test.
- `pr_landing.rs`: the `repair_running` special case in the `Unarmed` arm, and the
  `Unarmed` variant itself, replaced by `Withdrawn { head_sha, removal }`. Also the
  re-read of `merge_queue_eviction` in `process_ci_fix`: the incident's check
  already carries the URL. Thread `CiCheck` URLs through instead.
- `release.rs`: `release_repair_running`, the re-arm block and the
  `manual_merge_removal_since` call in `wait_for_pr_merge`, and its own
  `merge_gate_state` NeedsRepair check. Also `PreparedRelease.armed_at` and every
  `armed_at` reset.
- `tests/release_tests.rs`: the stale `LoopflowPrMergeRemovals` fixture branch and
  the `2099-01-01` "stamped after any arm" trick, which only existed for `armed_at`.

Survive: `OpsError::ManualMergeRemoval` (cron accounting/history read it),
`repair_running`, `wait_for_release_repair`, `NeedsIntegration`, incident dedupe.

## Forbidden outcomes

- Release or landing keeping a private exception list next to the shared decision.
- Release re-arming on its own after a removal.
- Reading GitHub's actor or `manually_disabled` as proof that a person removed the request.

## Exclusions

- Docker readiness (LOO-583 item 4).
- Recording lf's non-repair disarms durably.
- Webhook-driven observation.
- Release's `NeedsIntegration` rebuild.

## Known limits (accepted)

- A person who dequeues in the seconds between a repair's `lf arm` and that repair
  Process exiting is read as ByLoopflow. The landing re-repairs; it does not stop.
- A repair that dies with no end record leaves its disarm read as Manual, so the
  landing blocks. That errs toward stopping.

## Done when

- A person's dequeue or disable stops both landing and release. Nothing else does.
- A release PR evicted with a green head reaches ci-fix with that eviction's
  merge-group run. It does not re-arm in a loop.
- Two evictions on one head with different runs admit two repairs. A `merge_conflict`
  eviction after a `failed_checks` one carries no stale run.

## Acceptance checks (gate, headless)

From `rust/`, with inherited `LF_*` unset and `< /dev/null`:

- `cargo test -p loopflow --lib -- ops::pr ops::pr_landing ops::ci_watch`. The table
  test `merge_request_removals_decide_landing_and_release_alike` covers manual
  dequeue, manual disable, `failed_checks` eviction, repair disarm (inside and outside
  a window), merged, and no event. It asserts the landing observation and the
  release wait for each.
- `cargo test -p loopflow --test release_tests release_run`. Includes
  `release_run_waits_out_a_repair_disarm_but_stops_on_a_persons` (fixture repaired)
  and new `release_run_repairs_an_eviction_with_a_green_head`. The latter expects
  a ci-fix prompt containing the merge-group run URL and no second `pr merge --auto`.
- `pr_landing` test: two `failed_checks` evictions on one head, with runs A then B,
  produce two distinct incident identities. The second is admitted.
- `cargo clippy -p loopflow --all-targets -- -D warnings`, `cargo fmt --check`.

## Slices (one PR)

1. **This slice:** `pr.rs` reader + repair windows (store query in
   `ci_incidents.rs`) + exact run lookup, with the table test over reader output.
2. Shared decision in `classify_github_observation` (`Withdrawn`, ByLoopflow/Unknown
   head-check fallthrough). Delete the landing special cases.
3. Release maps from the shared decision. Delete its classification, re-arm and
   `armed_at`. Repair and extend `release_tests`.
4. ci-fix.md: one line for `merge-request (<cause>)`: verify checks, then `lf arm`.

Check results: kickoff 2026-10-25, `cargo test --test release_tests
release_run_waits_out_a_repair_disarm` FAILED on base 4ea55cba4a (stale query name
from LOO-583); owner: this branch, or #1679 if it fixes first.
