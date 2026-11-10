# LOO-810 merged recovery verification · 2026-11-09

**Fix forward.** Verification found that an earlier successful New Task reply
changed the current selection and closed a newer New Task sheet; an
unknown-outcome reply also closed the newer sheet.

## Repair design · 2026-11-09

Keep creation receipts and recovery registration unchanged. Capture the original
WorkNavigation and its palette presentation generation before submitting. Only
that presentation may close on success or unknown outcome. Reuse WorkModel's
destination generation to suppress opening after navigation, including leaving
and returning to the same destination or repository. A newer sheet at the same
destination is a new presentation. Ordinary creation still opens its conversation.

**Delete — do not maintain:** unconditional current-palette dismissal and
unconditional navigation in `WorkModel.createTask`; no replacement outcome ledger,
retry owner or persistence format. Preserve recovery registration and confirmation.

Focused acceptance: successful/unknown replies after navigation, return,
dismissal and a newer sheet at the same destination; normal creation, preparation
and storage refusal remain covered by WorkDestinationTests. Gate owns broader
Swift tests and mounted acceptance remains unverified.

Check: `scripts/test_desktop.sh --jobs 4 --filter 'WorkDestinationTests/(lateCreationReplyKeepsNewerNavigation|newTaskSelectsTheTaskAndOpensItsConversation|newTaskSelectsBeforeReadingsConfirm|submittedCreationHasNoSecondSubmitButton|newTaskFailureBeforeFilingKeepsTheSheet)'` — Desktop build and 5 tests passed, including all 14 late-reply cases; log `/tmp/loo-810-creation-repair-swift.log`. Inherited `LF_*` and `GHOSTTY_RESOURCES_DIR` unset, stdin closed. An intermediate build rejected a test file edited during compilation; the final unchanged-source run exited 0. Broader Swift checks belong to gate; physical sleep/wake and mounted cold recovery remain unverified.

## Source and preceding progress

PR #1854 merged as `7cf2d25b3f01de2e7148c0d15b1dbe2d2ae445c8`.
`git fetch origin main` still returned that SHA. The retained checkout's HEAD
and main both have tree `c266f0e2fd7e72e799f2ca51ea18bb345551dc9a`;
verification built these merged source bytes in the existing Task checkout.
Only the new regression test differed from main during verification. That pass
included no production repair.

The preceding work intended restart-safe creation, bounded output, terminal
recovery, shared protocol routing and provider transitions. Its publication
checks at `1d5e6ada:scratch/loo-810-publication.md` show completed sync,
documented-command checks, regenerated website routes, and all five terminal
cases. That is progress. The merged PR still says mounted cold-app recovery and
physical sleep/wake with a configured provider are unverified; their absence is
an acceptance gap, not evidence that the implementation made no progress.

## Reproduced failure

Added `WorkDestinationTests.lateCreationReplyKeepsNewerNavigation`, with success
and unknown-outcome cases. It suspends the real WorkModel's creation call at the
query boundary, closes its original sheet, selects another Task, opens a new
New Task sheet, then delivers the earlier answer. The success case also checks
that the original result remains registered for presentation.

Command (inherited `LF_*` and `GHOSTTY_RESOURCES_DIR` unset, stdin closed):

```sh
scripts/test_desktop.sh --jobs 4 --filter 'CreationSubmissionTests|ProviderTransitionTests|WorkDestinationTests/lateCreationReplyKeepsNewerNavigation'
```

Desktop and its tests built. The nine existing creation-recovery and transition
tests passed; the new parameterized test failed with three assertions:

```text
unknownOutcome → false:
model.selection → task "issue-now", expected task "issue-review"
model.navigation.palette → nil, expected .newTask
unknownOutcome → true:
model.navigation.palette → nil, expected .newTask
Test run with 10 tests in 3 suites failed ... with 3 issues.
```

Full log: `/tmp/loo-810-verify-swift.log`.

The merged `WorkModel.createTask` cleared `navigation.palette` and opened the created Task
after awaiting the reply, without tying either action to the initiating
presentation/navigation. Its `SubmittedCreationError` handler also cleared the
current palette. `NewTaskSheet` keeps its request in local view state and leaves
Cancel enabled during submission, so dismissing a newer sheet loses that draft.

The repair retains the original result/recovery registration, but
applies navigation and dismissal only to the creation's originating presentation
when it is still current. Regression cases cover success, unknown outcome,
navigation away and return, and a newer sheet opened at the same destination
or another repository. The original submitted form closes even when its
repository is hidden, leaving the new repository's sheet intact. Existing
navigation ownership remains; no outcome ledger or mutation replay is added.
Normal creation still opens its result. Publication remains a separate step
in this Task, retaining #1854 as history.

## Other merged checks

- `cargo build -p loopflow --bin lf --jobs 4`: passed, `Finished dev ... in 1m 27s`.
- Built `target/debug/lf server describe --json` with a temporary `HOME` and
  `LF_HOME`, all inherited `LF_*` unset: parsed JSON equals the committed schema.
- Built `target/debug/lf server status` in that isolated Home: exit 0,
  `ready:false`, `listening:false`, `instance:null`, `revision:null`, `clients:[]`.
  It started no server. Initial probe corrections: `status --json` is invalid
  (exit 2); an absent LF_HOME directory gave ENOENT (exit 1). Creating the empty
  directory and using the documented command produced the reading above.
- `git diff --check`: passed.

Rust verification runs behind `scripts/test_network.py`, with inherited `LF_*`
unset, `GHOSTTY_RESOURCES_DIR` unset, closed stdin, and the built candidate pinned
as `LF_BIN` inside network isolation:

```sh
uv run python scripts/test_network.py env LF_BIN="$PWD/target/debug/lf" \
  cargo nextest run -p loopflow --lib --test served_reads --test transport_tests \
  --test integration --build-jobs 4 --test-threads 2 --no-fail-fast \
  -E 'test(server::) | test(store::sqlite::provider_transitions::) | test(documented_commands::) | test(served_task_create_survives_a_dropped_connection_and_server_restart) | test(served_session_new_recovers_its_input_after_a_lost_reply_and_restart) | test(server_operations_match_their_lf_commands) | test(server_reconnect_preserves_newer_viewers_input_and_geometry) | test(grace_expiry_refuses_bytes_before_repainting_the_terminal_screen) | test(exact_owner_refuses_a_replacement_while_session_attach_finds_it) | test(a_stalled_bridge_leaves_the_healthy_bridge_and_control_checkpoint_live) | test(resume_supersedes_its_open_socket_without_taking_a_newer_viewers_claims)'
```

Log: `/tmp/loo-810-verify-rust.log`. Result: `45 tests run: 45 passed
(1 slow), 3258 skipped`, in 103.982 seconds. All five terminal cases passed;
the production retention-expiry test took 64.395 seconds, including its
intentional 61-second wait. Budget/refusal/checkpoint, restart-safe Task/Session
creation, provider lineage, CLI/server parity and all four documented-command
checks passed. Both local test runners exited; their fixture servers stopped.

## Exact merge CI

`gh run list --repo loopflowstudio/loopflow --commit 7cf2d25b3f01de2e7148c0d15b1dbe2d2ae445c8`
observed these separately:

- **push, CI:** [42733784226](https://github.com/loopflowstudio/loopflow/actions/runs/42733784226), success.
- **push, macOS:** [42733784308](https://github.com/loopflowstudio/loopflow/actions/runs/42733784308), in progress at inspection. Both process-test jobs passed; Swift and UI compile jobs were still running.
- **merge_group, CI:** [42731866815](https://github.com/loopflowstudio/loopflow/actions/runs/42731866815), success. This is separate from the push results.

No physical sleep or installed-app trial ran. The focused failure warrants
iteration without waiting for a person; broader mounted acceptance remains with
the repaired change.
