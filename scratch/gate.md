# LOO-826 gate, November 11

Compare with main at `1570bef610`; include the existing uncommitted design note.

Plan from the 76 changed paths:

- Harness, hooks and launch paths: `harness::`, `context_block::`,
  `lf::commands::context::`, `lf::commands::util::`, `lf::tests::`,
  `agent::session::`, context/settings launch, startup, session CLI and launch
  contract tests; Pi and OpenCode Node probes.
- Server wake and DTOs: `server::boundary::`, `server::wake::`,
  `server::schema::`, `server::wire::`, `server::client::`, DTO fixtures;
  Swift headless build/tests selected by the changed-aware runner.
- Store/activity and migration: `store::sqlite::steers`, `session_events`,
  `task_comments`, `task_work`, `revisions`, `native_input`, session metadata,
  `store::migrations::`, `process::runtime::`, `transport::lifetime`,
  `session_steers::`; standalone `two_people` and `transport_tests`.
- Removed CLI command and observer startup: `binary(lf)`, `cli_discovery::`.
- Task steps now carry declared Work: `ops::run::tests::` and
  `task_graph_launch_tests::` as a supplemental focused run.
- Docs and resources: documented commands, architecture, website, Python and
  inventory checks selected by `scripts/test.py`; package extraction build
  because the embedded plugins changed.

LOO-841 already owns one Session inbox and actionable/default vs opt-in waking;
LOO-838 owns deletion of the retained `task-updates` no-op. These remain outside
this gate's code polish. No new Task or execution is needed.

Checks:

- `uv run python scripts/test.py --list`: 76 paths; architecture, Python, Rust,
  website, Swift selected. Full Rust matrix replaced with the filter above.
- `uv run python scripts/check_migrations.py`: pass; one unmerged additive
  migration, `agent_process_inbox.sql`.
- `node scripts/benchmarks/skill-invocation/test_{pi,opencode}_context.mjs`:
  both pass after fixtures included the shared client, as production does.
- `scripts/test.py --reuse-passing --rust-filter <above>`: resource preflight
  failed before product checks. Shared uv-cache inventory exceeded 15 seconds;
  115 GiB free (32 GiB floor). Automatic build cleanup also returned exit 2.
  No cache repair attempted. Running the same generated command plan directly,
  serially, retaining each phase's original timeout; logs in
  `.lf/tmp/gate/direct-826/`.
- `uv run python scripts/check_architecture.py`: pass.
- `cargo build -p loopflow --bin lf --jobs 4`: pass (149 s; Rust 1.99.1).
- Python: 652 pass; the one nested-sandbox skip passes separately (653 total).
- First Rust pass found four failures: the Codex login-move fixture waited
  75 seconds for its old native client to be signalled; three Pi native
  fixtures omitted the newly required nullable `inbox` field. Fixed the Pi
  wire fixture and made the execution owner check its exec'd client's
  attachment at its existing two-second lifecycle cadence. A failed store
  read leaves the client running. This restores the observer's retirement
  duty without restoring its process or Task-update polling.
- First Rust subset: 840/844 pass; the four failures above are the repair set.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone`: app build and 806 tests in
  98 suites pass; `check_swift_multiplatform_boundaries.py` passes.
- First website attempt selected `~/.cargo/bin/lf` before the candidate and
  failed at `help --all --json`; rerun puts `target/debug` first on PATH.
- Follow-up Rust subset: 270/271 pass, including all three Pi fixtures and the
  Task-launch checks. The first Codex retirement repair stopped the terminal
  but returned signal 9 without the move notice. The final repair moves reads
  to the existing lifecycle worker, signals only the native client, and keeps
  the observed move as the fallback while recording its exit.
- `cargo test -p loopflow --test session_launch_contract a_codex_terminal_moved_to_another_login_ends_and_says_where_it_went -- --exact --nocapture`:
  pass (10.49 s test execution) with the final repair and prepared candidate CLI.
- Query plans for `idle_conversations` (all and one Session) and undirected
  Task comments use `processes_unfinished`, attachment/activity indexes and
  `idx_task_comments_task`; no full historical-process or Task scan. Checked
  with measured large-store statistics and a 10,000-Task estimate, not a timed
  10,000-Task workload. Script and output: `/tmp/loo-826-query-plans.py` and
  `.lf/tmp/gate/repaired-826/query-plans.log`.
- Final transport/launch/docs subset: 212/213 pass (448 s execution). Codex
  account move and both new retirement tests pass. The remaining Claude fixture
  quit immediately after the credential changed, before its resumed viewer had
  attached, allowing resume to relaunch it. The fixture now sends input through
  that viewer and waits for the native client to receive it before quitting;
  its focused recheck follows. No retry or longer deadline was added.
- `cargo test -p loopflow --test session_launch_contract a_running_claude_named_onto_another_login_keeps_its_client_and_says_where_it_runs -- --exact --nocapture`:
  pass after synchronizing on input reaching the resumed client (14.05 s).
- `uv run python dev.py test` in `website/`, candidate first on PATH: 73 pass,
  including accessibility and responsive layout; architecture check also passes.
- Packaging warns that the existing locked `chacha20 0.10.0` is yanked on
  crates.io (November 11). Dependencies are unchanged by this branch.
- `cargo package --locked --allow-dirty -p loopflow --jobs 4`: pass; 894 files,
  extracted source archive compiled successfully (389 s).
- `cargo clippy --all-targets --jobs 4 -- -D warnings`: pass (333 s).
- `cargo fmt --all -- --check` and `git diff --check`: pass.

All planned local checks completed. Initial failures are accounted for above;
no required headless check is deferred. The full Rust matrix belongs to CI.
The ten-Session live demo and Codex `/new`/`/fork` rendering checks remain with
demo/review; existing real-provider QA covered four Sessions. LOO-841 retains
the known Task-progress delivery-confirmation limitation. No test server or
watcher started by gate remains running.
