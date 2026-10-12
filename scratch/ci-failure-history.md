# Loopflow PR CI failure history

Jack Heart requested a read-only investigation of CI failures from 2026-09-12 through 2026-10-12, with test value judged independently of runtime. This report extends, and does not modify, `scratch/testing-audit.md`. No builds, tests, provider calls, Git mutations, or implementation changes were made for this investigation.

**The strongest reduction opportunity is duplicated and stale proof, not deletion of frequently failing behavioral tests.** The architecture checker failed in two jobs on exactly the same 31 candidates. Several leading Rust failures occurred before their behavioral assertions. Installation acquisition failed repeatedly without running tests. Conversely, a migration failure exposed a real transaction bug, and a successful installation job silently selected zero tests for one proof.

## Inventory and completeness

Repository: `loopflowstudio/loopflow`; workflow [`ci.yml`](https://github.com/loopflowstudio/loopflow/actions/workflows/ci.yml), workflow ID `225107949`; event strictly `pull_request`. Push, merge-group, release, Kata, and billing activity are excluded.

The frozen cohort is runs **created from 2026-09-12T00:00:00Z through 2026-10-12T21:48:26Z**. October 12 is partial, not a complete day. This uses UTC calendar dates, not Los Angeles midnight boundaries. Attempts of these runs are included; retries of runs originally created before September 12 were not searched separately.

| Inventory | Result |
|---|---:|
| Distinct workflow run IDs / head SHAs | 969 / 969 |
| Workflow attempts, including earlier attempts | 986 |
| Run IDs with explicit reruns | 9; 17 extra attempts |
| Independent PRs | 279 |
| Latest run outcomes at inventory snapshot | 606 success, 280 failure, 82 cancelled, 1 in progress |
| Attempt outcomes, retaining earlier attempts | 606 success, 295 failure, 84 cancelled, 1 in progress |
| Attempt-specific job records | 12,371 |
| Carried successful results deduplicated | 137 |
| Distinct job observations after deduplication | 12,234: 7,034 success, 1,006 failure, 303 cancelled, 3,891 skipped |
| PRs with any failed job / any identifiable failed test | 116 / 82 |

Daily run inventory:

| September date | Runs | October date | Runs |
|---|---:|---|---:|
| 12 | 0 | 01 | 61 |
| 13 | 0 | 02 | 30 |
| 14 | 1 | 03 | 11 |
| 15 | 0 | 04 | 44 |
| 16 | 0 | 05 | 36 |
| 17 | 0 | 06 | 76 |
| 18 | 1 | 07 | 58 |
| 19 | 0 | 08 | 98 |
| 20 | 1 | 09 | 64 |
| 21 | 0 | 10 | 33 |
| 22 | 11 | 11 | 72 |
| 23 | 26 | 12, partial | 32 |
| 24 | 26 | | |
| 25 | 34 | | |
| 26 | 20 | | |
| 27 | 36 | | |
| 28 | 42 | | |
| 29 | 100 | | |
| 30 | 56 | | |

Read-only REST collection used `/actions/workflows/225107949/runs?event=pull_request&created=<daily-range>&per_page=100&page=N`: **32 pages over 31 daily partitions**, each reconciled against its API `total_count`. September 29 required a second, empty page. Daily partitioning avoids the filtered-runs API's 1,000-result ceiling. All 986 attempt-specific `/actions/runs/{id}/attempts/{n}/jobs` listings were retrieved, with pagination until exhausted and no unresolved listing errors. Earlier attempt metadata was fetched for all nine rerun run IDs.

PR association: 13 runs have direct `pull_requests` associations. The remaining 956 uniquely match PR head repository, head branch, and the PR's open interval. Four pages of PR metadata, 400 rows ordered by update date, reached July 22. All 969 runs have exactly one association; none ambiguous or unmapped. Thus 279 is a metadata-correlated PR denominator, not a claim that every Actions row supplied a direct PR link.

**Do not sum attempt job lists as executions.** GitHub assigned different job IDs to carried successes but preserved their start/end timestamps. Deduplication uses `(run_id, job name, started_at, completed_at, conclusion)`, keeping the earliest attempt. For example, Rust in [run 37992574422](https://github.com/loopflowstudio/loopflow/actions/runs/37992574422) finished at 21:29:01 in attempt 1; attempts 2–8 repeat that same interval. All 137 duplicates were successes. Failure rankings are unaffected; success denominators would otherwise be inflated.

Raw API responses, logs, and analysis scripts remain outside repository Markdown context at `/tmp/loopflow-ci-failure-history.5DxABU/` (`inventory.json`, `pr-mapping.json`, `unique-jobs.json`, `log-inventory.json`, `analysis.json`). The directory is local and temporary, not a durable published archive. Only aggregates, public repository references, and diagnostic interpretations appear here.

### Logs and censoring

- Retrieved **all 1,006 failed-job logs** and **258/303 cancelled-job logs**. The remaining 45 cancelled jobs return HTTP 404. No HTTP 410 was observed; 404 does not establish whether a log expired, was deleted, or never existed. Two transient retrieval failures recovered on retry. A local `gh` terminal-escape refusal was also resolved; it is not missing evidence.
- Parsed Rust nextest/libtest, pytest, Swift Testing and XCTest failure formats. Deduplicated repeated summaries within each job. **332 failed jobs contain identifiable test failures, with 338 distinct historical test names.** Names are not merged across renames or rewritten suites. Four successful installation logs were additionally read to verify what reruns exercised.
- **674 failed jobs lack a parsed test name.** Most are policy, aggregate, build, or acquisition failures, itemized below. This is not 674 mysterious failed tests.
- Twelve cancelled jobs contain identifiable test failures before cancellation. They are retained in local data but excluded from the definitive failed-job ranking. Cancellation and older nextest fail-fast behavior censor exposure; absence from the ranking is not proof a test was stable.
- Collection was not atomic: the one running workflow completed jobs while listings were gathered. The workflow snapshot above remains frozen rather than silently replacing its outcome. Deleted runs and logs cannot be recovered through this inventory; sparse early dates describe available data, not proven low activity.

## Failing jobs, with denominators

Denominator is **success + failure after carried-result deduplication**. Cancelled/skipped observations are shown separately and are not passes. These are job failure rates over this changing CI configuration, not individual-test failure probabilities. PRs count distinct PRs with at least one failure; one PR may appear in several rows.

| Job | Failed / completed | Rate | Failed PRs / PRs with completed job | Cancelled | Skipped |
|---|---:|---:|---:|---:|---:|
| rust-test | 221 / 609 | 36.3% | 74 / 276 | 75 | 287 |
| swift-test | 54 / 616 | 8.8% | 29 / 274 | 68 | 287 |
| task-installation | 44 / 483 | 9.1% | 24 / 233 | 45 | 287 |
| python-test | 42 / 677 | 6.2% | 20 / 276 | 6 | 287 |
| architecture-check | 31 / 680 | 4.6% | 16 / 276 | 1 | 287 |
| website-test | 29 / 666 | 4.4% | 15 / 276 | 17 | 287 |
| rust-lint | 24 / 667 | 3.6% | 17 / 276 | 16 | 287 |
| loopflow-ui-test | 21 / 640 | 3.3% | 9 / 276 | 41 | 287 |
| e2e-smoke | 8 / 661 | 1.2% | 4 / 276 | 22 | 287 |
| migration-check | 2 / 680 | 0.3% | 1 / 276 | 3 | 287 |
| scratch-clear, policy | 156 / 962 | 16.2% | 36 / 279 | 6 | 0 |
| tests-result, aggregate | 374 / 699 | 53.5% | 116 / 276 | 3 | 281 |

`merge-proof` was skipped in all 740 observations where that job existed. `tests-result` reports upstream failure/cancellation; it is not independent test evidence. Together, aggregate and scratch-policy jobs account for **530/1,006 failed-job observations**. The 156 scratch failures are historical: [PR #1361](https://github.com/loopflowstudio/loopflow/pull/1361) changed PR scratch to successful checkpoint deferral on September 30. Current `.github/workflows/ci.yml:62–73` still enforces cleanup for landing events. Do not optimize a retired failure policy as though it remains today's test bottleneck.

Failed jobs can occur inside ultimately cancelled runs, and earlier failed attempts can precede success. Therefore failed-job/run-attempt counts must not be compared directly with the 280 latest failed workflow runs.

## Most frequently identifiable tests

Counts are failed **job executions / distinct run IDs / distinct PRs**. These top entries each failed once per run; repeat counts mostly represent changed candidates, not same-SHA reruns. No per-test pass denominator was collected: successful suite jobs do not establish that a particular test existed, was selected, or ran.

| Historical test name | Jobs / runs / PRs | Representative failed job |
|---|---:|---|
| `python/tests/test_architecture.py::test_current_repository_has_no_unexplained_architecture` | 31 / 31 / 16 | [114647812464](https://github.com/loopflowstudio/loopflow/actions/runs/38198945175/job/114647812464) |
| `website/tests/test_portable_architecture.py::test_portable_architecture_matches_the_internal_surface[chromium]` | 24 / 24 / 11 | [106989822259](https://github.com/loopflowstudio/loopflow/actions/runs/35800600824/job/106989822259) |
| `controller::task::planning_tests::claimed_autonomous_boundary_settles_once_at_the_human_node` | 15 / 15 / 3 | [106989822400](https://github.com/loopflowstudio/loopflow/actions/runs/35800600824/job/106989822400) |
| `ops::task::tests::parked_human_boundary_reports_blockers_without_provider_preflight` | 11 / 11 / 4 | [108606610802](https://github.com/loopflowstudio/loopflow/actions/runs/36314530655/job/108606610802) |
| `planning_reconnect_tests::public_flow_reconnects_planning_without_another_turn` | 8 / 8 / 6 | [114053576423](https://github.com/loopflowstudio/loopflow/actions/runs/37999406667/job/114053576423) |
| `session_cutover_tests::a_taskless_step_records_its_decision_on_the_invocation` | 8 / 8 / 1 | [109310164216](https://github.com/loopflowstudio/loopflow/actions/runs/36539158341/job/109310164216) |
| Swift: `block clicks copy command and output, then the live prompt clears selection` | 7 / 7 / 7 | [107495316481](https://github.com/loopflowstudio/loopflow/actions/runs/35956325508/job/107495316481) |
| `golden_prompt::golden_prompts_match_python` | 7 / 7 / 7 | [110493528558](https://github.com/loopflowstudio/loopflow/actions/runs/36899101786/job/110493528558) |
| `website/tests/e2e/test_docs.py::test_docs_security_page_states_forwarded_authority_guarantees[chromium]` | 7 / 7 / 5 | [109143416766](https://github.com/loopflowstudio/loopflow/actions/runs/36486152047/job/109143416766) |
| `wave_resolution_matrix::registry_is_complete` | 6 / 6 / 5 | [110493528558](https://github.com/loopflowstudio/loopflow/actions/runs/36899101786/job/110493528558) |
| `planning_reconnect_tests::work_watch_reconnects_repository_planning` | 6 / 6 / 4 | [114053576423](https://github.com/loopflowstudio/loopflow/actions/runs/37999406667/job/114053576423) |
| `python/tests/test_release_automation.py::test_release_installer_uses_the_promotion_boundary_to_activate_the_binary` | 6 / 6 / 4 | [109143417262](https://github.com/loopflowstudio/loopflow/actions/runs/36486152047/job/109143417262) |
| `terminal_control_tests::terminal_control_accepts_a_model_option` | 6 / 6 / 4 | [108735996524](https://github.com/loopflowstudio/loopflow/actions/runs/36360294044/job/108735996524) |
| `pr_tests::task_resume_revokes_auto_merge_before_returning_to_human_review` | 6 / 6 / 3 | [108508271443](https://github.com/loopflowstudio/loopflow/actions/runs/36279415171/job/108508271443) |
| `python/tests/test_loopflow_skill_alignment.py::test_agent_surfaces_share_the_inspection_commands` | 6 / 6 / 3 | [109736850998](https://github.com/loopflowstudio/loopflow/actions/runs/36668044778/job/109736850998) |
| `controller::task::planning_tests::loop_requires_a_decision_and_discards_it_on_interrupt` | 6 / 6 / 2 | [108673840503](https://github.com/loopflowstudio/loopflow/actions/runs/36338499434/job/108673840503) |

Historical names are intentional. For example, the golden test is now `golden_prompts_match`, the Flow reconnect test is now `public_flow_reconnects_planning_at_turn_and_step_boundaries`, and the old review controller was removed. Treating those as unchanged tests would hide changed contracts.

## Classification from assertions and fixes

### 1. Architecture-map drift is duplicated proof, with mixed behavioral value

**High confidence.** Every one of the 31 Python repository-map failures coincides with an `architecture-check` failure on the same run. `python/tests/test_architecture.py:121–124` calls `architecture.check_repository(ROOT)` and asserts `report.ok`; `.github/workflows/ci.yml:34–42` runs the same checker independently. These are 31 shared incidents, not 62 independent regressions.

Representative diagnostics identify an unmapped `/bin/sh` edge ([job 107820804813](https://github.com/loopflowstudio/loopflow/actions/runs/36055357417/job/107820804813)), an unmapped `/bin/cat` edge ([job 108303194278](https://github.com/loopflowstudio/loopflow/actions/runs/36206212875/job/108303194278)), and missing SQLite owner/mirror entries for `task_flow_positions` and `wave_chapters` ([job 108489638014](https://github.com/loopflowstudio/loopflow/actions/runs/36272735244/job/108489638014)). These prove map/source disagreement, not failed Task execution. Ownership omissions may be valuable architecture review; an exact executable spelling can be maintenance-only. The appropriate disposition depends on which contract the map is intended to enforce.

**Simplify:** retain one real-repository map check, remove its duplicate invocation from the other suite, and keep the checker's small fixture tests (`python/tests/test_architecture.py:127` onward). No new coverage is needed to remove the duplicate. Do not delete schema-owner checking just because wording/path checks were noisy.

### 2. Portable architecture and prose assertions impose avoidable editorial maintenance

**High confidence on observed causes; medium on broader deletion scope.** Of 24 portable-architecture failures, **23 report stale generated `docs/architecture.html`**. One is an exact `AgentSession history` assertion after the terminology changed. [Commit 9a2b5168f1](https://github.com/loopflowstudio/loopflow/commit/9a2b5168f150d036ca8626483621e29d9e1e4f6e) fixes that assertion to `LfSession history`; it does not fix the renderer. Current `website/tests/test_portable_architecture.py:6–17` combines generator freshness with title, terminology, link and stylesheet strings. Despite its historical `[chromium]` ID, the body launches a Python renderer and reads a file; it does not exercise a browser.

[Commit 2aab852716](https://github.com/loopflowstudio/loopflow/commit/2aab8527162ed7dc8042b35b221f3984397a4525) also removes obsolete daemon flags from the installer source-string test and replaces retired security-page wording. Current `python/tests/test_release_automation.py:263–268` still verifies shell text, not successful promotion. Thus the 7 security-page and 6 installer failures must not be labeled security or installation regressions from their names alone.

**Delete/simplify candidates:** the exact Session-history phrase and retired terminology checks; narrow large prose goldens to content selection, ordering, escaping, and omission rules. Keep generated-document freshness if the committed artifact is part of delivery, preferably once without browser setup. Preserve useful portable-link/style assertions with a tiny rendered fixture. Preserve promotion safety through the existing disposable installation path before removing its source-string surrogate. The seven golden failures only establish whole-output mismatch (`rust/loopflow/tests/golden_prompt.rs:102–125`); this sample does not prove all seven were harmless prose changes.

### 3. Leading Task-controller failures combine fixture errors with a real migration defect

**High confidence on 12 fixture failures; mixed/unknown for the full historical category.** Twelve of the 15 `claimed_autonomous_boundary...` failures are on PR #1270 and cannot resolve the candidate `lf` binary. They fail before assertions that the boundary is parked, its claim cleared, and its version advanced: historical `rust/loopflow/src/controller/task/mod.rs:2035–2054` at `bce5589027`. [Commit 4cff4b20d0](https://github.com/loopflowstudio/loopflow/commit/4cff4b20d099e0ce127c159e2e4f72e94e5e34a4) adds the fixture's `TestLfBinGuard::pin()`. This is deterministic fixture setup failure, not twelve demonstrations of a broken claim-settlement algorithm.

The other three report nested SQLite transactions, an unavailable temporary database, and Project-authority mismatch. **The transaction failure is valuable product evidence:** [job 109130805169](https://github.com/loopflowstudio/loopflow/actions/runs/36482349277/job/109130805169) fails during initialization; [2aab852716](https://github.com/loopflowstudio/loopflow/commit/2aab8527162ed7dc8042b35b221f3984397a4525) removes the second transaction owner and adds a focused proof of canonical initialization/upgrade, backup frontier, retained rows, and idempotence. The remaining two are not classified as harmless without more diagnosis.

The 11 `parked_human_boundary...` failures include six missing historical `session_run_id` assertions, three count mismatches, and two whole-position mismatches. The eight taskless-decision failures are concentrated in PR #1296, rather than eight independent PRs. Their old result/row expectations need to be read against that cutover; this investigation does not establish a single root cause for them.

**Keep the behavior, retire obsolete tests with their owner.** [66eb9c5c6a](https://github.com/loopflowstudio/loopflow/commit/66eb9c5c6aa4b429f3556b5d50867ea14930d657) removes the review-launch controller and its test module; subsequent Workflow work replaces that model. Do not resurrect those tests because they rank highly. Keep focused migration/backup and present-day Workflow/Flow transition proofs. For current fixtures, use the prepared candidate executable and isolated store; no installed binary should decide the outcome. These historical failures support that boundary, not a broad new test layer.

### 4. Planning reconnect needs a smaller, faithful fixture, not deletion

**High confidence on several fixture mismatches; changed contract on October 12.** The two leading reconnect tests share `tests/e2e/planning_reconnect.py`, reached through `rust/loopflow/tests/planning_reconnect_tests.rs:21–74`. Three PR #1499 runs fail both tests because the fake Linear response lacks `dueDate`; [f5cedb7071](https://github.com/loopflowstudio/loopflow/commit/f5cedb707114d3c110084bebb9b660dd389978e3) adds the field. PR #1521 fails because the fake mutation expects a top-level name while production sends an input object; [d5b3ee6872](https://github.com/loopflowstudio/loopflow/commit/d5b3ee68726e52a9024aace7638dd119c4aad833) repairs that fake response and success result. Later changes [1f664032a8](https://github.com/loopflowstudio/loopflow/commit/1f664032a8290c3cf010adbb06258e50faacdcbb) and [068375b595](https://github.com/loopflowstudio/loopflow/commit/068375b5953b25245867502e0c8d1a50667f09a1) update provider-stream consumption, canonical table assertions, and actual outage setup.

PR #1540's old expectation of catching up while an agent is idle conflicts with its deliberate removal of per-agent polling. [The final diff](https://github.com/loopflowstudio/loopflow/commit/9d0d41f5e07fa938a34677f10a6716c4de99b36d) asserts idle silence and recovery at Flow boundaries instead. That is a changed product contract, not evidence to restore continuous polling to satisfy an old test.

**Keep/redesign:** retain a real CLI-to-local-HTTPS recovery proof for each distinct watcher/Flow boundary, preserving pending writes and Task placement without duplicate delivery (`tests/e2e/planning_reconnect.py:386–410`). Share minimal valid provider payloads and verify small mutation/decoding results before launching the long scenario. Replace retired-table assertions with observable saved state where possible. Do not add a live Linear call. Baseline `docs/reviews/ci-under-five-minutes.md:244` measured one reconnect case at **156.229s**; that cost is independent of this failure frequency and does not justify deleting recovery coverage.

### 5. Desktop timing failures protect real interaction, but the timeout sampled stale state

**Medium-high confidence.** All seven historical block-copy failures report zero blocks when two were expected at `GhosttyTerminalInputTests.swift:42`. [1b059203b6](https://github.com/loopflowstudio/loopflow/commit/1b059203b60f1673f4736dfdcc8c6941b3330087) changes the polling loop to read state again after a delayed wake-up before declaring timeout. That diff supports a scheduling/fixture explanation; there was no explicit same-SHA retry proving this particular test stable afterward. The later terminal redesign also changes the test surface, so this is not a current seven-failure flake estimate.

**Keep/fix:** copying the selected command/output and clearing selection at a live prompt benefits Desktop users. Keep a headless interaction proof; repair observation/fixture readiness rather than asserting a narrow elapsed-time budget or deleting terminal interaction coverage. Do not infer that all Swift failures share this cause: 18 unnamed Swift failures are compilation errors and five are whole-step 20-minute timeouts.

### 6. Installation acquisition is a recurring environment failure; green selectors can still omit proof

**High confidence.** Fourteen of the 44 failed installation jobs fail acquiring the image, with no named test failure. Seven are attempts 1–7 of [PR #1511 run 37992574422](https://github.com/loopflowstudio/loopflow/actions/runs/37992574422), all on SHA `5ed36f28`: Docker Hub header/auth timeouts, then a successful attempt 8. Other incidents include registry rate/data limits and a cold image creation timeout. Changing the default registry in [9a2b5168f1](https://github.com/loopflowstudio/loopflow/commit/9a2b5168f150d036ca8626483621e29d9e1e4f6e), then bounded acquisition handling in [PR #1529](https://github.com/loopflowstudio/loopflow/pull/1529), addresses infrastructure. It did not eliminate all incidents: [job 114588670169](https://github.com/loopflowstudio/loopflow/actions/runs/38179408870/job/114588670169) still exhausts the mirrored registry's data limit before tests run.

**Fix acquisition, preserve isolation.** Keep local image reuse and distinguish acquisition failure from a test failure; avoid retrying successful suites. Current `scripts/test_task_installation.py:23–67` already makes that distinction. Further mirrors/cache changes need measurement, not an assumption that more retry code is useful. No hardware pricing conclusion follows from registry outages.

**Concrete false-green finding:** [successful job 114041830183, attempt 8](https://github.com/loopflowstudio/loopflow/actions/runs/37992574422/job/114041830183) reports **zero passed, 29 filtered out** for `tests/flow_tests.rs`. At that SHA, `scripts/test_task_installation.py:75–77` uses `--exact --ignored`, but `rust/loopflow/tests/flow_tests.rs:2019–2020` defines the selected topology/return-count test without `#[ignore]`. Cargo succeeds after selecting nothing. [2fa19f2309](https://github.com/loopflowstudio/loopflow/commit/2fa19f2309a63cc44a339fe8f25954bee123fd2c) changes selection to `--include-ignored`; that fix had not reached this branch. Later [1930805f40](https://github.com/loopflowstudio/loopflow/commit/1930805f40a52cd38583510bf88e10cd5ee4a2bc) also fixes library-versus-integration target selection. Current script uses `--include-ignored` at line 135, but exit status alone still does not reject a misspelled/removed exact selector.

**Cheapest credible improvement:** make each selected proof report exactly one discovered/executed test and reject zero matches in the existing runner. This is stronger and smaller than another test that freezes the runner's command string. This four-log success sample does not measure the prevalence of historical silent omissions.

Keep the cross-Task disposable test despite five failures across four PRs. Its current assertions prove the child Flow succeeded in Y's checkout while preserving X's caller association (`rust/loopflow/tests/session_lifecycle_tests.rs:1265–1288`). [38e8b62f40](https://github.com/loopflowstudio/loopflow/commit/38e8b62f4004027994a4327164c367052ef06872) corrects fixture agent defaults after an explicit invocation-wide override correctly reached the child. Removing that ownership proof would lose more than removing a prose assertion.

### 7. Registry completeness is implementation-facing, but deletion is not yet justified

The six `registry_is_complete` failures include a newly introduced `wave cron sync` command not classified in the test registry ([job 110493528558](https://github.com/loopflowstudio/loopflow/actions/runs/36899101786/job/110493528558)). Current `rust/loopflow/tests/linear/wave_resolution_matrix.rs:508–548` checks a hand-maintained classification against Clap. This catches missing test-matrix classification, not necessarily wrong dispatch.

**Simplify cautiously:** retain direct cases proving explicit selection, ambient selection, and conflicting context resolve the intended Wave. Reduce a redundant registry only after mapping its distinct command-dispatch obligations to those remaining assertions. Frequency is not evidence that all command cases are interchangeable. The prior baseline measured the broad resolution matrix at 121.612s; the tiny completeness assertion must not be assigned that whole cost.

## Same-SHA attempts: all nine retried run IDs

These are identical **head SHAs**, not controlled identical environments. Runner images, network, cache, fetched history and possibly merge-ref checkout can differ. A later success is evidence of instability of the overall check; it does not prove a harmless test flake.

| Run / PR | Attempt sequence | What the job evidence establishes |
|---|---|---|
| [36350070259](https://github.com/loopflowstudio/loopflow/actions/runs/36350070259), #1302 | cancelled → success | Swift cancelled in attempt 1; aggregate failed. No failed test identified in that attempt. |
| [36520042451](https://github.com/loopflowstudio/loopflow/actions/runs/36520042451), #1312 | failure → cancelled | Swift Cargo dependency acquisition failed; retry was cancelled. No successful retry evidence. |
| [37371691156](https://github.com/loopflowstudio/loopflow/actions/runs/37371691156), #1444 | failure → success | Python/Rust/lint/installation cancelled initially; aggregate red. This is not a named-test fail/pass. |
| [37370458281](https://github.com/loopflowstudio/loopflow/actions/runs/37370458281), #1447 | failure → failure → success | Scratch job cancelled in the first two attempts; dependent proof skipped. No test flake established. |
| [37365608251](https://github.com/loopflowstudio/loopflow/actions/runs/37365608251), #1445 | failure → cancelled → failure | Different upstream jobs cancelled; aggregate remains red. Cancellation reason not established. |
| [37269226752](https://github.com/loopflowstudio/loopflow/actions/runs/37269226752), #1437 | failure → success | Installation step failed without an extracted test/error cause; successful retry has seven one-test passing summaries. Instability established, cause unknown. |
| [37992574422](https://github.com/loopflowstudio/loopflow/actions/runs/37992574422), #1511 | seven failures → success | Image acquisition timed out seven times. Successful retry exercised tests, but one exact selector selected zero; see above. |
| [38179408870](https://github.com/loopflowstudio/loopflow/actions/runs/38179408870), #1491 | failure → success | Registry data limit, then nine one-test passing summaries. Installation environment failure, not failed test assertion. |
| [38119214480](https://github.com/loopflowstudio/loopflow/actions/runs/38119214480), #1522 | failure → failure | Installation rate-limit failure recovered and nine selected tests passed; the **same four Rust tests failed in both attempts**. |

For the last row, `launch_nonzero_exit`, `bare_lf_launches_a_conversational_prompt_with_an_agent_option`, `mixed_provider_flow_keeps_launch_accounts_across_steps`, and `headless_resume_reads_the_saved_workspace_and_retains_process_provenance` fail with surviving-process-group or unavailable headless identity errors ([first Rust job](https://github.com/loopflowstudio/loopflow/actions/runs/38119214480/job/114410351335), [second](https://github.com/loopflowstudio/loopflow/actions/runs/38119214480/job/114412869035)). Keep them pending diagnosis: these errors touch process ownership and cleanup. The installation retry's success is not evidence that those four failures were harmless. No isolated later fix is attributed to them here.

## Failures without test identities and remaining uncertainty

| Failed jobs without parsed test names | Count | Interpretation |
|---|---:|---|
| tests-result | 374 | Aggregate result, not independent proof |
| scratch-clear | 156 | Historical delivery policy |
| architecture-check | 31 | Static map checker; same 31 incidents also identified in Python |
| rust-lint | 24 | Formatting/static-analysis job; individual root causes not exhaustively classified |
| task-installation | 24 | 14 image acquisition, 7 compilation, 3 unresolved step failures |
| swift-test | 26 | 18 compilation, 5 step timeouts, 1 Cargo dependency fetch, 2 unresolved process/runner failures |
| loopflow-ui-test | 21 | All contain compiler errors; no evidence that a UI test assertion ran and failed |
| rust-test | 7 | All contain compiler errors |
| e2e-smoke | 8 | 4 compilation, 4 shell-smoke failures including obsolete `wt` command resolution; no structured test name |
| website-test | 1 | `setup-uv` fetch failure |
| migration-check | 2 | Missing released `0.12.24.001_release.sql` relative to origin/main, both on one PR; static protection of released history |
| **Total** | **674** | No missing failed-job logs; missing identities are accounted for above |

The three unresolved installation job IDs are `113392985283`, `111632486201`, and `114399684003`; unresolved Swift job IDs are `113117237485` and `111387200620`. Raw evidence is retained locally. The five Swift timeouts identify a 20-minute job-step boundary, not a proven single hanging test. Compilation failures often repeat across Rust, Swift CLI-build, installation and Xcode jobs; their job count is not their independent defect count.

The classification is deliberately sampled rather than a manufactured all-failures pie chart. All failed logs were parsed; assertions and fixes were investigated most deeply for the recurring entries above. Golden changes, several old cutover failures, lint causes and remaining named tests lack full causal classification. A later passing candidate can include many edits or an updated base; a nearby fix diff is stronger than a commit title but still not an isolated experiment. No broad suite was rerun to claim causality.

Source line references describe the supplied checkout during investigation (HEAD observed `f52728b4bc68a92f5142cf93c446a4666d4c7712`) unless a historical SHA is named. Parent changes continued independently. Historical CI commands were taken from logs/commit source, not inferred from the current workflow. Existing cost measurements remain in `docs/reviews/ci-under-five-minutes.md` and `scratch/ci-baseline`; this report does not turn failure counts into runtime savings, runner pricing, or a five-minute acceptance claim.

## Most decision-useful findings

1. **Remove one duplicate architecture repository check.** All 31 failures coincide across Python and the dedicated job. Keep one owner and the checker's fixture tests.
2. **Stop treating all red jobs as failed tests.** Aggregate and historical scratch policy account for 530 of 1,006 failed-job observations; 21 UI jobs failed compilation, not UI assertions.
3. **Reduce editorial test code selectively.** The renamed Session-history string is a demonstrated brittle assertion. Stale generated HTML should have one cheap freshness proof; installer/security wording does not prove runtime safety.
4. **Fix shared fixture fidelity before deleting behavioral tests.** Twelve leading Task-boundary failures lacked the candidate binary; reconnect failures repeatedly came from stale fake payloads/protocols. Retain boundary and recovery assertions.
5. **Installation retries primarily expose acquisition problems.** Fourteen jobs failed image acquisition; seven retries belong to one SHA/PR. Reuse successful proof and address acquisition without discarding disposable-account or published-authority coverage.
6. **Make zero selected tests fail the proof runner.** A green installation rerun demonstrably ran zero tests for the selected Flow proof. Existing selector fixes help, but success status alone remains weaker than claimed execution.
7. **Keep slow tests with concrete protection.** Migration/backup preservation caught a real nested-transaction defect; cross-Task installation, planning recovery and terminal copying protect distinct user behavior. Optimize setup and narrow obsolete assertions.
8. **Do not label rerun recovery “harmless flakes.”** Six run IDs eventually succeeded, but three recovered from cancellation-only upstream failures; another run repeated four Rust ownership failures unchanged. Resolve causes at the assertion/environment boundary and retain unknowns explicitly.
