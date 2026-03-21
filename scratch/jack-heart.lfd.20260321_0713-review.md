# Review: session attach contract cleanup

## What was implemented

- Added daemon HTTP routes for terminal sessions: list, get, attach, start, complete, and cancel.
- Replaced attach-time launch argv with structured tmux connection metadata (`session_name`, `host`, `cwd`, `status`).
- Added Swift models and service parsing for terminal sessions plus `TerminalConnectionInfo`.
- Updated Concerto's terminal workspace to build the local `tmux attach-session` command or a remote `ssh ... tmux attach-session` command from connection info instead of daemon-supplied argv.
- Documented the lifecycle/client-access design and the non-tmux attach assumption in `scratch/`.

## Key choices

- **Return connection metadata, not executable argv.** This keeps `lfd` out of the terminal byte path and lets clients decide whether to attach locally or over SSH.
- **Normalize loopback hosts to `localhost`.** The attach route collapses `127.0.0.1`, `::1`, and `localhost` so Concerto can reliably choose the local tmux fast path.
- **Reject non-tmux attach attempts with `412 Precondition Failed`.** The branch treats attach as a tmux-only capability instead of fabricating a misleading launch payload.
- **Keep session lifecycle APIs explicit.** Separate list/get/attach/start/cancel endpoints and matching Swift service methods keep the state machine reviewer-visible.

## How it fits together

`lfd` stores and updates `TerminalSession` records, then the new `/v0/terminal-sessions/*` routes expose those records and return a transport-agnostic attach payload for tmux-backed sessions. Concerto fetches that payload through `WaveService`, turns it into a local tmux or remote SSH command in `TerminalAttachCommand`, and hands that command to Ghostty without routing terminal bytes back through the daemon.

## Risks and bottlenecks

- Remote attach currently assumes the machine is reachable over SSH at the same host the client used for the HTTP request.
- Non-tmux sessions are intentionally non-attachable; callers must handle the `412` path cleanly.
- There is still no executor-regression suite covering queued activations, cancellation propagation, and other daemon executor parity cases called out in the design doc.
- `xcodebuild` validation is slower than package tests because it builds the full Concerto app and package dependencies.

## What's not included

- Harness server mode for non-terminal/mobile clients.
- SSH brokering, embedded SSH auth, or any terminal-byte proxying through `lfd`.
- Executor regression coverage beyond the focused attach/session tests added on this branch.

## Validation

Done-when checks from `scratch/jack-heart.lfd.20260321_0713.md`:

- `TerminalLaunchSpecDto` deleted from Rust/Swift codepaths (`rg "TerminalLaunchSpecDto|TerminalLaunchSpec" rust swift docs scratch`).
- Attach endpoint returns `TerminalConnectionInfo` (`rust/loopflow/src/lfd/http/routes/terminal_sessions.rs`, `swift/LoopflowCore/Models/TerminalSession.swift`).
- Concerto attaches through client-built tmux/SSH commands (`swift/Concerto/Platform/macOS/Views/TerminalWorkspaceView.swift`; validated by tests below).
- Terminal bytes stay out of `lfd`; the daemon returns metadata only and Ghostty attaches directly to tmux.

Commands run:

- `cargo fmt --check`
- `cargo clippy -p loopflow -- -D warnings`
- `cargo test -p loopflow terminal_sessions -- --nocapture`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests/GhosttyTerminalViewTests -only-testing:ConcertoTests/RepoStateInteractiveSessionTests`

Result: all passed during gate. No additional code changes were needed in this pass.
