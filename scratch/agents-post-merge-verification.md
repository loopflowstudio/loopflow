# LOO-873: model-unavailable fix forward

**Implementation ready for gate.** The provider-result regression now completes
Fable → Opus, stops pinned and exhausted orders, and leaves quota windows and
account cooldowns untouched. PR #1906 remains merged history; this repair is local.

Task comment `b488fa11-c370-4127-957c-fc85437801a9` includes
“when claude:fable is refused (usage limit or model unavailable)” and says to
try another Claude model before Codex. Later direction puts that model order
in `agents`. The merged PR acknowledges that only usage limits fall through.
That disclosure does not deliver the model-unavailable claim. Headless decision:
repair this concrete case in the same Task, without reopening the other recorded
product choices.

The preceding pass intended ordered fallback, useful stop advice, and recovery
without replaying completed steps. Its QA covered Claude model moves and a
Task capacity wait, but left Codex-to-Claude and login-to-login recovery
unobserved. This pass exercised both successfully through the built CLI and
provider protocol fixtures. It also reproduced the model-unavailable gap on
main; this is new runtime observation, not an inference from absent earlier QA.

## Main and CI

- PR: https://github.com/loopflowstudio/loopflow/pull/1906, merged
  2026-11-13T02:59:45Z at `112c27a053e19f5fe08c5394c624ece981acbd8e`.
- `git fetch origin main`; `lf sync --manual main --json` printed
  `Synced onto main; the sync and edits remain local.` Local HEAD is
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

## Commands and results

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

## Fix-forward design (2026-11-12)

Use `AgentFailure::ModelUnavailable` for provider error lines that name an
unavailable model. The existing retry loop moves directly to the next allowed
entry, without rotating logins or recording spent quota. Preserve the thread
within a provider and the original task across providers. Explicit model pins
stop once with the existing exhaustion advice; interactive runs stay unchanged.
Generic service failures retain transient backoff. Record moves and terminal
stops through the existing recovery record so Task runs do not replay the refusal.

Delete — do not maintain: usage-limit-only stop wording in Graph and Task
status is replaced with provider-refusal wording. No deletion targets remain.
Keep the existing recovery owner, configuration, quota recording, and
reset-wait behavior; no schema change.

In `agent/mod.rs::_run_with_transient_retries`, match arms separate account
rotation, ordered fallback, credential stops, and transient backoff. Model
refusals cannot enter account rotation; saved `refusal` covers both account
and model errors when a fallback has no eligible login. The provider-result
test keeps each case's order, launches, and stop advice together in its table.

Focused checks cover classification from provider errors (never answer prose),
ordered model/provider fallback, pins, exhaustion, and unchanged quota evidence.
Remaining: gate owns `agent::tests`, `ops::task_execution`,
`a_run_stopped_by_a_usage_limit`, `binary(agent_tests)`, and
`documented_commands` suites plus formatting and Clippy. QA owns a built-CLI
walkthrough of the original Fable refusal and explicit `-a claude:fable` pin.
No harness decoder, transition recorder, configuration, or schema changed.

- `uv run --no-sync python scripts/test_network.py cargo test -p loopflow --lib --test agent_tests --jobs 4 model_unavailable -- --test-threads=1` (inherited `LF_*` removed, stdin closed): 3 passed, including all three provider-result scenarios; test build and fixture CLI build passed. macOS linker reported a large `__eh_frame` warning.

- `uv run --no-sync python scripts/test_network.py cargo test -p loopflow --lib --jobs 4 a_pinned_agent_refused_at_launch_fails_with_its_limit -- --test-threads=1` (same isolation): passed after pin advice was changed to name another model or provider.
- `cargo fmt --all -- --check`; `git diff --check`: passed. No test or provider process remained under this checkout's target directory.
- Compress: `uv run --no-sync python scripts/test_network.py cargo test -p loopflow --lib --test agent_tests --jobs 4 -- agent::tests:: model_unavailable --test-threads=1 < /dev/null`: 49 unit tests and the three-case provider-result test passed; test and fixture CLI builds passed with the same macOS linker warning. Gate owns the broader suites and Clippy listed above.

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

Handle a provider's model-unavailable refusal by trying the next configured
entry, keeping list order and explicit pins. Do not mark the login's quota
spent for a model-availability error. Add a regression at the provider-result
boundary and check exhaustion advice for this separate refusal kind.

Fixture scripts and raw results remain under `/tmp/lf-verify-873`:
`check.py`, `remaining.py`, `managed_retry.py`, `unavailable.py`, `results.json`.
The local build and all test commands ended. The timed-out fixture's children
were terminated; final process inspection found no remaining verification
CLI, provider, server or watcher processes. No app or release check applies
to this CLI-only, unreleased change.
