# LOO-850: Open the Task conversation after task run

Add `lf task run --open` using the existing launch behavior, primary Session
and `lf open` host selection. Open from the initiating CLI after launch returns;
do not wait for the next interactive step. Failed launches, preview and quote
open nothing. Plain `task run` stays unchanged; no `task start` alias.

## Shipping docs

The intended behavior is documented in:

- `docs/work.md`: invocation, opening timing, host selection and recovery.
- `docs/workspaces.md`: entry point alongside existing conversation commands.

`lf open --host desktop --file docs/work.md --file docs/workspaces.md` reported
"No Desktop Files view attached; nothing opened."

## Delete — do not maintain

None: this adds an opt-in composition of the existing run and open commands.
The Graph launcher and primary Session selection remain their existing owners.

`rust/loopflow/src/bin/lf.rs::checkout_task` shares checkout selection across
run, open and hold, keeping registry failures distinct from an unbound checkout.
Opening remains a child CLI invocation because transport delegation replaces
the running process and remote execution must leave presentation local.

## Remaining verification

Gate: `test(task_graph_launch_tests::) | test(lf::tests::) | binary(lf) |
test(work_open_tests::) | test(documented_commands::) | test(cli_discovery::) |
test(task_initialization_tests::)`;
formatting, clippy and affected documentation checks are owned by gate.

`cargo test -p loopflow --lib task_run_open`: passed, 5 tests (2026-11-11);
macOS linker warned that `__eh_frame` exceeds 16MB.
`cargo test -p loopflow --bin lf task_run_open`: passed, 2 tests (2026-11-11).
`cargo fmt` and `git diff --check`: passed.

Compress's first two focused runs each passed 4 tests and failed 1 on fixture
expectations. `--context` reported "--context requires a skill, Graph or inline
agent request; use --explain to inspect Work without launch"; `--quote` reported
"--quote needs --pool or --rent". Both cases now expect refusal and no opening.
