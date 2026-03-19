<<<<<<< HEAD:wave/lfd/4-daemon-aware-cli-contract.md
---
linear_id: 55445a30-5c37-4a42-b773-66adbedb0dda
---
=======
>>>>>>> eb790e5f (concerto: stabilize bundled daemon terminal handoff):wave/lfd/02-daemon-aware-cli-contract.md
# 01: Daemon-Aware CLI Contract

**Finish line:** `lf` can run normally in a plain shell, but when it detects an `lfd`-managed environment it emits structured lifecycle events that let `lfd` track runs, sessions, waits, and outcomes without scraping terminal output.

## Context

The runtime reframe only works if `lfd` can observe the real CLI without becoming the place where flow semantics live. Terminal scraping is too brittle: aliases, wrappers, subshells, shell noise, and prompt formatting all make it hard to tell what actually happened. The clean boundary is for `lf` to know what it is doing and report that back in structured form.

This contract has to work for both automated runs that `lfd` starts itself and interactive runs started by a human or agent inside an attached daemon-owned shell. It also needs to survive local-first adoption without painting remote or SSH-style access into a corner.

## What to build

1. **Detection contract.** Define how `lf` detects `lfd`: env vars, auth token, socket/HTTP target, session ID, run ID, wave ID, repo/worktree identity. Keep the contract explicit and versioned.

2. **Lifecycle events.** Define structured events for:
   - command start
   - resolved step / flow
   - wave / run / session correlation
   - interactive wait points
   - completion
   - failure
   - cancellation

3. **Delivery semantics.** Make event delivery reliable enough that `lfd` can reconcile process state and store state without callback shell hacks. If delivery can fail, define retry and fallback behavior deliberately.

4. **Backward-safe CLI behavior.** Outside an `lfd`-managed environment, `lf` should behave like the normal CLI. The daemon-aware path is additive, not a forked CLI.

5. **Parity tests.** Add tests proving that the same `lf <flow-or-step>` command can run with and without `lfd`, with the daemon-aware path adding observability rather than changing execution semantics.

## Design guidance from tmux study

### Event framing: learn from control mode, don't copy it

tmux control mode uses `%begin`/`%end` framing with command numbers for request-response correlation, and `%`-prefixed async notifications that never interleave with response blocks. The principle is right: structured framing with clear boundaries between request-response and async events.

But tmux's line-oriented text protocol is optimized for terminal-to-terminal bridging. `lf` → `lfd` should use a richer format (JSON-over-HTTP or JSON-over-socket) since both sides are programs.

Key control mode lessons to keep:
- **Command numbers / correlation IDs.** Every event from `lf` should carry a run ID that correlates back to the `lfd`-started or `lfd`-observed execution. tmux uses monotonic integers; loopflow should use the existing `LfdId` scheme.
- **Async notifications are separate from request-response.** Lifecycle events (step started, wait point hit, completion) are fire-and-forget notifications, not request-response. Don't force `lf` to wait for `lfd` acknowledgment before proceeding.
- **Flow control matters.** tmux added `pause-after` because high-output panes could overwhelm control clients. Agent sessions can produce massive output. The event protocol should be resilient to `lfd` being slow — either fire-and-forget with best-effort delivery, or bounded queue with drop-oldest semantics. Never block `lf` execution on event delivery.

### Identity: pre-assign, don't discover

tmux's identity model works because the server assigns all IDs — clients never create sessions/windows/panes independently. For daemon-started runs, `lfd` should pre-assign run ID, session ID, and wave association before spawning `lf`. For CLI-started runs observed by a running `lfd`, the detection handshake should let `lf` register and receive an ID immediately rather than self-assigning.

This follows tmux's principle: one authority for identity (the server), zero for identity conflicts.

### Authentication: Unix permissions first, tokens later

tmux uses pure filesystem permissions on the socket directory. No cryptographic auth. This works for local use. `lfd` should start the same way: the shared runtime store (SQLite file or Unix socket) uses filesystem permissions. Add token-based auth only when remote access arrives.

### Transport: WezTerm's Domain abstraction is the model

WezTerm proved that local, SSH, Unix socket, and TLS connections can all implement the same spawn/pane interface. The `lf` → `lfd` event contract should be transport-agnostic from day one: define events as typed messages, let the delivery mechanism vary (HTTP for now, socket later, remote transport eventually) without changing the event schema.

## Key consumers

### `lf` calling the attention API

When `lf` hits `WaitInteractive` for a checkpoint step, it should `POST /attention` with the appropriate context (step name, terminal session ID, design path). When the step completes, it should `POST /attention/{id}/resolve`. The HTTP routes already exist — the daemon-aware CLI contract is what lets `lf` discover and authenticate to them.

## Open questions

- Should event delivery go over HTTP, a unix socket, stdio side channel, or some combination? (Guidance: HTTP is simplest for v0 since `lfd` already has an HTTP server. Stdio side channel is worth considering for daemon-spawned processes where `lfd` owns the PTY.)
- How much identity should `lfd` pre-assign versus letting `lf` create and report? (Guidance: pre-assign for daemon-started runs, register-on-detect for CLI-started runs.)
- What is the minimum event set that keeps store reconciliation reliable without over-coupling `lf` and `lfd`? (Guidance: start with the tmux notification set as a reference — the ~20 control mode notifications cover lifecycle, not content. `lf` events should be fewer: start, step-resolved, wait, complete, fail, cancel.)

## Done when

- `lf` can detect an `lfd`-managed environment and authenticate back to it
- `lfd` receives structured lifecycle events for both automated and interactive execution
- Run/session attribution no longer depends on terminal scraping or ad hoc shell callbacks
- Running `lf` outside `lfd` still behaves like the normal standalone CLI
- Tests pin the event contract tightly enough that `lf` and `lfd` cannot drift silently
