# Resume and title gate

Scope: provider departure retains ordinary Session identity/history without changing
input concurrency checks; bare Sessions avoid `default`; serialized OSC updates
survive provider titles and reattachment, retaining cmux/tmux controls. No install,
publication or landing watcher in this run. Parent handles landing.

Reconciled via `lf sync --manual` / `lf sync --continue` against main
`1c757e9fe2028c0073414dc6826395931a5aa743`. Kept typed AgentProcess IDs,
asynchronous exit settlement and main's directory reporting. The upstream
config migration patch has no branch diff.

Changed-path plan:
- `store/sqlite/processes.rs`, `harness/agent_process.rs`: attachment/process,
  Session replacement/retirement and runtime tests; Session launch/CLI contracts.
- `naming.rs`, `terminal_title.rs`: naming and host policy unit tests.
- `ghostty_vt.rs`, `transport.rs`, `transport/output.rs`: VT/output/transport
  unit tests, transport integration binary (retention, directory, viewer, exit),
  startup cost contract and interactive Graph boundary.
- `docs/surfaces.md`, transport README: architecture and website checks.
- Changed-aware runner additionally selects headless Swift build/tests for VT.
- CI owns Linux Clippy/transport and disposable installation proof
  `declared_agent_can_start_another_tasks_graph`; no installed build is changed.

Toolchain: rustup stable 1.99.1 / Clippy 0.1.99 (updated channel, unchanged).
All test commands strip inherited LF_* and close stdin; one build at a time.

## Result

Gate passes for the reconciled tree. Final HEAD is recorded in `.pr-copy-ref`.
The only post-sync changes are test readiness/expectation fixes:
- The existing title test required exactly one OSC, contradicting live updates;
  retain its expected title/provider-output checks and the new parser/stream proofs.
- Publish the provider fixture's JSON with atomic rename before readiness polling.
- Wait for both the completion receipt and socket cleanup under the existing
  20-second shutdown deadline. The first run observed a receipt before cleanup
  on shell cycle 7; the repaired test passed all 20 cycles. No retries added.

Commands and results (LF_* stripped; stdin closed):
- `uv run python scripts/test.py --list`: 12 product/test paths ultimately changed; architecture, focused Rust, website and headless Swift selected.
- `uv run python scripts/test.py --reuse-passing --rust-filter 'test(naming::) | test(terminal_title::) | test(ghostty_vt::) | test(transport::) | test(harness::agent_process::) | test(store::sqlite::processes::) | test(store::sqlite::sessions::) | test(store::sqlite::session_events::) | test(session_record::runtime::) | test(agent_startup_tests::) | test(session_cli_tests::) | test(interactive_graph_keeps_input_and_advances_only_after_each_provider_exits) | binary(session_launch_contract) | binary(transport_tests) | binary(lf)'`: 320/322 Rust passed, the two fixture failures above; architecture passed; 760 Swift tests and boundary check passed; website selected old installed lf because rustup's bin preceded the candidate. Log `/tmp/codex-resume-gate.log`.
- `PATH="$PWD/target/debug:$HOME/.cargo/bin:$PATH" uv run python scripts/test.py --base HEAD --website --reuse-passing --rust-filter 'binary(session_launch_contract) | binary(transport_tests) | test(session_cli_tests::terminal_titles_follow_session_rename_and_reconnect_without_provider_accounts)'`: PASS, 127/127 Rust, Clippy all targets with warnings denied, format/inventory/architecture, 73/73 website tests; 338s. Here HEAD was the reconciled merge before fixture fixes. Log `/tmp/codex-resume-gate-repair.log`.
- `cargo fmt --all` and `git diff --check`: PASS; no additional formatting changes.
- Linux Clippy/transport and installation proof `declared_agent_can_start_another_tasks_graph`: deferred to CI. Live cmux and installed-build verification remain unverified, not passed. No required check timed out.

195 unchanged Rust results plus the 127-check repair pass cover the original
322-check plan. Swift inputs did not change after its pass. The initial macOS
linker warning concerns the large unwind section; builds succeeded. Resource
preflight/postflight passed; optional cleanup could not run through the old
installed CLI, but no build cleanup was needed for the available disk budget.

Delivery direction retained verbatim: Jack said "i dont need to reiew any of this
yet just get it all landed". Parent runs `lf land --wait-and-fix`; no publication,
watcher or install was started here. PR copy retains the exact generalized-host
scope, prior disposable Rex/Herdr/tmux observations, and the cmux limitation.
