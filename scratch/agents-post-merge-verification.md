# LOO-873: model-unavailable fix forward

Publishing and post-merge verification remain; the final checks are recorded below. The model-unavailable
repair, Auto-Ship stop repair and compress changes are committed through
`3c721c645e`, including the sync onto `dfbea8cf81`. The focused refusal regression
and isolated CLI stop/status/restart scenario passed again after compress.
The earlier incomplete validation is superseded by those results, not by
PR #1906's merged CI. No Wave or product question is open.

## Outcome and constraints

One ordered `agents` list carries provider and model choices. A contextual
`default_agent` moves or adds an entry to the front; a narrower list replaces
the wider list according to existing setting precedence. `agent` and `-a`
pin a provider, and a named model pins that model. Bare Claude retains its
existing Opus → Sonnet → Haiku fallback when it has no configured next entry.
Keep the deprecated comma `default_agent` readable with its warning.

The Task comments record these words from Jack Heart:

- `3620444c-bdc4-44fe-837e-fa59603ad3d6`: “maybe we should say agents needs to be: [claude:opus, codex, claude:fable]”.
- `6642bd2d-cac8-48ee-9520-64a7e2813548`: “if your top level is : agents: [a,b,c] / default_agent: d / then the real order is d,a,b,c”.
- `a2be0aec-0f87-48d0-97d6-bfda1134b7d4`: “Only for the scope at which the default_agent works”.
- `aa962b91-55ca-4d3f-b9a6-53774dbce716`: “not too worried about occasionally duplicative trying of same harness”.

Task comment `b488fa11-c370-4127-957c-fc85437801a9` includes
“when claude:fable is refused (usage limit or model unavailable)” and says to
try another Claude model before Codex. Later direction puts that model order
in `agents`. The merged PR acknowledges that only usage limits fall through.
That disclosure did not deliver the model-unavailable claim; this repair adds
it without changing the configuration choices above. No product decision is
currently open.

## Upstream and merged CI

`git fetch origin main` during realign at `3c721c645e` succeeded;
`git log --oneline HEAD..origin/main` was empty. Since #1906, #1901
(`dfbea8cf81`) changed planning and Session reads,
including provider-transition reads. It is already included in `31d2fc6d28`;
the provider-result regression passed after that sync. No Wave is assigned.

- PR: https://github.com/loopflowstudio/loopflow/pull/1906, merged
  2026-11-13T02:59:45Z at `112c27a053e19f5fe08c5394c624ece981acbd8e`.
- `git fetch origin main`; `lf sync --manual main --json` printed
  `Synced onto main; the sync and edits remain local.` At that verification, HEAD was
  `51aa9a3b1a8bda076d46e894405d9e407ae91d54`. Its tree and the merge commit's
  tree are both `cdaca6bea8ebc5e8d47298f2ee88c4d24f81ad68`.
  `git diff --exit-code origin/main -- . ':!scratch'` exited 0.
- **Post-merge push:** https://github.com/loopflowstudio/loopflow/actions/runs/43355918497
  — event `push`, exact merge SHA above, `completed`, `success`. Initially
  in progress; success observed again at the end. The workflow reused the
  exact-commit queue proof for some suites; lint, build, Task installation,
  e2e smoke and the aggregate result succeeded in this push run.
- **Merge queue, separately:** https://github.com/loopflowstudio/loopflow/actions/runs/43354487892
  — event `merge_group`, exact same SHA, `completed`, `success`.
- **PR checks, separately:** https://github.com/loopflowstudio/loopflow/actions/runs/43352284784
  — the PR's checks succeeded. They are not the push-run result.

## Merged-version observations

All fixture invocations removed inherited `LF_*`/`REX_*` variables and used
private `HOME`, `LF_HOME`, Claude and Codex homes under `/tmp/lf-verify-873`.
`scripts/test_network.py` denied external network access. Claude stream JSON
and Codex Unix WebSocket app-server stand-ins supplied refusals; no personal
login or real provider quota was used.

- `cargo build -p loopflow --bin lf --jobs 4` with inherited `LF_*` unset and
  rustup stable first on PATH: passed in 4m 59s. Copied binary reported
  `lf 0.15.3+51aa9a3b1 (build 51aa9a3b1 on v0.15.3)`.
- `uv run --no-sync python scripts/test_network.py cargo test -p loopflow
  --lib --jobs 4 -- config::settings:: agent::tests:: ops::task_execution::
  a_run_stopped_by_a_usage_limit a_run_that_ended_waiting a_restarted_wait
  a_shipping_tasks_restarted_wait hard_limit_moves_the_route_to_the_next_account
  a_fable_limit_leaves_other_models_on_the_account
  a_credits_rejection_holds_the_launched_model
  a_model_the_held_login_cannot_take --test-threads=4` with the built CLI first
  on PATH, inherited `LF_*` unset, stdin closed: **79 passed, 0 failed**,
  execution 48.23s. Log: `/tmp/lf-verify-873/tests.log`.
- `lf set --global agents claude:opus,claude:sonnet`; `lf config settings`:
  `agent    claude:opus, then claude:sonnet`.
- `lf set --global skill.verify-step.default_agent claude:haiku`;
  `lf config settings --skill verify-step --explain`: Haiku, then Opus, then
  Sonnet. Inspection without the skill still showed Opus, then Sonnet.
- `lf set agents claude:haiku`; `lf config settings --explain`: Haiku alone,
  `Personal project`; the global list was overridden.
- A comma `default_agent` remained readable and printed
  `is the old spelling; write agents: [claude:opus, claude:sonnet] and default_agent: claude:opus`.
  The initial driver incorrectly expected the literal word `deprecated`;
  correcting that assertion required no product change.
- `lf -b graph verify-order`, one `verify-step`, Opus refused:
  `Continuing on claude:sonnet: claude_result: You've hit your usage limit for opus. Upgrade or try again later.`
  Sonnet completed; exit 0.
- Same command with `agents: [codex, claude:sonnet]`, Codex emitted a protocol
  usage-limit error:
  `Continuing on claude:sonnet: codex_error: You've hit your usage limit. Try again later.`
  Claude completed; exit 0. With no saved Codex transcript, the log explained
  that Claude received the whole task; this exercised launch refusal, not
  transfer of an existing conversation's transcript.
- `lf -a claude:opus -b graph verify-order`: Opus only; exit 1;
  `Stopped: No fallback remains. Change an agent or -a pin ...`.
- Both models refused: one launch per entry, exit 1;
  `Stopped: Nothing is left in its order after claude:opus, claude:sonnet; add another agent to agents`.
- Two seeded managed Claude logins, ordered `first, second`, same model:
  the first refused and the second succeeded; exit 0. The stand-in's launch
  log showed `first/opus`, then `second/opus`. The first fixture attempt read
  an unrelated scoped Keychain item and falsely refused twice, reaching the
  expected capacity wait. It was stopped; the corrected fixture selected the
  current home's Keychain item, and passed. No product repair was needed.

## Current approach

`lf/commands/graph.rs::TaskRunStopped::stop` now prints the stop, withdraws
Auto-Ship approval and returns the original exit status for both callers.
`ship_halt` does nothing without approval and sets no person's stop marker.
The refusal test uses `support::recorded_graphs` for both Graph counts.

`agent/mod.rs::_run_with_transient_retries` is the single walk: model
refusals skip account rotation and quota recording, while usage limits try
logins first. `_climb` retains a same-provider thread or imports history across
providers, supplying the original task when no transcript is available.
`ops/task_execution.rs::limit_stop` reads the existing recovery record and
removes duplicate trailing periods. Configuration, schema and reset waits stay
unchanged; docs are in `docs/{config,orchestration,subscriptions,troubleshooting}.md`.

## Unobserved cases to retain

- A refusal scoped to one login's plan ("not available on your plan") reads
  the same as a model that is down, so a second login that could run the model
  is skipped and the step goes on at the next entry. Trying logins would need a
  per-login model hold. Revisit if a provider is seen sending one.
- A line naming a model with both "is currently unavailable" and "try again
  later" classifies as a model refusal: it moves when an entry is left, and on
  a pin or the last entry stops once instead of backing off. The observed
  Fable line has no retry wording.
- Fable unavailable, then every Opus login held until a known reset: the step
  waits on Opus capacity; the wait walks from Opus onward and does not launch
  Fable again.

## Remaining work

- Publish the repair, then verify the merged repair and its push CI. PR #1906
  validates the earlier usage-limit implementation only. No app or release
  check applies to this CLI-only change.

## QA findings and resolution

The first built-CLI QA results are in `/tmp/lf-qa-873/results.json`;
`scratch/qa.html` was not retained. The retained CLI snapshots below cover the
stop/status/restart acceptance without that walkthrough artifact.
That walkthrough found two stop-line faults:
a direct `lf -b <skill>` (no fallthrough outside a Graph step) advised "add
another agent to `agents`" when the list already had one, and a provider reason
ending in a period printed `model.. No fallback`. A third fault was
behavioral: `lf -b task run` approves Auto-Ship but did not withdraw it on a
refusal stop, so the next Task read restarted `lf ship`, which met the same
refusal once more before halting. `TaskRunStopped::stop` now calls `ship_halt`
for both commands. The regression incorrectly expected
`ship_stopped_at` (a person's stop); it now checks withdrawn `shipped_at`,
repeated status reads past the retry deadline, and manual continuation.
The corrected regression and isolated CLI repeat passed, including after
centralizing stop handling in compress (below). The prior direction to finish
this regression and diagnose a hang is resolved: the long run was compiling
its fixture CLI, then failed an incorrect assertion; the prepared-CLI run
completed in 5.77s.
The former `/tmp/lf-qa-873/test.log` has no completed suite result;
`test2.log` exited 127 because `timeout` was unavailable. Neither is a passing
result; the completed runs below supply the missing validation.

## Auto-Ship validation, November 13 UTC

- `LOOPFLOW_TEST_LF="$PWD/target/debug/lf" uv run --no-sync python scripts/test_network.py cargo test -p loopflow --lib --jobs 4 a_run_stopped_by_a_usage_limit -- --nocapture --test-threads=1 < /dev/null` (inherited `LF_*` removed, stable Rust, `TMPDIR=/private/tmp`): **1 passed, 0 failed**, 5.77s after a 1m 19s test build; `/tmp/lf-qa-873/refusal-regression-fixed.log`. The first run built its fixture CLI internally and then failed the incorrect `ship_stopped_at` assertion after 536.71s. A process sample showed Rust compilation, not a fixture deadlock; the prepared-CLI run completed normally. The macOS linker again reported the large `__eh_frame` warning.
- `uv run --no-sync python scripts/test_network.py "$PWD/.venv/bin/python3" /tmp/lf-qa-873/refusal-followup.py < /dev/null`: **passed**, `/tmp/lf-qa-873/refusal-cli2.log`; full results `/private/tmp/lf-qa-873-followup2/results.json`. The initial driver used unsupported `server status --json`; removing `--json` fixed the driver before the fresh run. No product change was needed.
- `cargo fmt --all -- --check`; `git diff --check`: **passed**. Gate owns broader suites and Clippy.

The isolated CLI used a fresh git-planning repository, private Machine and
provider homes, a fake Keychain, one managed Claude login, and provider protocol
fixtures with external networking denied. `first-step` ran on pinned Opus;
`verify-step` ran on pinned Fable and received
“Fable 5 is currently unavailable. Switch to another model.”
It stopped once: one Graph, two provider launches, no Auto-Ship approval, and
no person's stop marker. Three text status reads, then text and JSON reads
after moving the failed-process timestamp past the retry deadline, left those
counts unchanged. Status said “stopped after a provider refusal” and no longer
said “Ship failed” or printed doubled punctuation.

Removing the verify-step pin and manually running the Task completed the second
Graph at `[2/2] verify-step`: Fable refused, Opus completed, and first-step did
not run again. Quota-window count stayed zero and the account cooldown stayed
unset. Auto-Ship was approved again by the manual headless run. The fixture
server and credential holder, plus the previous QA server, were stopped.

## Compress checks, November 13 UTC

- `cargo build -p loopflow --bin lf --jobs 4` (inherited `LF_*`/`REX_*` removed, stable Rust): passed, 5m 11s; `/tmp/lf-873-compress-checks.log`.
- `uv run --no-sync python scripts/test_network.py "$PWD/.venv/bin/python3" /tmp/lf-873-compress-cli.py < /dev/null`: passed after centralizing stop handling; `/tmp/lf-873-compress-cli.log`, full snapshots `/private/tmp/lf-qa-873-compress/results.json`. One refusal stop; repeated text/JSON reads beyond the retry deadline left one Graph and two launches. Manual continuation added only Fable → Opus at the stopped step; quota windows stayed empty and cooldown unset. The isolated server and holder were stopped.

- `LOOPFLOW_TEST_LF="$PWD/target/debug/lf" uv run --no-sync python scripts/test_network.py cargo test -p loopflow --lib --jobs 4 -- agent::tests:: ops::task_execution:: a_run_stopped_by_a_usage_limit task_ship_stops_shipping_when_its_run_fails_the_same_way_every_attempt a_stopped_run_is_started_again_from_where_it_began --test-threads=1 < /dev/null` (same isolation): **54 passed, 0 failed**, 46.20s after a 1m 16s build; `/tmp/lf-873-compress-checks.log`. The macOS linker reported the existing large `__eh_frame` warning. Stopped the test's leftover holder, whose log was under `/private/tmp/.tmprcvdzx`.
- `cargo fmt --all -- --check`; `git diff --check`: passed. `lf context`: scratch under both limits; broader suites and Clippy remain with gate.

## Earlier repair checks

- After sync onto `dfbea8cf81434b72c2243a416827a29994afc877` as `31d2fc6d28`: `uv run --no-sync python scripts/test_network.py cargo test -p loopflow --test agent_tests --jobs 4 model_unavailable_result_falls_through_or_stops_without_spending_quota -- --exact --test-threads=1` (inherited `LF_*`/`REX_*` removed, stable Rust, stdin closed): passed, all three scenarios, 281.22s including fixture CLI build.
- Earlier classification, order and pin checks: `git show 31d2fc6d28:scratch/agents-post-merge-verification.md`, “Repair check results”. The uncommitted pre-compress notes, including the Auto-Ship QA, were moved to `/tmp/lf-873-pre-compress-plan.md` before editing.

## Reproduced gap before repair

Configuration: `agents: [claude:fable, claude:opus]`.

`lf -b graph verify-order` received this Claude error result:

```text
Fable 5 is currently unavailable. Switch to another model.
```

It printed:

```text
Error: agent exited with code 1: claude_result: Fable 5 is currently unavailable. Switch to another model.
Error: verify-step: claude_result: Fable 5 is currently unavailable. Switch to another model.
```

Exit 1. The recorded model sequence was only `['fable']`; Opus never launched.

Fixture scripts and raw results remain under `/tmp/lf-verify-873`:
`check.py`, `remaining.py`, `managed_retry.py`, `unavailable.py`, `results.json`.
The local build and all test commands ended. The timed-out fixture's children
were terminated; final process inspection found no remaining verification
CLI, provider, server or watcher processes.

## QA at `3c721c645e`, November 12

- Fresh build `lf 0.15.3+3c721c645.dirty`. `scripts/test_network.py` ran `/tmp/lf-qa-873-final/drive.py`: every configuration, fallthrough, pin, exhaustion, direct-skill and legacy-spelling scenario **passed** on the first run. It also ran `task_stop.py`, a copy of the compress Task driver rooted at `/private/tmp/lf-qa-873-final-task`: **passed**. The walkthrough is in `scratch/qa.html`. No product fix was needed.
- Seen, but not changed: the setting-validation errors share the `invalid graph:` prefix (`lf set --global agents clod:opus`). This predates the change.

## Publication checks, November 13 UTC

- `lf sync`: passed without conflicts; actual base `dfbea8cf81434b72c2243a416827a29994afc877`, product code unchanged from the previous QA.
- `scripts/test.py --reuse-passing --rust-filter 'test(agent::tests::) | test(ops::task_execution::) | test(task_graph_launch_tests::) | binary(agent_tests) | test(documented_commands::)'`: stopped before product checks because the uv-cache inventory exceeded 15 seconds; free disk was 47.5 GiB. Used the manual plan documented in `scripts/testing.md`; `/tmp/lf-873-publish-gate.log`.
- Architecture, Rust test inventory, `cargo fmt --all -- --check`, `git diff --check`: passed.
- `cargo build -p loopflow --bin lf --jobs 4`: passed, 1m 16s; `/tmp/lf-873-publish-build.log`.
- The same filtered nextest plan, with network sandboxing, prepared CLI, inherited `LF_*` removed and stable Rust: 128 passed, 1 failed in 116.071s; `/tmp/lf-873-publish-tests.log`. The failed capacity-wait restart fixture created an ordinary repeated failure before inserting its synthetic wait. The repaired terminal stop now withdraws approval during that initial failure. The fixture now restores the approval that a genuine capacity wait retains; production code was unchanged.
- `cargo test -p loopflow --lib --jobs 4 -- a_shipping_tasks_restarted_wait a_restarted_wait --test-threads=1` under the same isolation: all 3 affected fixture cases passed, 15.60s; `/tmp/lf-873-publish-restart.log`. The other 128 passing tests retain their result. Builds reported the existing macOS large `__eh_frame` linker warning.
- `cargo clippy --all-targets --jobs 4 -- -D warnings`: passed, 4m 44s; `/tmp/lf-873-publish-clippy.log`. Cargo emitted manifest-level unused-dependency warnings for unchanged `chrono-tz`, `rpassword`, and `tokio-stream` entries.
- `website/dev.py test` under `scripts/test_network.py`, with the prepared CLI on PATH: passed; `/tmp/lf-873-publish-website.log`. The first invocation used a root-relative script path from the website directory and exited before running checks; the corrected command above completed.
