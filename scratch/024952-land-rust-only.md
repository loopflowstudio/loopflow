# 024952 — Land Rust-only PRs on Linux-speed CI

Design accepted with Jack 2026-10-27; see the Task description. Jack, 2026-10-27:
all four items ship in this one PR; publish only after items 2–4 are built.
`scratch/ci-churn-research.md` is the evidence for items 2–4.

## 1. macOS gating — built
- `scripts/macos_inputs.py` → per-job outputs (`swift-test`, `macos-process-test`, `loopflow-ui-test`, `any`).
- macOS jobs moved to reusable `.github/workflows/macos.yml`; `ci.yml` calls it on `pull_request` only.
  `tests-result` accepts a skipped `macos` outside PRs, or on a PR when the classifier succeeded and said none.
- `.github/workflows/macos-main.yml` + `scripts/macos_main.py`: post-merge batch since `refs/tags/last-green`;
  green force-moves the tag; red bisects first-parent via `workflow_dispatch` and opens an unarmed `macos-revert/<sha>` PR.
- Nightly smoke `lf list` uses a temp `LF_HOME`. Dependabot close-on-red strips `macos / ` prefixes from check names.
- Deleted: `affects_macos`, `_UNAFFECTED`/`_STILL_AFFECTED`, the single `macos` output, and `ci.yml`'s
  inline macOS jobs with their merge-proof cache-warming conditions.

## 2. Land resync only on a reported conflict — built
`prepare_pr` reads GitHub before disarming (`needs_resync` → `pr::pr_requires_resync`) and merges main only for:
a local merge, no PR yet, a stacked child whose parent landed, `mergeStateStatus` DIRTY, BEHIND with no merge
queue (strict checks would otherwise strand auto-merge), or a queue eviction with reason `merge_conflict`.
Otherwise the repair is pushed on top (`push_with_upstream_if_needed`). Both `require_task_pr_range_nonempty` proofs stay.
Test: `final_preparation_keeps_published_history_and_merges_main_only_on_conflict` (was `…when_the_base_changes`,
which assumed a sync on every land).

One rule (concept review, 2026-10-27, built): `pr::pr_requires_resync(&PrMergeObservation, removal)` decides
"GitHub can't merge this PR as it is" for `lf land` (`needs_resync`), the landing watcher (`decide_merge` →
`required-integration`) and the release wait (`NeedsIntegration`). Queue membership → no; DIRTY → yes; BEHIND only
without a queue; an unarmed PR on a queue evicted for `merge_conflict` → yes. `merge_needs_integration` deleted, so a
repair disarm on a queue with BEHIND no longer tells the repair loop to integrate. `decide_merge` takes the observation.
Second concept pass (after #1741/#1745): `lf land`, the watcher and the release wait still read the one rule; a
queue eviction for `merge_conflict` surfaces as a failing check and the following `lf land` resyncs. Nothing to change.

## 3. Flaky tests — investigated; nothing left to quarantine
The brief asks to quarantine the report's cross-PR flaky tests. The evidence below finds each one fixed, deleted,
a real failure, or a production bug, so this PR quarantines none. Review should confirm that reading.
CI logs since Sep 29 (about 280 job logs) give these causes:
- `session_steers::public_context_capability…`: partial `handoff.json` read. Fixed by #1731 (tmp file + rename).
- `install::artifact_tests::a_settled_install…`: running binary pruned before it was observable (Oct 8–9).
  Fixed in #1615, which waits for the child's `ready` line.
- `ProgramStatusTests.staleSurfaceAndClosedCallbacksCannotPublish`: 300 ms fixed wait. Already observation-driven with `.timeLimit`.
- `registry_is_complete`: real failures, from unclassified `history search` and `wave new-chapter` on their own branches.
- `loopflow_warnings_preserve_native_drafts…`, `native_runner_reconnects_planning…` and `task_delete_binary…`
  are no longer in the tree.
- Swift 20-min timeouts: the hung tests (Git planning, Navigation preview/Apply) have been deleted. Since then there
  has been one hang, `AppAppearanceTests` "window modifier follows preview, Cancel and Apply" (Oct 25). The PR step
  limit is 5 min (already on main) and `test_desktop.sh` names unfinished tests. One occurrence is below the deletion bar.
- Still open, outside this PR: `database is locked` / `SQLITE_BUSY_SNAPSHOT` (extended 517) in waited landing
  (3 runs under its old name), `work_watch` and `store_contention`. It is a production bug, so the tests should
  stay. Likely cause: deferred `conn.transaction()` read→write upgrades, which `busy_timeout` never retries
  (`store/sqlite.rs`, `durable.rs`, `repositories.rs`, `linear/observations.rs`). Candidate Task: IMMEDIATE write transactions.

## 4. Conflict hot spots
- Built: `docs/architecture-reference.md` keeps the intro and the checked inventory; the contract moved by area to
  `docs/architecture/reference-{cutover,models,code,runtime,processes}.md` (website pages registered). Inbound
  anchors rewritten, AGENTS.md included; release notes left as shipped.
- Built: `docs/lf-reference.md` commits only its preamble; `website/dev.py sync-docs` appends `lf help --all --json`.
  `reference_matches_the_compiled_cli` deleted. AGENTS.md, TESTING.md and `.lf/skills/pr-message.md` updated.
- Built: `ops/task.rs` 6,841 → 2,691 lines. PR delivery moved to `task/delivery.rs`, read-only views to
  `task/snapshot.rs` and tests to `task/tests.rs`; callers keep their `ops::task::` paths through re-exports.
  `ops/human_session.rs` 2,412 → 1,579 lines, with all its tests in `human_session/tests.rs`. Its record/launch
  production seam stays: one file for one owner was enough.
- Out of scope (Jack, 2026-10-27): Wave memory as one file per entry is Task 0662af.

## After merge (PR notes)
Ruleset 11018096 before: `strict_required_status_checks_policy: true`, `grouping_strategy: ALLGREEN`.
Apply:
```
gh api repos/loopflowstudio/loopflow/rulesets/11018096 > before.json
jq '{name,target,enforcement,conditions,bypass_actors,rules: [.rules[]
  | if .type == "required_status_checks" then .parameters.strict_required_status_checks_policy = false
    elif .type == "merge_queue" then .parameters.grouping_strategy = "HEADGREEN" else . end]}' before.json > after.json
gh api -X PUT repos/loopflowstudio/loopflow/rulesets/11018096 --input after.json
```
Then check:
- A Rust-only PR's PR and queue runs skip macOS with `tests-result` green.
- `macos-main.yml` runs once and creates `last-green`.
- The next nightly passes.
- Measure the week before vs after: open→merge median/p90, macOS jobs per merged PR, and post-arm resyncs and conflicts per PR (item 2).

## Checks
Item 1: `uv run pytest python/tests/test_macos_inputs.py test_macos_main.py test_ci_proof.py test_release_automation.py` → 81 passed; `actionlint` clean.
Item 2: `cargo test -p loopflow --test integration land_tests::` → 30 passed.
Item 4: `scripts/check_architecture.py` passes; `pytest python/tests/test_architecture.py` → 31 passed; `cargo clippy -p loopflow --all-targets -D warnings` clean.
`cargo test -p loopflow --lib -- ops::task:: ops::human_session::` → 70 passed.
The `website/tests` suite needs playwright and is deferred to CI `website-test`.
Compress: `cargo clippy --all-targets -D warnings` clean; `ops::human_session::` lib → 34 passed; `land_tests::final_preparation` → 2 passed.
After syncing main (70825b1e8a): `scripts/check_architecture.py` now skips out-of-line `tests.rs` modules (the split's `/bin/sleep` fixtures read as production edges); clippy/fmt clean; `ops::task:: ops::human_session::` lib → 72 passed; `land_tests::` → 30 passed; Python CI/architecture suites → 103 passed; actionlint shows only pre-existing SC2016 info in `dependabot-auto.yml`.
Compress after sync: inlined `render_lf_reference` into `website/dev.py::_generate_lf_reference`; `_generate_lf_reference` on a temp copy → 2,428-line reference from installed lf. No other reductions found.
Checkpoint 8d6fd07572 (all four items committed): `cargo fmt --check` clean; earlier focused checks still apply, no code changed since.
Compress after checkpoint: `cargo clippy --all-targets -D warnings` and `cargo fmt --check` clean; #1743's architecture-reference edit carried into the kept inventory; only a doc-comment rewrap in `land.rs`.
Realign after merging #1742 (06a199b4b3): it touches none of the split or gated files; no code change, earlier checks still apply.
One resync rule: `grep merge_needs_integration` empty; `cargo clippy --all-targets -D warnings` clean; `--lib ops::pr` (incl. `pr_landing`) → 68 passed, plus `release::…merge_request_removals_decide_landing_and_release_alike`; `land_tests::` → 29/30 under machine load, the miss (`waited_land_retains_intent…`, 30 s fixture deadline) passed alone with and without this change.
Compress: `ReleasePrWait::NeedsIntegration` carries no state (an eviction's `merge_state` misnamed the cause); `cargo clippy --all-targets -D warnings` clean, `--lib ops::release` → 45 passed.
Realign after merging #1741/#1745 (26677d6f1e, no resync files touched): `grep merge_needs_integration` empty; `cargo clippy --all-targets -D warnings` clean; `--lib ops::pr` → 66/67, the miss (`pr_landing::…unchanged_incident_gets_one_repair…`, main's backoff-timing assertion at :2011, scripted observations) passed alone.
Gate (b150d79a0e, load avg ~225 on 16 cores from other worktrees' builds): `scripts/test.py --reuse-passing` → architecture pass; website caught a stale `docs/architecture.html` (portable render missed the reference split), regenerated, `test_portable_architecture` → pass; python and rust phases hit wall budgets from machine load, not hangs. Python finished by hand (remaining files → 182 passed; branch suites → 103 passed). Rust: fmt + test inventory pass, nextest reached 841/3014, its one failure (`graph::tests::an_interrupt_ends…`, 20 s timing) passed alone; full nextest deferred to CI `rust-test`.
Sync pending: main (26edfafb49) conflicts in `docs/architecture-reference.md`, `docs/lf-reference.md`, `ops/human_session.rs` (main edits tests that moved to `human_session/tests.rs`); left for the land sync.
