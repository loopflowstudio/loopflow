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
`checkout_task` shares checkout selection with run, hold and ship, keeping
registry failures distinct from an unbound checkout.

The Graph launcher and primary Session selection remain their existing owners.
`commands/graph.rs::carry_task_run` starts the background conversation for
non-interactive runs; `commands/open.rs` resolves its primary and host.
The foreground timing assumption remains in `scratch/questions.md`.

Fetched `origin/main` on 2026-11-11: `baa09168df` is already in this branch.
Its idle-conversation wake change leaves primary selection with the existing
conversation machinery. No Wave is selected.

## Delete — do not maintain

No remaining deletion targets. The wrapper's local omitted-Task lookup and
filesystem-selector forwarding are gone, along with the test expecting remote
cwd forwarding. `checkout_task` remains shared by ordinary run/hold/ship dispatch.

## Remaining verification

The remote regression simulates execution and selection replies at SSH, then
uses the actual local opener against two repositories with the same Task alias.
It covers remote-only filesystem selectors, omitted Tasks, JSON flags, `--`,
primary reuse and failed selection reads. Configured remote execution remains a demo
check. Selection is read after successful launch; it adds a read round trip.

Gate: `test(task_graph_launch_tests::) | test(lf::tests::) | binary(lf) |
test(work_open_tests::) | test(documented_commands::) | test(cli_discovery::) |
test(task_initialization_tests::) | test(machine_commands::task_run_open) | binary(global_commands) |
(binary(transport_tests) & test(conversation_open_))`; formatting, clippy and
affected documentation checks remain. Cover foreground `-b`/`-i` opening order and a valid pool/rent quote;
the focused fixtures cover bare runs, waiting Tasks and invalid quote refusal.
Demo: direct terminal and agent sibling-tab opening through the real host.

## Shipping docs

The intended behavior is documented in:

- `docs/work.md`: invocation, opening timing, host selection and recovery.
- `docs/workspaces.md`: entry point alongside existing conversation commands.

`lf open --host desktop --file docs/work.md --file docs/workspaces.md` reported
"No Desktop Files view attached; nothing opened."

## Focused checks

`cargo test -p loopflow --test integration task_run_open_resolves_remote_selectors_before_opening_locally`: passed after compression, 1 test including `--` (2026-11-11), simulated SSH and attachment.
`cargo test -p loopflow --lib --bin lf task_run_open`: passed after compression, 5 library and 2 binary tests including local `--json` and `--` (2026-11-11); macOS linker warned that `__eh_frame` exceeds 16MB.
`cargo fmt --check` and `git diff --check`: passed.

Earlier preview fixtures expected success for refused invocations: `--context`
reported "--context requires a skill, Graph or inline agent request; use --explain
to inspect Work without launch"; bare `--quote` reported "--quote needs --pool or
--rent". Those expectations were corrected before this pass. This pass's remote
fixture corrections added the SSH version probe and removed invalid combined
selectors; root `--json` needs preview, and `--cwd` conflicts with `--wt`.
