# Release memory

Release is the nested Wave `infrastructure/release`. Jack Heart selected its
objective and ownership of release-focused planning on 2026-09-28. Its GOAL.md
owns the authored release schedule. Live Linear ownership and schedule cutover
remain pending until the subwave implementation lands and installed `lf` applies
the split; authored files do not establish that cutover.

## Scheduled release failure boundaries (2026-09-28)

Jack Heart requested autonomous prevention and v0.12.24 delivery after
[the scheduled release PR #1309](https://github.com/loopflowstudio/loopflow/pull/1309)
failed. [Release recovery · LOO-328](https://linear.app/loopflow/issue/LOO-328)
tracks [the prevention PR #1311](https://github.com/loopflowstudio/loopflow/pull/1311).
The Swift prevention also addresses part of LOO-326; its other three faults
remain open. LOO-285 retains broader scheduled-opportunity accounting and
observation windows.

**Observed chain:** required Swift CI timed out → GitHub correctly blocked
merge → release's private wait never inspected failed checks or entered CI
repair → the reporting agent exited zero → cron recorded success without a
published release. The September 28 10:00 PDT firing created the PR at 17:09
UTC. Swift built in 115 seconds, then emitted no progress before the 20-minute
step timeout. Release polled until its one-hour timeout. The cron receipt
`cron_fefee553f15d40408dc9b8cce4435e69` records 17:00:06–18:10:15 UTC and exit 0;
the host log retains the agent's explicit failure report.
[Swift job evidence](https://github.com/loopflowstudio/loopflow/actions/runs/36456179108/job/109042980641)
and [the required aggregate gate](https://github.com/loopflowstudio/loopflow/actions/runs/36456179108/job/109052053064)
remain the hosted observations.

- **An entry point must preserve recovery ownership.** Release called only
  `finish_arm_after_rebase`, removed its checkout, and watched mergeability.
  Ordinary landing's CI repair therefore never ran. `release.rs` now observes
  failed required checks, materializes or reuses its checkout, and enters
  `pr_landing::watch_armed_pr`. Blocked repair retains authored work. Release
  still owns version metadata rebuilding when main advances, including during
  repair. Tests through the release entry point prove simulated failed-check
  repair before tagging; ordinary landing tests alone missed this boundary.
- **Cron must observe the operation's result.** A successful report of failure
  is not successful release execution. `.lf/flows/release-run.yaml` invokes
  `release run patch` directly. Cron prefers this Flow over the same-named
  skill; operation errors reach its exit status. Blocking operations run off
  the async Flow executor so the landing controller can enter its own runtime.
  The CLI Flow regression proves a publisher failure remains nonzero.
  Existing hosts must sync cron with the updated installed CLI and checkout;
  a committed Flow alone does not replace a loaded skill target.
- **Bound async fixture cleanup without hiding the defect behind retries.**
  The complete real CLI transport suite reproduced a hang locally. Sampling
  its owned helper showed `ActiveRunsObservationTests.realCLI → $defer #2 →
  NSConcreteTask.waitUntilExit`, with no remaining child processes. The fixture
  unconditionally joined a client again after an explicit join. Cleanup now
  terminates only a still-running owned child; normal exits use bounded async
  observation. The isolated cancellation test passed before this change, so
  it did not establish the cause. A whole-step timeout bounded resource use
  but preserved no hosted stack.
- **Separate missing evidence and cold setup from operational failure.**
  Delivery exposed two more boundaries: a fresh hosted runner exhausted the
  installation proof's 30-second container deadline while downloading its
  image, and GitHub briefly reported no checks after a new head was pushed.
  The proof now downloads the image under its own network deadline before
  creating the container. All four disposable Linux installation scenarios
  then passed locally. The check reader treats GitHub's empty check response
  as missing evidence to keep watching; authentication and malformed responses
  remain errors. A parser regression retains the observed GitHub response.

After the Swift change, eight transport tests passed in 24.762 seconds and all
338 Swift tests passed in 93.129 seconds. All 57 release tests, 14 landing tests,
ten Python release automation tests, cron precedence and Flow failure proofs,
formatting and all-target Clippy passed locally. Provider fixtures are simulated;
they do not establish hosted publication. The local sample proves the cleanup
defect, not the exact blocked stack in #1309 or the earlier #1303 hang. Hosted
CI, release publication and installed cron activation are pending at this curation.

The same cron log reported an incompatible development store that prevented
journal recording. That separate boundary did not cause hosted Swift CI to
fail. #1308's branch/Home isolation landed during this investigation; its
merge alone proves neither installed recovery nor publication. Preserve the
populated Home and historical receipts rather than rewriting the failed attempt.
