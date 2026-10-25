# LOO-583: merge-queue eviction left a landing dropped

**Symptom.** PR #1676 merge-group run 40166998506 failed only `task-installation`
(`docker info` timed out after 10s, right after the Cargo cache restore; the image
step had reached Docker seconds earlier). GitHub's queue removed the PR
(`RemovedFromMergeQueueEvent reason=failed_checks`, actor `github-merge-queue`) and
cleared auto-merge. The landing check read "no merge request" as a person revoking
intent and blocked: "run lf arm to resume landing".

**Repair.**
- `pr.rs`: `merge_queue_eviction` reads the PR's latest merge-request event; a queue
  removal that isn't manual or `merged` is an eviction, carrying the failed
  merge-group run URL. A later arm, dequeue or manual disable supersedes it.
- `pr_landing.rs`: an unarmed landing with an eviction is `Failing` on
  `merge-queue (<reason>)`, so ci-fix gets the run URL and either re-arms (runner
  fault) or repairs. A manual disarm still blocks.
- `ci-fix.md`: how to read a merge-queue failure; re-arm, don't rerun the run.
- `test_task_installation.py`: waits up to 120s for Docker before failing as an
  infrastructure fault.

A compress pass dropped `MergeQueueEviction.at` (never read) and flattened the
ci-fix URL lookup; behavior unchanged.

**Verification.** `cargo test -p loopflow --lib -- ops::pr_landing ops::pr::tests
ops::ci_watch`: 69 passed (2026-10-24, after compress; one unnamed failure in
the first of three runs, not reproduced — gate should watch for a flake). `pytest python/tests/test_task_installation.py`: 8 passed.
Clippy clean. Replayed the timeline query and run lookup against PR #1676.
Gate (2026-10-25): `cargo fmt --check`, `cargo clippy -p loopflow --all-targets -D warnings`,
`ruff check`/`format --check` clean; focused Rust suite 69 passed twice (no flake
reproduced; tests wait >60s each on shared-lock contention, 1.3s alone, pre-existing);
pytest 8 passed. Full matrix deferred to CI.

**Unproved.** A live eviction → watcher → ci-fix → re-arm on an installed build. An
evicted PR whose merge-group run is older than the latest 100 merge-group runs
gets no URL, and a second eviction on the same head then hits the
"already completed a repair" dedupe.

## Why it happened (5 whys)

**Affected.** LOO-578's landing (PR #1676) sat blocked ~12 minutes with green PR
checks until the repository conversation ran `lf arm` by hand. Recovery: PR re-armed
manually; the code repair is committed on this branch (767ee9976f), unpublished
and unproved on an installed build.

1. **Why was the landing left blocked?** `classify_github_observation` mapped
   "no merge request" to `Unarmed`, and `Unarmed` blocks with "run lf arm" unless a
   repair worker is live (`pr_landing.rs`, the `Unarmed` arm). The queue's own
   removal fell into the person-withdrew-intent bucket.
2. **Why does "no request" mean "a person withdrew"?** The landing's default is
   *block*, with exceptions added one cause at a time. Git history shows the
   exceptions arriving as incidents: #1668 (03:03 UTC, a person's dequeue must
   stop a release), #1673 (05:09 UTC, lf's own repair disarm must not block),
   now this eviction (05:50 UTC). Three PRs in three hours each taught one path
   one more reason auto-merge disappears.
3. **Why did each fix only cover its own cause?** The question "who removed the
   merge request?" has two owners with opposite defaults. `release.rs`
   (`wait_for_release_pr`, ~3770) re-arms on *any* removal except a manual one
   (`manual_merge_removal_since`); `pr_landing.rs` blocks on any removal except
   a repair or, now, an eviction. Both read the same GitHub timeline through
   `pr.rs`, but each encodes its own policy, so a cause handled in one is
   unhandled in the other.
4. **Why was the eviction invisible as a failure?** GitHub attaches merge-group
   runs to `gh-readonly-queue/*/pr-<n>-*`, not to the PR head's status rollup.
   Every lf reader looked at PR-head checks, which stayed green. The only signal
   was a timeline event (`RemovedFromMergeQueueEvent reason=failed_checks`) that
   nothing read before this branch.
5. **Why did the merge group fail at all?** `task-installation` probed
   `docker info` once with a 10s timeout (added 2026-09-28, #1308, to fail fast
   on a *hung* daemon). A transient stall after the Cargo cache restore looked
   identical to a hung daemon, and the merge group re-ran the job instead of
   reusing the PR run's proof.

Root conditions: (a) merge-intent classification is duplicated with opposite
defaults; (b) the merge queue is a CI stage lf did not observe; (c) a single-shot
environment probe with no transient tolerance. "Someone forgot to re-arm" is not
a cause: no signal told anyone the landing had dropped.

### Branches and contradicting evidence

- **Release has the mirror bug.** On an evicted release PR, `release.rs` re-arms
  immediately. A runner fault: fine. A *real* merge-group failure (green PR
  head, red merge result, e.g. a semantic conflict with main): it re-queues, the
  queue evicts again, and `merge_gate_state` only reads PR-head checks, so it
  never returns `NeedsRepair`. Inferred from code, not observed.
- **Landing dedupe.** A second eviction on the same head with the same synthetic
  check name hits "CI incident already completed a repair" and blocks again,
  even when the second cause differs. Already listed under Unproved.
- **Run lookup.** `failed_merge_group_run` scans the latest 100 merge-group runs;
  busy queues may miss it, leaving ci-fix to search by itself.
- Unanswered: why the daemon stalled after cache restore (disk I/O on the hosted
  runner is the guess; no runner metrics were pulled). Whether 120s is enough is
  unmeasured.

### Prevention (proposed, not approved)

1. **One merge-intent reader** in `pr.rs`, e.g. `merge_request_removal(repo, pr,
   since) -> Removal { Manual | Evicted{reason, run} | ByLoopflow | Unknown }`,
   used by both landing and release. Each caller then decides only what to do,
   not who did it. Default for non-manual removals is *keep intent*; only
   `Manual` blocks. Proof: one table test over timeline fixtures (manual dequeue,
   manual disable, failed_checks eviction, repair disarm, merged) asserting both
   callers' decisions. This deletes the per-path exceptions rather than adding a
   fourth.
2. **Treat the merge group as a check source.** Release's repair gate should see
   `merge-queue (<reason>)` the same way landing now does, so a real merge-group
   failure routes to ci-fix instead of an endless re-arm. Proof: release wait
   test with an eviction whose PR head is green returns `NeedsRepair`.
3. **Key eviction incidents on the merge-group run**, not just the head, so a
   second eviction is new evidence. Proof: two evictions with different run URLs
   on one head admit two repairs.
4. Docker readiness is fixed on this branch (120s wait). No further change
   unless it recurs; if it does, pull runner disk metrics before raising limits.

## Launch decision (2026-10-24)

This Task keeps the shipped repair (767ee9976f) as its core: publish it as-is.
Prevention (1)–(3) is filed as LOO-584, "Landing and release agree on who
removed a PR's merge request", under infrastructure, not launched: `--stack-on`
needs LOO-583's published PR. Launch it with
`lf task run LOO-584 --stack-on LOO-583` once that PR exists, or bare after it
lands. Item (4) needs no Task unless Docker readiness recurs.

## Concept review (2026-10-25)

No change. The repair keeps one product contract: a merge request the queue
dropped is a CI failure for repair; only a person's dequeue or disable withdraws
intent. Rechecked PR #1676's live timeline: GitHub's eviction leaves no
`AutoMergeDisabledEvent` behind it, so "latest event is the eviction" holds, and a
later arm or `merged` removal supersedes it. The one-reader and release-parity
concepts stay with LOO-584; no open product choice here.
