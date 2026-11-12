# LOO-850: Open the Task conversation after task run

Add `lf task run --open` using the existing launch behavior, primary Session
and `lf open` host selection. Open from the initiating CLI after launch returns;
do not wait for the next interactive step. Failed launches, preview and quote
open nothing. Plain `task run` stays unchanged; no `task start` alias.

## Current approach

`rust/loopflow/src/bin/lf.rs::run_task_and_open` runs the original invocation
without `--open`, then invokes `lf open` only on success. This stays outside
transport delegation, which replaces the process, and remote Machine dispatch.
After success, the same invocation prefixed with `--explain --json` reads
selection on the launch Machine. Its exact Task and repository IDs select local opening;
`--repo`, `--cwd`, `--wt`, and `--machine` never enter the opener.
Prefixing the preview flags handles `--` and the command's own `--json`
without reconstructing the invocation or passing parsed CLI state twice.
`task_run_open_args` carries only presentation and account options.
`checkout_task` shares checkout selection with run, checkout, hold and ship, keeping
registry failures distinct from an unbound checkout.

The Graph launcher and primary Session selection remain their existing owners.
`commands/graph.rs::carry_task_run` starts the background conversation for
non-interactive runs; `commands/open.rs` resolves its primary and host.

Re-fetched `origin/main` during realign on 2026-11-11: its tip is
`649f11db4b` (#1885, Session and Task identity in context), already merged
at `48e65c170d`; it doesn't touch `task run` or `lf open`. The newer live-Session change affects `open::available`'s
listing query, not Task selection or launch timing. The earlier idle-conversation
wake change (`baa09168df`) leaves primary selection with the existing
conversation machinery. No Wave is selected.

## Headless coverage

The two remaining coverage findings are closed in
`rust/loopflow/tests/linear/task_graph_launch_tests.rs`:

- `task_run_open_waits_for_foreground_runs` holds the Graph for both `-b` and
  `-i`, checks that nothing opens while held, then checks the successful Graph
  outcome captured at attachment by `GraphTask::open_fixture`.
- `task_run_open_successful_quote_launches_and_opens_nothing` uses a configured
  rental offer, succeeds, and checks that no attachment, Graph or rental was
  created and the Task's Graph position stayed unchanged.

The six opening tests passed after fixture consolidation. They establish the
implemented timing. No implementation gap remains from that feedback.

## Remote selection

`tests/machine_commands.rs::task_run_open_resolves_remote_selectors_before_opening_locally`
simulates execution and selection replies at SSH, then uses the actual local
opener against two repositories with the same Task alias and different Task IDs.
It does not exercise an actual remote Graph or host attachment. Selection is read
after successful launch; it adds a read round trip.

## JSON output

`task run --open --json` writes exactly the `task.run` value to stdout.
`task_run_open_args` no longer forwards `--json`; `run_task_and_open` sends the
opener's stdout to stderr when the launch has `--json`, and an opening failure
still exits non-zero.

## Remaining verification

Foreground timing follows the brief's "as soon as launch returns": `-b` and
`-i` launches return after their foreground run, so they open then. Concept
review (2026-11-11) found no remaining product choice in it; the demo shows it.

Gate: `test(task_graph_launch_tests::) | test(lf::tests::) | binary(lf) |
test(work_open_tests::) | test(documented_commands::) | test(cli_discovery::) |
test(task_initialization_tests::) | test(machine_commands::task_run_open) | binary(global_commands) |
(binary(transport_tests) & test(conversation_open_))`; clippy and affected
documentation checks remain. Formatting passed on the current Rust tree;
reuse that result unless the tree or gate's command plan changes.
Demo: configured remote execution, direct terminal opening and agent sibling-tab
opening through the real host.

## Shipping docs

`docs/work.md` covers timing, host selection and recovery; `docs/workspaces.md`
lists the entry point alongside existing conversation commands.

`lf open --host desktop --file docs/work.md --file docs/workspaces.md` reported
"No Desktop Files view attached; nothing opened."

## Focused checks

`cargo test -p loopflow --test integration task_run_open_resolves_remote_selectors_before_opening_locally`: passed after compression, 1 test including `--` (2026-11-11), simulated SSH and attachment.
`cargo test -p loopflow --lib task_graph_launch_tests::task_run_open` (with `LF_*` unset and null stdin): 6 passed after consolidating attachment fixtures (2026-11-11), including foreground ordering and the successful rental quote; macOS linker warned that `__eh_frame` exceeds 16MB.
`cargo test -p loopflow --lib --bin lf task_run_open` (with `LF_*` unset and null stdin): 7 library (6 `task_graph_launch_tests::task_run_open*`) and 2 binary tests passed after the one-JSON-document change, including stdout parsing as one value (2026-11-11).
`cargo fmt` and `cargo clippy --all-targets -- -D warnings`: passed after compressing `run_task_and_open`'s parameter destructuring; `cargo test -p loopflow --bin lf task_run_open`: 2 passed (2026-11-11).

Earlier preview fixtures expected success for refused invocations: `--context`
reported "--context requires a skill, Graph or inline agent request; use --explain
to inspect Work without launch"; bare `--quote` reported "--quote needs --pool or
--rent". Those expectations were corrected before the recorded checks. The remote
fixture corrections added the SSH version probe and removed invalid combined
selectors; root `--json` needs preview, and `--cwd` conflicts with `--wt`.
