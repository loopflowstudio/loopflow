A disappearing terminal connection did not tell clients whether execution finished, the attachment moved, or the connection failed. An orderly ending now sends one final `0x07` Close frame with `replaced`, `finished`, or `retired`. EOF without that frame remains unknown.

<!-- loopflow:task-pr-context:start -->
> [!NOTE]
> **Task:** [Tell a terminal client why its bridge ended · LOO-774](https://linear.app/loopflow/issue/LOO-774/tell-a-terminal-client-why-its-bridge-ended)
> **PR lifecycle:** Merge delivers the code. Record independent later work as Tasks and comments; complete the delivered outcome.
<!-- loopflow:task-pr-context:end -->

The frame follows final output outside the PTY byte stream, so an unfinished escape cannot swallow it. The server and raw Desktop relay pass it through unchanged. Reattachment finds existing execution by Session, with fresh control claims and no input replay or provider launch. A replacement need not be ready yet.

This composes with held terminals: restarting the server retains the terminal and its PTY position; yielding the attachment reports `replaced`; task cleanup and shell Close report `retired`. The held-terminal revision advances to 2, retaining Position at `0x05` and Aside at `0x06`. `lf server describe` includes the close schema and reasons.

Desktop's decoded relay publishes `ended` on the private Unix status stream, disables input, preserves the execution exit code, and explains replacement or retirement. It stops automatic recovery. Existing generation fencing, native readiness and heartbeat behavior remain in place.

Validation after integration: `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed. Focused Rust protocol/relay tests and headless `ServerConnectionTests` are running; CI covers the affected suites on the published head.

Recorded November 8 macOS QA exercised real Codex resume and rollover, same-Session reattachment, outage recovery without replaying disconnected typing, native quit, shell retirement with job cleanup, and slow-client shutdown. Those observations predate the held-terminal integration and do not validate this final implementation. Mounted Desktop presentation and an installed-build upgrade remain unverified; a display-capable release check should confirm the final status, disabled input, stopped recovery, and retained Session identity and history through the actual upgrade.
