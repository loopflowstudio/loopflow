# Launcher output over Codex input

Jack Heart reported that `Preparing conversation (20s)` stayed in Codex's input
area and its timer continued increasing in `etude.experiment-api`.

The provider-switch branch of `human_session::open` retained its `Startup`
worker across `import`, which waits for the new native client to exit. Other
resume paths already drop the worker before handing over terminal ownership.
The repair drops it before importing the conversation; no history is removed.

The CLI regression uses an isolated Machine, a Claude transcript, a Codex
protocol stand-in and a real PTY. The unfixed CLI emitted repeated spinner
frames after `NATIVE_TERMINAL_READY`, including increasing seconds and erase-line
escapes. With the repair, that terminal assertion passes. A separate history
assertion exposed missing `userType: external` in the fixture; the fixture now
uses the native transcript shape.

Review: retain one spinner owner, join it before the blocking import, and keep
the existing import/history path. The test checks terminal bytes after handoff,
not destructor calls. It kills only its own process group on timeout.

Checks: `cargo test -p loopflow --test integration -- session_cli_tests:: agent_startup_tests::` — 23 passed; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` — passed. Cargo ran with inherited `LF_*` unset and stdin closed.
The reported existing terminal has not been reopened with the new executable.
