# LOO-825 gate, 2026-11-10

Comparison: checkout base `0766e9cbf` through `42b8c0f35`, including local edits.

Check plan:

- `open`, its host/workspace adapters, Desktop inspection, primary selection and
  transport → `lf::commands::open::`, `lf::commands::desktop::`,
  `ops::human_session::`, `transport::`, `work_open_tests::`, `transport_tests`.
- CLI dispatch/navigation → `lf::tests::`, `wave_resolution_matrix::`,
  `cli_discovery::`, CLI binary tests. DTO changes → `dto_fixtures::` and
  `server::schema::tests::`.
- Docs, builtins and command examples → `documented_commands::`, architecture
  map and website tests.
- Swift opening state, retained panes, transport readings and DTOs → headless
  Swift app/view tests and multiplatform check; Ghostty view changes also reach
  the Xcode terminal-fallback build.
- Native host arrangements → ignored isolated Rex/Herdr fixtures, one at a time.
  Prior QA already records real conversation routing and failure/retry. Native
  provider authentication and visible-window judgment remain outside headless gate.

Investigate the three recorded 2-second host-request timeouts under the same
four-test parallelism, sampling the fixture child before it is killed. A later
pass alone does not explain those failures. Then run the changed-aware runner
with the listed Rust filter, `--reuse-passing` and `--loopflow`.

The focused diagnostic used the prior PATH toolchain (Rust 1.98.1): 34/34
Work-opening tests passed in parallel in 22.292 s. The host process monitor
found no qualifying shell/Perl child lasting over 0.5 s, so it captured no
stalled-child sample. This does not explain the earlier timeouts.

Production latency signals already exist: `SessionsLatencyMetrics` records
`time_to_live_pane`, and `Perf.agentInput("input_ready", …)` marks input readiness.
No latency improvement is claimed from this verification run.

## Current checks

All commands clear inherited `LF_*` and close stdin. The final gate puts
`target/debug` and rustup's `~/.cargo/bin` first on PATH (Rust 1.99.1).

- `uv run python scripts/test.py --list` — identified architecture, Rust, website and Swift; Xcode added for the changed Ghostty fallback boundary.
- `uv run python scripts/test.py --reuse-passing --loopflow --rust-filter 'test(lf::commands::open::) | test(lf::commands::desktop::) | test(lf::tests::) | test(wave_resolution_matrix::) | test(cli_discovery::) | test(work_open_tests::) | test(dto_fixtures::) | test(documented_commands::) | test(transport::) | test(server::schema::tests::) | test(ops::human_session::) | binary(transport_tests) | binary(lf)'` — PASS, five suites in 891 s; gate.
- Architecture/inventory and `cargo fmt --all -- --check` — passed; gate.
- Filtered Rust nextest run — 450/450 passed in 271.280 s; gate.
- `cargo clippy --all-targets --jobs 4 -- -D warnings` — passed with Rust 1.99.1; gate.
- `cd website && uv run python dev.py test` — 73/73 passed; gate.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone` — app built, 804 tests passed in 167.375 s with WindowServer denied; gate. Optional display diagnostics skipped.
- `uv run python scripts/check_swift_multiplatform_boundaries.py` — passed; gate.
- Xcode `build-for-testing` via `--loopflow` — passed, app and test runners compiled with the terminal fallback; gate. No hosted UI behavior claimed.

Full logs: `.lf/tmp/gate/run-26370/`; traced output and process inventory:
`.lf/tmp/gate-open/full/`. All 34 Work-opening tests passed in this run too.
The process monitor sampled descendants every ~0.08 s and would sample a host
shell/Perl child observed for over 0.5 s; none qualified. No timeout was increased
and no source change was made to explain the prior failures.


## Native acceptance and fixture repair

- `cargo test -p loopflow --test integration rex_native_fresh -- --ignored --test-threads=1` — passed in 10.24 s after migrating the fixture's in-host arrangement command from `open` to `workspace`; gate.
- `cargo test -p loopflow --test integration herdr_native -- --ignored --test-threads=1` — passed in 5.39 s; gate (under the network sandbox).
- `cargo test -p loopflow --test transport_tests rex_ -- --ignored --test-threads=1` — three passed in 12.83 s: manual-resume cwd, fresh conversation with typed input, existing provider with retained draft; gate.

The first Rex attempt was incorrectly wrapped in `scripts/test_network.py`.
The native terminal reported exactly `rex: exec /usr/bin/login: Operation not
permitted`; the workflow pane exited 127 and conversation opens timed out after
30 s because their terminal command never executed. A capture of the isolated
workspace conversation showed the same launcher error. Running the documented
direct commands fixed this environment mismatch. These are separate from the
older two-second CLI stand-in timeouts, which remain unexplained.

The fixture now includes terminal text when its directory assertion times out.
`rust/loopflow/tests/README.md` records the macOS native-launch constraint.
Initial logs: `.lf/tmp/gate-open/native-{0,1,2}.log`; passing Rex logs:
`.lf/tmp/gate-open/native-direct-{0,2}.log`. Fixtures stopped their private servers.

Remaining shipping blocker: trace a failing 2-second host request to its stalled
child/OS wait. The prior failure is retained in `publish.md`; this gate's passes
are not its explanation. No host timeout was increased. No production behavior
changed during this gate.

- `uv run python scripts/test.py --base HEAD --reuse-passing --rust-filter 'test(work_open_tests::) | test(documented_commands::)'` — passed after the fixture/docs repair: 38 tests, architecture, inventory, formatting and Clippy; two suites in 33 s. Trace: `.lf/tmp/gate-open/repair/`; no stalled-child sample or host timeout.
- `git diff --check` — passed after the final edits; gate.

Unchanged production Rust, Swift, website and Xcode inputs keep the full gate's
results. Optional display checks and authenticated provider behavior remain
unmeasured. No PR was published and no Task completion was recorded.
