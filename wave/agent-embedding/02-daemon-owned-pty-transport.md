# 02: Daemon-Owned PTY Transport

**Finish line:** `lfd` owns interactive shells / PTYs and automated run processes, all of them executing normal `lf <step-or-flow>` commands in the correct worktree and executor environment. Concerto or SSH-style clients attach to those daemon-owned sessions, while `lf` detects `lfd` and emits structured lifecycle events back to it.

## Status

This diff improves the terminal workspace seam, but it does **not** close out interactive sessions working end-to-end inside the Loopflow Swift app. Flow override visibility, bundled-`lfd` lifecycle, retry-loop fixes, and terminal auto-presentation moved forward, but daemon-owned PTY transport, `lf`/`lfd` event parity, and fully reliable in-app handoff for interactive runs remain open. Treat item 02 as still in progress.
The deeper runtime reframe now lives in `wave/lfd/`: `lfd` as runtime host, real `lf` commands as the execution path, and SSH-style / daemon-owned shell access as the interactive model. Item 02 should consume that wave, not redefine it.

## Context

The local terminal workspace milestone shipped. `lfd` now creates durable `terminal_sessions`, persists their status, emits lifecycle events, and lets Concerto surface a tracked Terminal tab only when the selected wave has an active interactive run. That solved the product problem of making terminal embedding additive instead of a full-window takeover.

What shipped is still a transport shim, not the final model. Today `attach` returns a `TerminalLaunchSpec`; the stored session still carries agent argv; Ghostty locally launches a wrapped command; and completion depends on a `curl` callback back into `lfd`. That keeps lifecycle state consistent, but it is not the same as a human running `lf design`, it does not generalize cleanly to remote executors, and it leaves terminal cleanup vulnerable to callback failures.

The target inversion is: `lfd` hosts the terminal environment and supervises processes, while the real `lf` CLI performs execution and reports structured events when it detects an `lfd`-managed session. Automated runs still begin in `lfd`, but they do so by forking normal `lf <flow-or-step>` commands in the right worktree. Interactive clients — Concerto first, SSH-style access later — attach to fresh or existing daemon-owned shells and run the same commands by hand.

This is the next risky learning step for the wave. Finish it before broader lifecycle, remote, or compositor work grows around the temporary launch-spec path.

## What to build

1. **Server-owned shells / PTYs in `lfd`.** Add a PTY/session manager that creates, tracks, resizes, writes to, reads from, and cleans up interactive sessions. `TerminalSession` should describe a daemon-owned shell or PTY session and its worktree / run association, not a UI-local launch trick.

2. **`lfd` starts normal `lf` commands.** Automated runs should still be initiated by `lfd`, but by forking normal `lf <flow-or-step>` commands in the correct worktree and executor environment rather than executing flows through a second bespoke daemon path.

3. **`lf` becomes daemon-aware.** When `lf` detects an `lfd`-managed environment, it should emit structured lifecycle events: command start, resolved flow/step, run/session association, interactive wait points, completion, and failure. `lfd` should observe execution through those events rather than scraping shell output.

4. **Transport API, not launch specs.** Replace the current attach path with an attach/read-write/resize/close protocol. HTTP plus websocket streaming is fine; the important part is that `lfd` stays the process owner and clients become terminal attachments to fresh or existing sessions.

5. **Wave-run integration and reconnect.** PTY exit status should continue to resume or fail the waiting wave run, but completion should no longer depend on a shell callback. Support live detach/reattach during one daemon lifetime so Concerto can survive view changes and future CLI / SSH-style clients can reuse the same primitive.

6. **Executor-neutral seams.** Local execution is the first proof point, but the model must leave a clean seam for container and remote executors. Do not bake host-local shell assumptions into the transport contract.

7. **Proof through tests.** Add backend and client coverage for PTY session lifecycle, attach authorization, resize/input handling, exit-driven wave resume/failure, `lf` event reporting, and reconnect behavior.

## Follow-on once the transport exists

- Reuse the same PTY primitive for remote-repo terminal embedding instead of leaving remote repos on queue/detail-only surfaces.
- Add an attached CLI or SSH-style entrypoint so Concerto and terminal clients share one interactive execution model around real `lf` commands.
- Consider scrollback persistence after the live attach path is stable; do not block the transport milestone on durable replay.

## Open questions

- How much scrollback should `lfd` retain for reconnect during a single daemon lifetime?
- What is the right auth shape for streamed terminal attach across local and future remote deployments?
- What is the cleanest way for `lf` to detect and authenticate back to `lfd` — env vars, socket path, token file, or some combination?
- Should `TerminalSession` store both the requested loopflow step/flow and the resolved executor command, or can it derive the latter on demand?

## Done when

- Automated runs started by `lfd` execute normal `lf <step-or-flow>` commands in the correct worktree
- Running a paused wave interactively from Concerto or an attached shell shows the real `lf <step-or-flow>` command in the terminal, not a wrapped raw agent command
- `lf` detects `lfd` and emits the structured events needed for wave/run/session tracking
- `lfd` owns the PTY lifecycle; Ghostty only renders and forwards terminal I/O
- Terminal session completion no longer depends on a local `curl` callback
- Detaching and reattaching during a live session works without losing the wave/run association
- The API is transport-neutral enough that container, remote, and SSH-style clients can adopt it without a second interactive model
