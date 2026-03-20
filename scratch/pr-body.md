## Try it!

```bash
# Flow engine + shared journal contract
cargo test -p loopflow golden_flows journal::tests::journal_writes_run_flow_and_step_events_in_wave_worktree

# Live daemon HTTP + websocket behavior
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v

# Concerto workspace / multiplexer state
swift test --package-path swift --filter MultiplexerTests

# Full gate pass (clean UI test still blocked)
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
cargo test -p loopflow docker_
uv run pytest python/tests/
tests/e2e/test_smoke.sh
cd swift && xcodegen generate && xcodebuild clean test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

What you'll see:
- flow/journal tests exercise the new shared runtime contract and expanded builtin flow graph
- the e2e API suite hits live wave CRUD plus concurrent websocket/session fanout
- `MultiplexerTests` covers the new terminal workspace layout model in Concerto
- every documented CI-style check passes except the clean macOS UI test, which still fails bootstrapping `ConcertoUITests-Runner`

## Intent

This branch makes loopflow feel like one system. `lf` remains the real execution engine, `lfd` becomes the supervising runtime host around it, PM sync becomes a first-class workflow instead of a side export, and Concerto becomes a workspace client that can show portfolio state, attention, runs, and local terminal sessions on top of the same runtime model.

## Assumptions

- `lfd` should supervise and observe normal `lf` execution rather than grow a second long-term flow executor.
- Wave authors can move onto the renamed flow families (`build`, `garden`, VSM governance) and priority-bucket conventions without needing compatibility shims for older branch-local content.
- Concerto terminal embedding is still local-first; remote terminal hosting can come later without changing the higher-level workspace model.
- PM sync targets Linear and Asana as the active providers, with roadmap ordering expressed through bucketed wave items.

## Key decisions

- Replaced the older runtime/meta-file model with direct journal v2 events that `lfd` can replay without a translation layer.
- Renamed `tend` → `garden`, added loop/xor/VSM flow structure, and updated builtins/docs to match that model.
- Expanded lfd/ops PM support around init, pull, status, provider-specific APIs, and priority-bucket reconciliation instead of keeping PM support as export-only plumbing.
- Built Concerto's terminal work around workspace state, multiplexed layouts, and attention/run context instead of a terminal-takes-over screen.

## Not included

- Daemon-hosted PTYs or remote terminal transport
- Dedicated CLI escalation signals for `*.escalated` journal events
- A fix for the clean `ConcertoUITests-Runner` bootstrap failure
