# MTG-344 launch recovery

2026-11-14 UTC. Installed lf: 0.15.3+9206f321d.

Reported observation: “Every launch immediately fails with invalid control authority: Previous AgentProcess has no observed exit.” Report named interrupted design step `1377bca0` and said the rewrite had not started.

Before recovery:
- Task checkout clean, HEAD `a3b0967f8815b289425163896de109f5ef495c8d`.
- Design LfProcess `1377bca0-68a9-46df-8592-9552790385a3` and provider `fe767bb9-d072-4597-a2cf-ce2b88c0ba1e` both completed interrupted at epoch 1794626249; provider spawn_state=exited. PIDs 28682/29143 absent.
- Primary conversation `31169d1dbbad45c0897e441845903793`: provider `bc3d47e4-e8b7-47d9-9d7e-99453adc5cce`, PID 82436, birth 1794629948. OS ps confirms Codex alive. Leave it alone.
- Latest failed Graph `f5175b72-a722-4831-840e-54cfddb317f8` logs both Claude fable accounts cooling, then “Continuing on codex:gpt-6.1-sol”, then the authority error. Failed agent reservation `04997915-c61a-4868-813f-7f38dc96a29e` has no PID and is completed spawn_failed.
- `lf process prune --dry-run --json` offers no settlement for these already closed design/failed agents. No prune applied.
- Task lookup from this Loopflow checkout is repository-scoped and fails; use `lf --repo /Users/jack/src/manabot ...`.

Private pre-change row snapshots and failed-launch log: `/Users/jack/.lf/incident-backups/mtg-344-launch-20261114T051640Z`. No process rows modified.

## Recovery and validation

Changed only the personal repository-local preference `/Users/jack/src/manabot/lf.config.local.yaml`, creating:

```yaml
skills:
  write-docs:
    agent: codex:gpt-6.1-sol
```

Command: `lf --repo /Users/jack/src/manabot config set skill.write-docs.agent codex:gpt-6.1-sol --json`. This selects Codex directly for write-docs in manabot, bypassing the failing pre-spawn Claude-to-Codex fallback. It affects other write-docs launches in that repository until removed. Undo with `lf --repo /Users/jack/src/manabot config unset skill.write-docs.agent`. No global preference, process record, source code, Task branch, or PR edited. The config file was absent before recovery.

Validation: `lf --repo /Users/jack/src/manabot task run MTG-344 --from write-docs --json` registered Graph `6be75d3c-5c3f-46d9-8a31-2a4a74e7eb59`, reached write-docs, and launched Codex PID 24020 (birth 1794633414), AgentProcess `5d18f55c-fd7b-4452-afed-8cd7a52dca23`, Session `session_c2bd360b03394769a5645197d6c890f0`. `ps` confirmed the provider. No cooling/fallback/authority error appeared.

Stopped validation with `lf --repo /Users/jack/src/manabot task interrupt MTG-344 --json` at epoch 1794633424, before continuing the design. Provider and step recorded interrupted, provider spawn_state=exited; Graph recorded failed with status 130. PIDs 22871/23703/24020 gone. Task is idle, next command `lf task run MTG-344`; no run left in the background. `task wait` did not return after interruption, so its own wait process was terminated (SIGTERM); Task status and process/OS checks verified shutdown instead.

Post-check: Task worktree still clean and HEAD still `a3b0967f8815b289425163896de109f5ef495c8d`. This validates startup only; design/rewrite completion was not attempted. A prior attempt to add `--directive` to the existing Task returned “Task MTG-344 already exists; use lf task steer …” and launched nothing.

## Initial debug hypothesis (before reproduction)

Reproduce an account-selection refusal before any provider spawn, followed by cross-provider fallback, using isolated fixture accounts/store. Inspect `_run_with_transient_retries` / `_run_harness_once` in `rust/loopflow/src/agent/mod.rs`, `ProcessRuntime::retry_agent_process`, and `SqliteStore::replace_session_agent_process`. Current source calls replacement on retry; replacement requires completed_at or a positive `close()` result, whereas `close_session_agent_process` returns false for a reservation with no PID/birth. Unlike `prepare_session_agent_process` / `with_session_resume`, replacement does not admit a never-spawned reserved record. This is a likely cause of the observed fresh-launch failure, not confirmed by a failing fixture yet. Check the installed build `9206f321d` as well as this checkout. Do not loosen death checks for a spawn_requested record with uncertain liveness.

The original interrupted design process already has an exit; changing it would not repair this pre-spawn fallback path. Claude account capacity remains unavailable until the logged reset times (2026-11-17/18 for fable). Keep the local override until fallback is fixed, then remove it and verify the ordinary order.

## Root-cause debug, 2026-11-14 UTC

Isolated reproduction added in `rust/loopflow/tests/agent_tests.rs`:
`pre_spawn_account_refusal_falls_through_to_codex`. Two synthetic Claude
accounts are connected but cooling; the launch order is `claude:fable`, then
`codex:gpt-6.1-sol`. Providers are stand-ins, with separate native homes and an
ephemeral store. No manabot command or rewrite is involved.

Before the fix, the real `run_agent` path printed:

```text
Continuing on codex:gpt-6.1-sol: failed to select provider account: no eligible managed claude account: 'first@example.com' cooling until 2026-11-14T06:21:55Z, 'second@example.com' cooling until 2026-11-14T06:21:55Z
ExecutionFailed("invalid control authority: Previous AgentProcess has no observed exit")
```

The test failed before reaching the Codex stand-in. The reported duration
(65.23s) includes a nested fixture CLI build, not a capacity wait.

Confirmed cause: account refusal happens before `record_agent_process_launch`,
so the invocation still owns a `reserved` AgentProcess without PID/birth.
The fallback loop marks a different agent as a retry, and the runtime calls
`replace_session_agent_process`. That function discarded the existing progress
helper's `reserved` answer, required exit/close, and the close function correctly
returned false without a PID/birth. This rejected a reservation as though it
were a potentially running provider. The same replacement implementation is
present in installed-build source `9206f321d`. Process `1377bca0` is unrelated to
this transition and needs no repair.

The targeted change admits only the existing `reserved AND pid IS NULL` state
alongside observed exits. Retiring that reservation sets completion without
changing its spawn state to `exited`. Uncertain `spawn_requested`, legacy
unknown state, and running processes still require observed exit or successful
shutdown; attachment authority and thread/history updates stay in the same
existing transfer lock and transaction. Resume already used this distinction;
retry had omitted it.

For 5whys: the adjacent fallthrough tests scripted `AgentAttempt` results and
therefore did not exercise reservation replacement. The existing replacement
regression starts with `record_agent_process_launch` (an uncertain spawn), so
its exit requirement is appropriate. The new regression crosses the missing
account-selection → fallback → runtime/store transition with real routing and
stand-in provider startup. The initial hypothesis that the interrupted design
needed settlement is contradicted by its recorded exit and by reproduction in
a fresh store with no interrupted design.

Next incident steps for launch-plan: ship/install the patched lf before removing
the manabot-local override. Then unset `skill.write-docs.agent` and validate the
ordinary configured fallback order with a bounded no-edit startup probe; stop
and check the probe's exit before any design/rewrite work continues. Keep the
override while the installed binary still has this bug. This debug run does not
change the override, production process rows, or MTG-344 Task state, and does
not launch the rewrite.

Validation (debug run; all `LF_*` variables cleared for Cargo tests, fixture
`LOOPFLOW_TEST_LF` pinned to this checkout's `target/debug/lf` after its build):

- `cargo test -p loopflow --test agent_tests pre_spawn_account_refusal_falls_through_to_codex`: failed with the exact authority error before the fix; passed afterward (1 test, 2.73s).
- `cargo test -p loopflow --test agent_tests`: passed, 20 tests (54.51s); debug run.
- `cargo test -p loopflow --lib store::sqlite::processes::discovery_tests::`: passed, 9 tests; debug run.
- `cargo test -p loopflow --lib invocation_`: passed, 4 tests, including actual throwaway-group shutdown and failed-close/stale-authority rejection; debug run.
- `cargo test -p loopflow --lib store::sqlite::processes::attachment_tests::`: passed, 10 tests, including uncertain-spawn preservation; debug run.
- `cargo fmt --check` and `git diff --check`: passed; debug run.
- `cargo clippy --all-targets -- -D warnings`: passed; debug run.

Test builds emitted the macOS linker warning `__eh_frame section too large
(max 16MB) to encode dwarf unwind offsets in compact unwind table`; test
execution succeeded. CI owns the full matrix; installed-binary validation and
override removal remain for launch-plan after deployment.

The reproduction and all checks have exited. No rewrite, Task Graph, server, or
watcher was started by this debug run. Source changes are local to this checkout;
the installed lf and the manabot override are unchanged.

## 5whys findings, 2026-11-14 UTC

Status: MTG-344's write-docs startup was blocked; launch recovery is validated
through the manabot-local override. The fallback defect is fixed in this
checkout, not in the installed binary. This run reconfirmed installed version
`0.15.3+9206f321d` and the override above. Rewrite completion remains outside
this incident. Other launches with the same pre-spawn refusal and fallback
transition are exposed; their actual number is unknown.

### Causal chain

1. Both configured Claude accounts were cooling. The captured production log
   shows the intended move to Codex followed by the authority error. Capacity
   exhaustion triggered fallback; it does not explain why available Codex
   could not start.
2. The first account refusal left an invocation-owned reservation, not a
   launched provider. Changing the selected agent made the next attempt a
   retry even though no provider had started. The fresh-store reproduction
   failed with the same error before the patch, excluding interrupted design
   history as a necessary cause.
3. Retry replacement required an exit or successful shutdown for that
   reservation. Shutdown correctly refused to claim death without OS identity.
   Replacement had discarded the shared progress helper's never-spawned answer;
   resume and spawn preparation already used it. This was a mismatch between
   existing lifecycle operations, not missing process settlement or a schema
   incapable of representing the state.
4. Existing routing tests supplied attempt outcomes instead of running account
   selection through runtime replacement. Existing replacement coverage began
   after a spawn request. Each tested its own behavior; neither crossed the
   transition that failed here. The new regression crosses it and checks both
   successful fallback and truthful lifecycle records.

No further organizational or prompt cause is established. Nothing in the
available facts explains why these accounts exhausted capacity, nor is that
needed to prevent this launch failure. Account cooling is a supported input.

### Prevention and launch-plan handoff

Already made: the replacement fix uses the existing never-spawned distinction;
no migration or process-row repair is needed. The regression, adjacent safety
tests, and short harness documentation cover the changed contract. This run
independently passed `cargo test -p loopflow --test agent_tests
pre_spawn_account_refusal_falls_through_to_codex` (1 test, 2.23s, LF variables
cleared and branch fixture binary pinned) and `git diff --check`. The same macOS
linker warning noted above appeared. Prior full agent/focused process tests and
Clippy results remain the debug run's results; they were not rerun here.

Next useful action for launch-plan: ship/install this patch, then remove the
repository-local override and validate the ordinary configured fallback with a
bounded startup probe. Confirm that fallback was actually exercised: successful
direct Claude startup after cooldown would not validate Claude-to-Codex recovery.
Stop the probe and verify provider/Graph shutdown and unchanged Task files/HEAD
before any rewrite work. Keep the override until the installed patch is ready.
This is the remaining deployment check, not a claim that production fallback is
already repaired.

Proposals only: use this regression's pre-spawn-refusal case when reviewing
future routing/lifecycle changes. No broader state-machine redesign, new guard,
or prompt change is justified by this incident: the model already expresses
reservation versus uncertain spawn, and the patch restores that distinction.

Open branch: the recovery run's `task wait` did not return after interruption.
Its waiter was terminated and shutdown verified separately. This run did not
reproduce that wait behavior; there is no basis to attribute it to reservation
replacement. Investigate separately if reliable interrupted-run waiting is
needed for the deployment probe. The interrupted design's recorded exit and
fresh-store reproduction contradict the initial stale-design explanation.

This 5whys run changed only this note, left the source fix and other changes
intact, and started no Task, rewrite, server, or watcher. Its test has exited.
Next action: launch-plan using `scratch/mtg-344-launch-recovery.md`.

## Execution decision, 2026-11-14 UTC

Keep the patch, regression and harness documentation together in this checkout.
The useful core is complete: pre-spawn account refusal can fall through to
another provider, reservations are retired truthfully, and uncertain/live spawns
retain their exit requirement. Continue this incident's delivery path here; no
new Task, Wave, competing Graph or state-machine redesign is needed. Launch
planning does not publish, merge, install or run the rewrite.

Current reads: MTG-344 remains idle after status 130, with all its Graphs stopped
or completed. Its checkout is clean at
`a3b0967f8815b289425163896de109f5ef495c8d`. Installed lf remains
`0.15.3+9206f321d`; the manabot-local Codex override is still selected.

Open PR overlap to reconcile during delivery:

- [#1931 · Revert interrupted Task moves](https://github.com/loopflowstudio/loopflow/pull/1931)
  touches `processes.rs`; no Task is linked in its description. Its stated
  merge-revert versus close-and-fix-forward choice remains with its maintainers.
- [#1904 · Run headless Work without per-Session owners](https://github.com/loopflowstudio/loopflow/pull/1904),
  Task LOO-858, touches all three changed files. Retain its launch/holder model
  when integrating this small retry correction; do not adopt its delivery work.
- [#1862 · Recover open Sessions after usage limits](https://github.com/loopflowstudio/loopflow/pull/1862),
  Task LOO-811, touches the harness README. Keep interactive recovery separate
  from this fresh-launch fallback fix. No additional product choice is needed
  for the reservation correction.

Delivery sequence and completion criteria (carry these into the PR body before
scratch cleanup):

1. Publish this single patch through the incident's delivery steps, reconcile
   overlapping changes, and let CI run the full matrix. Existing focused test,
   formatting and Clippy results are recorded above; rerun affected tests if
   integration changes the implementation. The PR must retain the cooling-account
   regression and uncertain-spawn safety coverage. Land through normal delivery.
2. Wait for a green published main artifact that contains the landed commit,
   then use `lf install --dev` on this Machine. Verify the installed revision
   contains the patch and new launches select it. Do not install the current
   newest green artifact merely because it is newer. `scripts/install.py local`
   builds an experimental CLI; it does not promote the managed-launch entry.
   Keep the override throughout shipping and installation. Do not roll over,
   resume or terminate unrelated Sessions to validate this fix.
3. Prepare a disposable, Task-free no-edit startup probe using the installed
   CLI and the ordinary manabot write-docs routing resolved without its temporary
   agent override. Use an empty disposable checkout and a no-op prompt in place
   of write-docs content; do not run `task run MTG-344` or load its design. Copy
   only routing preferences, never credentials, and keep real account capacity
   records unchanged. Capture the resolved provider order and compare it with
   manabot's ordinary order before launching. The probe must exercise actual
   Claude account refusal followed by Codex startup, not direct Codex selection.
   Bound the whole attempt to 60 seconds and stop its own process group on
   deadline; stop immediately once startup is observed. Do not use `task wait`.
4. Verify the probe's own provider/lf processes have exited using their recorded
   OS identities and process records. Require no authority error, completed
   never-spawned reservation without a fabricated exit, and observed exit of the
   launched provider. Recheck MTG-344's unchanged HEAD/files and idle state.
   If accounts no longer refuse, direct Claude success is inconclusive: keep
   the override and defer ordinary-fallback acceptance rather than manufacture
   production cooldowns. A disposable synthetic-account test can supplement,
   but cannot replace, this ordinary-routing observation.
5. Only after successful fallback and shutdown validation, run
   `lf --repo /Users/jack/src/manabot config unset skill.write-docs.agent`.
   Check effective write-docs routing now matches the probed ordinary order.
   If removal or that check fails, restore the exact Codex override. Record the
   installed revision, fallback observation, shutdown and final config result.
   Installation or probe failure leaves the workaround in place.

Done when the patch is shipped and selected by installed launches, ordinary
fallback has been observed with clean probe shutdown, and the temporary override
is removed. MTG-344 remains idle; rewrite/design continuation is separate work.
The independent interrupted `task wait` hang stays a named investigation here;
it is neither part of this patch nor required for the bounded probe. No follow-up
Task is launched from this planning run. All commands in this run have exited.
