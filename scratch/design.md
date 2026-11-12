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
The foreground timing assumption remains in `scratch/questions.md`.

Fetched `origin/main` on 2026-11-11: its tip is `4dc15b7302`, already merged
at `b7bed56e4b`. The newer live-Session change affects `open::available`'s
listing query, not Task selection or launch timing. The earlier idle-conversation
wake change (`baa09168df`) leaves primary selection with the existing
conversation machinery. No Wave is selected.

## Delete — do not maintain

No deletion targets remain. The local omitted-Task lookup and filesystem-selector
forwarding are gone; selection stays on the launch Machine. The foreground-only
attachment wrapper is also removed: `GraphTask::open_fixture` captures Graph
outcomes at attachment, so the foreground test reads that snapshot directly.

## Remote selection

`tests/machine_commands.rs::task_run_open_resolves_remote_selectors_before_opening_locally`
simulates execution and selection replies at SSH, then uses the actual local
opener against two repositories with the same Task alias and different Task IDs.
It does not exercise an actual remote Graph or host attachment. Selection is read
after successful launch; it adds a read round trip.

## Remaining verification

Foreground timing remains an assumption for demo, not a recorded product
decision; `scratch/questions.md` retains it.

Gate: `test(task_graph_launch_tests::) | test(lf::tests::) | binary(lf) |
test(work_open_tests::) | test(documented_commands::) | test(cli_discovery::) |
test(task_initialization_tests::) | test(machine_commands::task_run_open) | binary(global_commands) |
(binary(transport_tests) & test(conversation_open_))`; formatting, clippy and
affected documentation checks remain for the merged tree.
Demo: configured remote execution, direct terminal opening and agent sibling-tab
opening through the real host.

## Shipping docs

`docs/work.md` covers timing, host selection and recovery; `docs/workspaces.md`
lists the entry point alongside existing conversation commands.

`lf open --host desktop --file docs/work.md --file docs/workspaces.md` reported
"No Desktop Files view attached; nothing opened."

## Focused checks

`cargo test -p loopflow --test integration task_run_open_resolves_remote_selectors_before_opening_locally`: passed after compression, 1 test including `--` (2026-11-11), simulated SSH and attachment.
`cargo test -p loopflow --lib --bin lf task_run_open`: passed after compression, 5 library and 2 binary tests including local `--json` and `--` (2026-11-11); macOS linker warned that `__eh_frame` exceeds 16MB.
`cargo test -p loopflow --lib task_graph_launch_tests::task_run_open`: 5 passed, including the successful rental quote; the new foreground test failed because step output was absent from the caller log (2026-11-11). Its corrected fixture inspects recorded Graph outcomes at attachment.
`cargo test -p loopflow --lib task_graph_launch_tests::task_run_open_waits_for_foreground_runs`: passed for both `-b` and `-i` (2026-11-11); same macOS linker warning. The other five tests are unchanged since their pass.
`cargo test -p loopflow --lib task_graph_launch_tests::task_run_open` (with `LF_*` unset and null stdin): 6 passed after consolidating attachment fixtures (2026-11-11), including foreground ordering and the successful rental quote; same linker warning.
`cargo fmt --check` and `git diff --check`: passed.

Earlier preview fixtures expected success for refused invocations: `--context`
reported "--context requires a skill, Graph or inline agent request; use --explain
to inspect Work without launch"; bare `--quote` reported "--quote needs --pool or
--rent". Those expectations were corrected before the recorded checks. The remote
fixture corrections added the SSH version probe and removed invalid combined
selectors; root `--json` needs preview, and `--cwd` conflicts with `--wt`.
