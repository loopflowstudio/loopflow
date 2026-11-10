# LOO-810 creation recovery · reconciled 2026-11-09

The late-creation repair is implemented in `e36683e10b`, retained unchanged by
the sync merge `c37818e89`. The focused Desktop build and tests passed. Earlier
feedback describing the defect as still present predates this repair.

## Contract and implemented repair

Jack Heart: "Persist only immutable submitted creation requests: request ID,
original destination, exact versioned payload and submission time; server
receipts own outcomes." Also: "No editable outbox, generic replay or client
milestone ledger."

Creation receipts and recovery registration remain unchanged. `createTask`
captures its original WorkNavigation, palette presentation generation and
destination generation before submitting. Only that presentation closes on
success or unknown outcome. Opening also requires unchanged navigation,
including trips away and back to the same destination or repository. A newer
sheet at the same destination is a different presentation. Ordinary creation
still opens its conversation; a hidden original sheet closes without touching
the new repository's sheet. Unconditional dismissal/navigation are removed.

## Remaining acceptance

- Gate: affected Swift suites beyond the five focused tests below; reuse prior
  results only for unchanged source and command plans.
- Mounted cold-app recovery: lose an admitted creation answer, restart Desktop,
  and show the same Task/Session and first prompt without duplicate launch or
  prompt. Verify usable Inspect/Show result and clear the submission only after
  presentation. Confirm later navigation and a newer draft survive a late reply.
- Configured-provider sleep/wake: Task, terminal and conversation return without
  lost/repeated input or duplicate launch. Compose with LOO-812's silent writable
  bridge and healthy quiet provider checks, retaining owner/input/geometry through
  expiry and replacement. Headless protocol coverage does not establish this.
- Publish/review the repair separately from merged #1854. No demo occurred in
  the headless run; no new product choice is needed for the implemented repair.

## Concept review · 2026-11-09

Outcome for loop-or-next: nothing to change and no open product choice; advance.
The repair implements the existing requirement that a late creation must not
change later navigation or erase a newer draft. Presentation generations are
ephemeral UI state, not another creation record or outcome ledger.

Reviewed the normal reply, lost reply, cold recovery and navigation-away paths
against the C design retained at
`1d5e6ada:scratch/loo-810-server-contracts.md`. The decisive counterexamples are
leaving and returning to the same destination, and reopening New Task there:
equal destinations do not make the earlier presentation current again. The
implemented repair and its 14 regression cases account for both. Result
registration remains independent of opening, so declining late navigation does
not discard the submitted request.

No runtime checks were repeated in this review. The recorded focused checks
support the repair; mounted cold recovery, draft retention and configured-provider
sleep/wake still need the acceptance above. Review page: `concept-review.html`.

Check: `scripts/test_desktop.sh --jobs 4 --filter 'WorkDestinationTests/(lateCreationReplyKeepsNewerNavigation|newTaskSelectsTheTaskAndOpensItsConversation|newTaskSelectsBeforeReadingsConfirm|submittedCreationHasNoSecondSubmitButton|newTaskFailureBeforeFilingKeepsTheSheet)'` — Desktop build and 5 tests passed, including all 14 late-reply cases; log `/tmp/loo-810-creation-repair-swift.log`. Inherited `LF_*` and `GHOSTTY_RESOURCES_DIR` unset, stdin closed. An intermediate build rejected a test file edited during compilation; the final unchanged-source run exited 0. Broader Swift checks belong to gate; physical sleep/wake and mounted cold recovery remain unverified.

## Source and preceding progress

PR #1854 merged as `7cf2d25b3f01de2e7148c0d15b1dbe2d2ae445c8`.
At verification, `git fetch origin main` still returned that SHA. The retained
checkout and main both had tree `c266f0e2fd7e72e799f2ca51ea18bb345551dc9a`;
verification built those merged source bytes in the existing Task checkout.
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

The 14 repaired regression cases cross success/unknown outcome with newer Task
sheet, newer sheet at the same destination, newer repository sheet, navigation
away, navigation round trip, dismissal and repository round trip. Successful
replies remain registered for presentation even when opening is suppressed.

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

These CI observations concern #1854, not the later repair. No physical sleep or
installed-app trial ran. The original failure remains recorded above; the repair's
passing focused check does not replace broader gate or mounted acceptance.

## Live QA · 2026-11-09 after the repair

`qa.html` records this run against c37818e89. Branch CLI and Desktop builds
passed. Real CLI/socket creation, concurrent identical requests, lost answers,
receipt lookup, conflict refusal and server restart passed in a private Machine.
A run=true creation retained one completed Graph, one primary Session and one
provider user message containing its original request after restart and retry.
Session new-thread preparation also retained its identity after a lost answer.
A 100-read burst yielded 2 responses and 98 retained-byte refusals; after drain,
open and retained_bytes were zero. Real shell reconnection kept the newer
viewer's input and 31×99 geometry; 61.4-second expiry refused replay before bytes,
then returned a fresh screen without input authority. All QA processes stopped.

Mounted acceptance remains open: both isolated app launches returned
`cgWindowNotFound`; the short-path app's `lf desktop state --json` reported
`windows: []`. The initial long macOS temporary path exceeded Desktop's Unix
socket path limit. No mounted draft/cold-recovery or physical sleep/wake claim
follows from these checks. Updated `.lf/skills/qa.md` with the working short-path
setup and how to record empty-window results. No production repair in this step.
