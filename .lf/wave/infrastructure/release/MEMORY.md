# Release memory

## v0.13.6 publication and installation (2026-10-06)

The installed release controller completed [v0.13.6](https://github.com/loopflowstudio/loopflow/releases/tag/v0.13.6)
at 18:38:27 UTC from `e177eafe6ee6f45b2221da845d12dd8dd640a2bb`.
PR #1464 and candidate workflow 37509813134 passed. It includes #1452's reactive
Desktop, #1456's listing repair and #1462's retired telemetry-producer repair.
This was manual recovery after the scheduled attempt failed; it supplies no
credit toward LOO-285's two original unattended settlements.

Jack Heart reported `lf install` appearing hung. The supported installation
eventually completed its 2.2 GiB store backup, migration and artifact promotion.
CLI and Desktop report 0.13.6; a repeat install reported already installed.
Samples identified repeated unpinned preflight snapshots and a migration backup
sleeping between 64-page batches. Local prevention commit `3a9f55c8e` pins the
snapshot, removes the migration backup's deliberate delay and announces that
phase. Focused snapshot/integrity/exclusion tests and Clippy passed; the prevention
fix is not yet published. No raw state edit or process termination was used.

## Published recovery runtime (2026-10-06)

The normal installed `lf release run patch` completed v0.13.5 at
2026-10-06 13:28:48 UTC, from `4da5b8220a417677d0b6e4e3f9c3ec17fe850d65`.
[Release PR #1461](https://github.com/loopflowstudio/loopflow/pull/1461) and
[candidate 37468833549](https://github.com/loopflowstudio/loopflow/actions/runs/37468833549)
passed before signed preparation and public publication. `lf install` exited
successfully and `lf --version` confirmed 0.13.5; promotion recognized the
existing database exactly and applied no migration. An already-running Desktop
was not restarted as part of this installation.

This release includes #1457's contained publisher verification, #1455's explicit
historical-Exec acceptance, #1458's boot witness and #1459's PR-base recovery.
Docker's disposable ARM64 Ubuntu container ran successfully on this host before
the release. Publication succeeded through the configured publisher, without a
branch binary touching the installed Home. These manual results do not satisfy
LOO-285's two unattended scheduled settlements.

## Installed schedule repair (2026-10-05)

Infrastructure's installed release and telemetry jobs still referenced an old
immutable executable after v0.13.3 installation. Both were reinstalled through
`lf wave cron add` with their original 10:00 and 09:00 local schedules unchanged.
Readback confirms loaded jobs now use the machine installation gate, so future
published CLI promotions do not strand them on that old executable.

The installed owner remains `infrastructure`: `infrastructure/release` is not
registered, and no child-owner migration was attempted. Release history after
refresh exposes three unresolved historical opportunities with unknown timezone
provenance, zero executions and no qualifying pair. Do not backfill success or
count this repair as unattended settlement. LOO-285 remains open for two adjacent
original scheduled opportunities, distinct automatic executions, required proof
and at least one publication without manual repair.

## Release completion responsibility (2026-10-05)

Jack Heart directed Infrastructure on October 4 to ensure releases finish through
verified publication and installed acceptance. Release retains its execution and
evidence; Infrastructure follows interruptions through recovery and reports actual
stages in the ongoing conversation. A merged release PR is unfinished until the
public artifacts and configured completion checks pass.

The v0.13.1 version PR #1426 merged without publication. Supported recovery cut
v0.13.2, ultimately published at 2026-10-05 07:31:48 UTC from
`f4cbe142b9a320ac6c2488273345c28153b45e91` after
[candidate 37276274755](https://github.com/loopflowstudio/loopflow/actions/runs/37276274755)
passed. Published CLI and `/Applications/Loopflow.app` both installed as 0.13.2.
The default Session inventory contained 43 interactive Sessions, zero headless
and zero closed; native CLI discovery also passed. This delivers PR #1421's
filtering. It does not prove LOO-353's unfinished Waiting-first experience.

Recovery exposed three release boundaries. Installed 0.13.0 predates the
publisher's inherited-lock handoff, so recovery used a current source CLI with a
disposable Home, preserving the live Home. The publisher launcher retained an
isolated test lock. The five-minute deadline for the entire 76 MB artifact set
repeatedly killed healthy transfers near 200 KiB/s; fifteen minutes remains
bounded and completed the transfer. Finally, publisher preparation and public
verification still required XCUITest despite Jack's September 30 retirement of
that prerequisite (LOO-357, `release/UI_HOST_GATE.md`). macOS authentication
prevented UI runner initialization after Desktop had already built and signed.
[PR #1438](https://github.com/loopflowstudio/loopflow/pull/1438) landed the download
and publisher fixes while preserving required headless CI and artifact proof.

A follow-up found the same retired UI receipt in Rust's scheduled-settlement
validator, and public smoke still invoked the removed `lf catalog` command.
Remove the obsolete receipt requirement and use `lf list --json`; keep all actual
public verification stages required. The scheduled regression exercises published,
no-change, telemetry failure/recovery, missing proof and smoke failure without a
UI receipt. Publisher fixtures now reject unknown CLI commands instead of returning
a version for every invocation. The follow-up landed in PR #1441 and shipped in
v0.13.3, published at 2026-10-05 08:40:22 UTC from
`8ea0bec9cf4b0c08ca17c52e57de059000a7b0e3` after
[candidate 37283026406](https://github.com/loopflowstudio/loopflow/actions/runs/37283026406)
passed. The normal installed CLI completed that release without the temporary
recovery CLI. CLI and Desktop then installed as 0.13.3 without another migration.

A Docker startup failure recovered through one failed-job rerun on a fresh GitHub
runner. This manual recovery does not satisfy LOO-285's two unattended scheduled
settlements. Direct public read-back for 0.13.3 matched all seven GitHub asset
digests and both versioned/latest DMG downloads against the prepared checksums;
the website and crate registry reported 0.13.3. Installed CLI discovery and the
default Session inventory passed (43 interactive, zero headless, zero closed).
These observations are manual acceptance, not a scheduled verified receipt or
proof of two unattended settlements. Reopening Desktop remains necessary to
load the new UI in an already-running application.
Publisher checkout cleanup reported a retained lease at
`/Users/jack/src/loopflow.publish-default-v0-13-2`; preserve it for supported
reconciliation rather than removing it manually.

## Retained landing incident and minor recovery (2026-10-04)

Jack Heart authorized autonomous recovery and delivery under Infrastructure;
[LOO-373](https://linear.app/loopflow/issue/LOO-373) owns the retained Home landing
and deleted-tmux-cwd repairs. The original retry
`ff94aac9-b59d-409b-9bce-45dc921f1f1a` has no terminal receipt. Its retained
process receipt named absent PID 17052, and the exact minor/preparation leases
were unheld before supported re-entry. Process-list absence alone was not used
as publication authority.

`lf release run minor` completed as Exec `a7dee370-c624-4ac5-b063-a981714d7e7d`
with exit 0. [v0.13.0](https://github.com/loopflowstudio/loopflow/releases/tag/v0.13.0)
published at 2026-10-04 07:20:30 UTC from `12016c6d6dd34a4c1a553c25b5cf2529ee93615b`
and [candidate 37182312803](https://github.com/loopflowstudio/loopflow/actions/runs/37182312803).
The saved pair remains v0.12.32/v0.13.0, with its original patch commit and
prepared tree, now `completed: true`. No competing publisher, manual state
rewrite, or draft-bearing binary promotion was used. This release predates the
landing migration and startup prevention; installed recovery still requires
their published patch.

Prevention retires Home landing claims with their identity/PID/heartbeat/generation
retained on the landing, rejects old executable reacquisition, and preserves
delivery intent and failures. A released-frontier regression passes through
domain reads, stale-write rejection, real reconciliation with controlled provider
facts, and repeat reconciliation under both draft and canonicalized schemas.
An isolated tmux 3.7c server with deleted cwd launched a child into a quoted path
after explicit child `cd`; this proves the known defect, not the causes of every
LOO-371/372 timeout. Their original exited-pane stderr was unavailable. The active
LOO-371/372/326 Flows were preserved.

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

## Preserve the pending release version (2026-10-05)

Jack Heart requested autonomous 5whys and prevention after the v0.13.1 gap.
Incorrect earlier preparation commits may remain in history; a corrected commit
must retain and publish the same pending version. This supersedes recovery's
previous automatic patch-successor policy. The sourced causal analysis and
prevention live in `release/v0.13.1/RECOVERY.md`. Existing published artifacts,
tags and migration bytes remain immutable. Prevention PR #1451 shipped in
v0.13.4 and remains installed in v0.13.5. The historical v0.13.1 publication gap
was not backfilled; activation of prevention does not erase that gap.
