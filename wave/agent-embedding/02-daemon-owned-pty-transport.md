# 02: Daemon-Owned PTY Transport

**Finish line:** Interactive wave steps run inside `lfd`-owned PTY sessions that execute `lf <step-or-flow>` in the wave's executor environment, and Concerto attaches to that PTY stream instead of launching a wrapped local shell command.

## Context

The local terminal workspace milestone shipped. `lfd` now creates durable `terminal_sessions`, persists their status, emits lifecycle events, and lets Concerto surface a tracked Terminal tab only when the selected wave has an active interactive run. That solved the product problem of making terminal embedding additive instead of a full-window takeover.

What shipped is still a transport shim, not the final model. Today `attach` returns a `TerminalLaunchSpec`; the stored session still carries agent argv; Ghostty locally launches a wrapped command; and completion depends on a `curl` callback back into `lfd`. That keeps lifecycle state consistent, but it is not the same as a human running `lf design`, it does not generalize cleanly to remote executors, and it leaves terminal cleanup vulnerable to callback failures.

This is the next risky learning step for the wave. Finish it before broader lifecycle, remote, or compositor work grows around the temporary launch-spec path.

## What to build

1. **Server-owned PTY sessions in `lfd`.** Add a PTY/session manager that creates, tracks, resizes, writes to, reads from, and cleans up interactive sessions. `TerminalSession` should describe the session and its executor target, not a UI-local launch trick.

2. **Run the real loopflow command.** Interactive overrides should execute the normal CLI entrypoint inside the chosen executor environment: `lf design`, `lf review`, `lf ship-roadmap`, and so on. The command shown in the terminal should match what a human would run by hand.

3. **Transport API, not launch specs.** Replace the current attach path with an attach/read-write/resize/close protocol. HTTP plus websocket streaming is fine; the important part is that `lfd` stays the process owner and clients become terminal attachments.

4. **Wave-run integration and reconnect.** PTY exit status should continue to resume or fail the waiting wave run, but completion should no longer depend on a shell callback. Support live detach/reattach during one daemon lifetime so Concerto can survive view changes and future `lfq` clients can reuse the same primitive.

5. **Executor-neutral seams.** Local execution is the first proof point, but the model must leave a clean seam for container and remote executors. Do not bake host-local shell assumptions into the transport contract.

6. **Proof through tests.** Add backend and client coverage for PTY session lifecycle, attach authorization, resize/input handling, exit-driven wave resume/failure, and reconnect behavior.

## Follow-on once the transport exists

- Reuse the same PTY primitive for remote-repo terminal embedding instead of leaving remote repos on queue/detail-only surfaces.
- Add an attached CLI entrypoint such as `lfq run <wave> -- design` so Concerto and CLI clients share one interactive execution model.
- Consider scrollback persistence after the live attach path is stable; do not block the transport milestone on durable replay.

## Open questions

- How much scrollback should `lfd` retain for reconnect during a single daemon lifetime?
- What is the right auth shape for streamed terminal attach across local and future remote deployments?
- Should `TerminalSession` store both the requested loopflow step/flow and the resolved executor command, or can it derive the latter on demand?

## Done when

- Running a paused wave interactively from Concerto shows the real `lf <step-or-flow>` command in the terminal, not a wrapped raw agent command
- `lfd` owns the PTY lifecycle; Ghostty only renders and forwards terminal I/O
- Terminal session completion no longer depends on a local `curl` callback
- Detaching and reattaching during a live session works without losing the wave/run association
- The API is transport-neutral enough that container and remote executors can adopt it without a second interactive model
