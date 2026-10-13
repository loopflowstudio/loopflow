# CI timing history — 2026-10-12

Jack Heart requested an independent historical audit, without implementation or delivery changes. **Full PR CI did run under five minutes. The latest verified retained example is September 30, 2026, at 22:09 UTC: 4:36 on PR #1371.** The subsequent rise is predominantly Rust test execution, not queueing or compilation. Test count contributes, but increasingly expensive cases and fixtures matter much more than count alone.

This extends the [current baseline](ci-under-five-minutes.md) and [failure-history audit](ci-failure-history.md). It does not establish ten consecutive typical-code PR passes below five minutes, recommend removing behavioral coverage, or authorize the concurrent cache experiment.

## Evidence and boundaries

- Inventoried **6,713 retained CI workflow runs**, January 20–October 12, 2026: **4,547 pull_request, 981 merge_group, 1,185 push**. The latest creation in this snapshot is October 12 at 23:25:52 UTC. Outcomes: 4,654 success, 1,937 failure, 121 cancelled, one unfinished. Collection is not atomic; October 12 is partial.
- Started with monthly API partitions, recursively split any partition exceeding 1,000 runs, then paginated each leaf. Selected weekly first-attempt successful PR runs at the duration quartiles, plus weekly median push/merge-group runs. This is stratified reconnaissance, **not a random sample or population percentile estimate**. Dependency PRs are included when they execute the full workload.
- Expanded around July 15–24 and September 22–October 12. Reused 606 successful-run job listings from the failure audit; its complete recent PR inventory supports **318 full, first-attempt successful PR observations from September 22 onward**. Collected/reused **909 run job listings, 10,460 job records** overall. Extra short-run probes are deliberately excluded from the broad trend table to avoid biasing it downward.
- Probed **45 Rust job logs**: **34 available, 11 HTTP 410**. July 14 and earlier probes are expired; July 15 onward probes succeeded. Older job start/end timestamps remain available, but sampled older job responses have empty step lists. Thus January–June compilation/test splits and counts are unknown, not zero. This observed boundary is not a claim about a configured retention policy. Deleted runs cannot be counted, and sparse months do not prove low historical activity.
- Raw API JSON, logs, sampling/parsing scripts and derived data: `/tmp/loopflow-ci-timing-history/`. Reused originals: `/tmp/loopflow-ci-failure-history.5DxABU/` and `/tmp/loopflow-ci-baseline-lf12be237/`. These are temporary local evidence, not a published archive. No signed log URLs or credentials are included.

**Timing:** workflow wall time is run creation to the last completed successful job, normally `tests-result`; job execution is job start to completion. `updated_at` was used only for inventory sampling/screening and can differ from completion. Job creation-to-start is scheduling delay; run-to-job-start can also include dependencies. Neither measures time waiting for merge-queue admission or push-to-Actions dispatch. Reruns are excluded from trend medians; carried jobs are not summed as fresh executions. Runner occupancy, test-duration sums and wall time are different quantities.

**Full:** the historical required workload must execute, not merely have a green workflow or a successful named job. PR #1371's full job/step records and logs were checked directly. Push jobs can succeed while their test steps are skipped for exact merge-proof reuse. Historical branch-protection settings are not retained here: “required” means the versioned workflow's test/aggregate contract, not a reconstructed GitHub ruleset.

## Last verified sub-five-minute code PR

[PR #1371](https://github.com/loopflowstudio/loopflow/pull/1371), “Run Desktop checks headlessly and consolidate acceptance at gate,” changed implementation, test tooling and the workflow. It was not a documentation-only or checkpoint run.

| Event | Run | Measured wall | What executed |
|---|---|---:|---|
| PR, September 30 22:09 UTC | [36783883323](https://github.com/loopflowstudio/loopflow/actions/runs/36783883323) | **4:36** | All 12 non-skipped jobs; Rust 4:23, Xcode 3:44, installation 2:48, Swift 2:35 |
| Merge group, 22:14 UTC | [36784377506](https://github.com/loopflowstudio/loopflow/actions/runs/36784377506) | **4:30** | Full integration workload; Rust 4:15 |
| Main push, 22:19 UTC | [36784864191](https://github.com/loopflowstudio/loopflow/actions/runs/36784864191) | 5:35 | Mixed proof reuse/cache population; **not another full test observation** |

The PR Rust log records **1,960 passed, 11 skipped**, 83 seconds compilation and 148.765 seconds nextest execution. Rust started nine seconds after run creation, two seconds after job creation. “Full” retains intentional test-level skips; it does not mean every platform-specific or ignored test ran. The change also explicitly moved hosted UI diagnostics out of the default path, so its green matrix is not identical coverage to older UI-host testing.

All **273 later successful PR runs with inventory duration below five minutes** were inspected: each executed only `scratch-clear`. They are checkpoint deferrals, including code-bearing branches, not fast full proof. [PR #1361](https://github.com/loopflowstudio/loopflow/pull/1361) introduced that policy September 30. Documentation-only changes and release/main pushes must likewise not be counted as typical-code acceptance merely because they are green. The latest full baseline remains roughly 19–21 minutes; [PR #1543's candidate](https://github.com/loopflowstudio/loopflow/actions/runs/38314949621) improves individual jobs but is still over five minutes.

The #1371 PR merged at 22:19:09 UTC, 9:53 after PR-run creation. That interval includes two proofs and intervening overhead; it is **not push-to-merge**. Queue admission and the originating push timestamp were not collected. The later full merge-group baseline is 18:40–20:26 across four runs, separately from three PR baseline runs at 20:44–20:56.

## Broad history, then change-point sampling

Weekly quartile reconnaissance, restricted to runs whose full PR workload executed. `n` is sampled runs, not all PRs. Times are sample median wall times; the changing workload prevents treating these as an unchanged benchmark.

| Month | n | Median | Observed range | Main interpretation |
|---|---:|---:|---:|---|
| January | 6 | 2:30 | 1:56–2:57 | Earlier Python/Swift workload; no comparable Rust count |
| February | 12 | 2:52 | 2:23–3:37 | Rust job median 2:35 |
| March | 6 | 2:52 | 2:34–3:28 | Rust 2:13, Swift 2:22 |
| April | 6 | 5:21 | 3:08–15:07 | Substantial macOS scheduling delay in several runs |
| May | 14 | 5:09 | 3:03–8:03 | Runner waits commonly exceed execution growth |
| June | 14 | 5:09 | 3:07–6:27 | Rust median 3:59; mixed scheduling and build/test cost |
| July | 10 | 3:04 | 1:48–5:41 | Early-month reduction, sharp mid-month Rust growth |
| August | 10 | 6:02 | 5:12–6:39 | Rust median 5:56 becomes the sustained bottleneck |
| September | 9 | 8:11 | 5:00–10:23 | Growth followed by late-month optimization; sparse broad sample |
| October, partial | 5 | 9:01 | 6:48–19:20 | Checkpoint skips bias broad selection; use expanded daily cohort below |

### April–June: distinguish runner waits from slow tests

[April 24 run 24865454084](https://github.com/loopflowstudio/loopflow/actions/runs/24865454084) took **15:07**, but its longest job executed for **3:14**. Swift waited **11:48** between job creation and start; Xcode waited **7:46**. In [another April 24 run](https://github.com/loopflowstudio/loopflow/actions/runs/24865239492), a 6:01 workflow contained a 2:29 Swift job starting 3:28 after run creation. These are scheduling delays, not evidence that tests took 15 minutes.

There were execution outliers too: [April 27](https://github.com/loopflowstudio/loopflow/actions/runs/25011302472) had a 7:16 Xcode job with negligible initial scheduling delay. Expired logs prevent distinguishing compilation from a hosted-test hang in that run. Source history records earlier UI-host termination changes, but does not establish that outlier's cause.

### July 15–24: compilation and test execution both rise

One median-duration successful PR per available day was added around the change point:

| Date/run | Full wall | Rust build | Test execution | Passed cases |
|---|---:|---:|---:|---:|
| [July 15](https://github.com/loopflowstudio/loopflow/actions/runs/29456104910) | 1:58 | 1:04 | 22.134s | 1,373 |
| [July 16](https://github.com/loopflowstudio/loopflow/actions/runs/29502760119) | 3:04 | 1:19 | 48.215s | 1,588 |
| [July 17](https://github.com/loopflowstudio/loopflow/actions/runs/29598441446) | 4:26 | 2:05 | 112.145s | 1,884, including five leaky passes |
| [July 21](https://github.com/loopflowstudio/loopflow/actions/runs/29806192366) | 5:09 | 2:12 | 144.447s | 1,733 |
| [July 24](https://github.com/loopflowstudio/loopflow/actions/runs/30101044355) | 6:13 | 2:36 | 180.998s | 1,859 |

July 15→24: count grows **35%**, execution **8.2×**, build **2.4×**. The added command/environment matrix in [#1005, July 16](https://github.com/loopflowstudio/loopflow/pull/1005) is a concrete contributor: it takes **41.221s** in the July 17 sample, longer than the entire July 15 test execution. It later falls to 15.450s on July 24 while total execution keeps rising, so it cannot explain the entire increase.

Many authority, process, store and migration changes landed in this interval. These are different source candidates and runner/cache states, not an isolated commit bisect. The timing evidence narrows the sustained change to July 15–24; it does not justify blaming a single migration or PR. [#1037](https://github.com/loopflowstudio/loopflow/pull/1037) removed Xcode caching July 17, but Rust dominated these sampled runs, so that cannot explain their critical-path growth.

The earlier July speedup has a credible source mechanism: [#782](https://github.com/loopflowstudio/loopflow/pull/782) split lint/test compilation and added caches, [#787](https://github.com/loopflowstudio/loopflow/pull/787) adopted nextest, and [#795](https://github.com/loopflowstudio/loopflow/pull/795) replaced the intermittent hosted UI execution with build-for-testing. Expired early-July logs prevent assigning measured savings to each change.

### September 22–October 12: growth, successful reduction, then larger growth

Daily full successful first-attempt PR medians from the expanded recent cohort:

| UTC date | n | Full wall | Rust job | Installation job |
|---|---:|---:|---:|---:|
| September 22 | 5 | 6:30 | 6:22 | — |
| September 25 | 17 | 9:47 | 9:34 | — |
| September 27 | 10 | 10:50 | 10:39 | — |
| September 28 | 11 | 10:28 | 10:20 | 3:36 |
| September 29 | 39 | 7:24 | 6:17 | 2:58 |
| September 30 | 22 | 5:20 | 4:42 | 2:47 |
| October 1 | 27 | 7:08 | 6:38 | 4:03 |
| October 4 | 27 | 7:38 | 7:16 | 3:59 |
| October 6 | 28 | 9:08 | 8:48 | 4:10 |
| October 8 | 22 | 11:02 | 10:42 | 4:00 |
| October 9 | 10 | 12:56 | 12:34 | 5:16 |
| October 10 | 4 | 15:36 | 15:22 | 5:32 |
| October 11 | 12 | 15:42 | 15:22 | 5:30 |
| October 12 | 11 | 19:20 | 18:44 | 10:34 |

Installation medians use only runs where that job existed. Individual PRs can contain proposed changes before their merge date. The unsummarized intervening dates are retained in raw analysis; these rows are not a claim of consecutive acceptance runs.

September 23 added a separate CLI build/checkout-refresh proof in [#1273](https://github.com/loopflowstudio/loopflow/pull/1273). Four sampled September 23–28 logs show an extra dev compilation of approximately **47–61 seconds** after nextest. September 29's [#1321](https://github.com/loopflowstudio/loopflow/pull/1321) reused the integration-built CLI. [#1328](https://github.com/loopflowstudio/loopflow/pull/1328) optimized SQLite; [#1332](https://github.com/loopflowstudio/loopflow/pull/1332) corrected dependency-cache profile keys. Those changes align with a large reduction, not a regression: September 28→29 sampled nextest execution drops **388→212s with 2,132→2,148 cases**. Further September 30 reduction coincides with removed/reshaped execution infrastructure and **1,927** sampled cases. Several changes landed together; no exact saving is assigned to one of them.

## What grew inside Rust

Daily representative logs use the same nextest summary/build boundaries. Case sums include waiting and subprocess work, not CPU time.

| Date/run | Cases passed | Build | Execution wall | Sum of case seconds |
|---|---:|---:|---:|---:|
| [August 28](https://github.com/loopflowstudio/loopflow/actions/runs/33217139336) | 1,866 | 92s | 247s | 970 |
| [September 22](https://github.com/loopflowstudio/loopflow/actions/runs/35789257534) | 1,901 | 102s | 253s | 962 |
| [September 27](https://github.com/loopflowstudio/loopflow/actions/runs/36293938425) | 2,040 | 98s | 432s | 1,705 |
| [September 30](https://github.com/loopflowstudio/loopflow/actions/runs/36766607164) | 1,927 | 91s | 146s | 574 |
| [October 1](https://github.com/loopflowstudio/loopflow/actions/runs/36938099994) | 2,061 | 99s | 272s | 1,076 |
| [October 8](https://github.com/loopflowstudio/loopflow/actions/runs/37714083410) | 2,284 | 111s | 481s | 1,883 |
| [October 9](https://github.com/loopflowstudio/loopflow/actions/runs/37943094338) | 2,308 | 123s | 597s | 2,324 |
| [October 11](https://github.com/loopflowstudio/loopflow/actions/runs/38160526321) | 2,348 | 130s | 756s | 2,934 |
| [October 12, before #1540](https://github.com/loopflowstudio/loopflow/actions/runs/38208754634) | 2,527 | 134s | 960s | 3,814 |
| [October 12 baseline](https://github.com/loopflowstudio/loopflow/actions/runs/38303294677) | 2,548 | 136s | 1,044s | 4,148 |

September 30→October 12's daily samples: **31% more cases, 6.6× execution time, 5.1× average case duration**. Build growth is only 43 seconds of an approximately 857-second build-plus-execution increase. At September 30's average case cost, 2,527 cases would sum to roughly **753 test-seconds**, versus the observed **3,814**. This is a descriptive counterfactual, not a forecast: tests changed, and new tests can be more expensive.

Matched names also slow down. Between September 30 and October 8, **1,448 unchanged full test names** sum to **380→579 seconds**, with median positive-duration ratio **1.15×**. Between October 8 and 9, **2,108 shared names** sum to **1,742→1,851 seconds**. Matching drops the binary prefix but retains the module path; renamed/moved tests are unmatched, not automatically “new coverage.” No source-controlled A/B benchmark separates runner variance, fixture changes and production-path growth.

Across these logs, summed case time divided by execution wall is approximately **3.8–4.0**, consistent with four occupied test slots. This is neither CPU utilization nor evidence of a fully serial suite. Existing nextest exclusivity for the fleet-receipt test remains visible in source, but that case took only 4.404s in the October 8 sample; it cannot explain many minutes of growth. There is no sampled Rust matrix/shard expansion or runner-label transition explaining the October increase. `ubuntu-latest` is not a fixed hardware/image guarantee, so unmeasured host variation remains possible.

## Strongest causal leads and useful reductions

1. **Repeated matrix fixture setup has a specific source change.** [#1503](https://github.com/loopflowstudio/loopflow/commit/d20c56dafba9fd90407bf5be4075da65d38b4b8f) moved repository/Home/store seeding inside every command × environment cell because Task creation now mutates planning state. The matrix goes **6.185s on October 8 → 43.492s in that PR's October 9 run → 53.781s October 11 → 118.402s October 12**. The initial jump has a strong mechanism; the later doubling is not isolated to that patch. Profile seeding, migration application and command startup separately. Preserve independent mutable cells and resolution assertions; investigate safely cloned prepared fixtures or smaller behavioral representatives rather than sharing contaminated state.
2. **October 12 adds real wall-clock observation floors.** [#1540](https://github.com/loopflowstudio/loopflow/pull/1540) replaces idle polling with boundary-driven acquisition and watcher backoff. Its native quietness test explicitly waits **60 seconds** in the Codex case; the watcher proof now accommodates the real backoff. The latest baseline's watcher is **156.229s**, native quietness **134.280s**, matrix **121.612s**. Earlier October 11 watcher timing was **37.692s**. Retain idle-no-request, reconnect, turn/step-boundary and lost-reply assertions. Independent scheduling can keep these from starting at the tail; a deterministic test boundary is only useful if it preserves the actual regression contract. Shortening production observation windows is not justified by this audit.
3. **Broad fixture/production-path cost matters beyond the slowest tests.** October 12's baseline top 100 cases consume **44.8%** of case seconds; the seven longest consume only **16.5%**. [#1491](https://github.com/loopflowstudio/loopflow/pull/1491), merged October 12, expands shared planning and moves many tests; [#1499](https://github.com/loopflowstudio/loopflow/pull/1499) adds lifecycle proofs, including a case measured at **100.608s October 10**. These are source-backed investigation boundaries, not proof that all added work is waste. Measure fresh stores, repeated migrations, CLI launches and fixture teardown on fixed revisions before choosing reductions.
4. **Reuse compilation while preserving installation authority.** Required disposable installation arrived in [#1308](https://github.com/loopflowstudio/loopflow/pull/1308), September 28. Published-title coverage arrived in [#1508](https://github.com/loopflowstudio/loopflow/pull/1508), October 8. The latest baseline records multiple nested compilations; the existing #1543 experiment eliminates repeated Cargo rebuilds and brings installation **10:39→5:28**, while retaining a distinct published build. These hosted totals also include runner/cache variation, so the entire difference is not an isolated treatment effect. Its remaining cold managed/published compilation is **187s + 69s**. This is a much stronger reduction lead than deleting installation assertions. Warm dependency reuse must be measured separately from cold compilation; this audit did not touch the running experiment.
5. **Remove serial macOS build work from the Swift critical path where possible.** [#1509](https://github.com/loopflowstudio/loopflow/pull/1509), October 9, added macOS process-discovery tests after CLI compilation. The October 12 daily sample spends **147s** in that step, plus **74s** building the CLI and **74s** in Desktop build/tests. Preserve the macOS-specific deleted-executable/process-authority regression. Compile compatible targets together, or schedule independent proof separately. Faster Rust Linux execution alone will not make every required job sub-five.
6. **Build once and partition execution, with an honest floor.** The latest sum of **4,148 test-seconds** is substantial parallelizable work; runner waits are now small on the Rust critical path. But 136s compilation plus a 156s longest case leaves almost no five-minute budget for setup, transfers or imbalance if both stay serial on that path. Reuse the existing archive experiment; schedule long proofs separately and measure total runner occupancy. No paid runner purchase or optimal shard count follows from this audit.

The failure audit already identifies duplicated architecture checking and brittle editorial assertions. Those are useful cleanup targets, but their sub-minute jobs do not explain today's critical path. Keep behavioral coverage that has caught migration, process-ownership and planning-recovery defects. The priority is removing repeated preparation and arranging independent work, with test deletions justified by coverage value rather than duration alone.

## Review limits

The strongest finding is measured: the recent increase is test execution and heavier case cost. Exact causal allocation among new coverage, fixture setup and production changes remains open. Historical samples mix cache states and PR branches; successful-only timing also omits repair/cancellation costs. No tests, builds, Flows, commits, publication, merging, runner changes or implementation edits were performed. The existing cache-experiment files were left untouched.
