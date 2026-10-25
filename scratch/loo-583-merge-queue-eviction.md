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

**Verification.** `cargo test -p loopflow --lib -- ops::pr_landing ops::pr::tests
ops::ci_watch`: 69 passed. `pytest python/tests/test_task_installation.py`: 8 passed.
Clippy clean. Replayed the timeline query and run lookup against PR #1676.

**Unproved.** A live eviction → watcher → ci-fix → re-arm on an installed build. An
evicted PR whose merge-group run is older than the latest 100 merge-group runs
gets no URL, and a second eviction on the same head then hits the
"already completed a repair" dedupe.
