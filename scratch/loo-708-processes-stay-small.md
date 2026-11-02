# LOO-708: Loopflow's own processes stay small while conversations sit idle

Jack Heart measured about 150 `lf` processes holding 5.5 GB of footprint on
2026-11-02. Three changes, one PR. Measured on this Mac before the change
(footprint, release build): owner 16 MB, inner `session connect` 17 MB,
`lf -i` launcher 58 MB (peak 126 MB); a store-opening `lf` is 38 MB peak, a
Clap-only `lf --version` 6.8 MB peak.

## Decisions (Jack, 2026-11-02, Task directive)

1. The owner (`lf session serve`) takes over the launcher's wait; no `lf -i`
   stays beside Claude. Chosen over freeing the launcher's heap in place.
2. The viewer is a thin relay of a few MB, profiled first.
3. Desktop's chain flattens to one owner plus one viewer per conversation.

Scope split (Jack Heart, 2026-11-02): the viewer wire format (binary frames,
snapshot-first stream, byte-counted resume, nonce-tagged control messages)
belongs to LOO-712, running in parallel; this one changes which processes exist
and what each holds, and leaves the stream encoding in `transport.rs` alone.
The LOO-672 audit's 5 ms serve loop (200 wakeups/s per idle owner) is in scope:
the loop blocks in `poll` on its descriptors and a wake pipe that a blocking
child wait and the lifetime policy write to. This leaves SIGCHLD to background
runtimes and eliminates the global signal descriptor. Timed output waits use
the actual exit/replay deadline.

## Approach

**Exec, not a new wait.** A process that already owns the child keeps owning
it across `exec`: PID, parent, PTY, process group and the owner's `wait`
all survive. So nothing learns to wait on a non-child.

- **Launcher → provider.** `run_native_session` (the generic/Claude branch,
  owned launch under a transport owner) records the AgentProcess identity for
  its own PID, hands the owner a `NativeExit` record over the control socket
  (`Request::ExitRecord`, acknowledged with `Response::Accepted`), then `exec`s
  the provider `Command`. The launch lock stays held until identity is recorded.
  The owner completes the launcher's ledger row through `record_process` on child
  exit, performs what the launcher used to: AgentProcess exit, attachment
  finish, capture settlement (`CaptureHandoff` carries attempt/seq state),
  account re-pin, moved notice, native exit text, status report, diagnostics
  line, and runs the repository's planning sync meanwhile. A refused or failed
  handoff falls back to the ordinary spawn-and-wait. Codex and OpenCode keep
  their launcher (relay clients, private servers). Graph steps share a terminal
  but remain children of their Graph: the owner declines their wait handoff,
  so the existing launcher settles each step before the Graph continues.
- **connect → `lf -i`.** `serve_locked` `exec`s the `lf -i` command instead of
  spawning it; the launch lock crosses the exec as an inherited descriptor
  (`LF_SESSION_LAUNCH_LOCK`) and the launcher drops it where it already drops
  the resume path's lock. Handoff env uses `carry_across_exec`.
- **Viewer → relay.** `transport::attach` completes the viewer's ledger row
  and `exec`s `lf session attach OWNER [--moved TEXT]`, handled at the top of
  `run()` before Clap, tracing or any store: socket, stdin, stdout only.

## Delete — do not maintain

Removed in this diff:

- `spawn_session_process`'s resumable-poll loop, `session_is_resumable` and
  the binary digest diagnostics (`ops/human_session.rs`), with their tests
  `foreground_session_can_wait_for_first_use_before_native_history` and
  `initial_session_publication_requires_history_and_a_live_agent`: replaced by
  the inherited launch lock. A provider that exits before publishing history
  now fails the connect with the launcher's own error
  (`session_lifecycle_tests` asserts "failed to start").
- Launcher-side post-wait bookkeeping in `run_native_session` for the exec
  path; the non-exec path keeps it for Codex/OpenCode/remote clients.
- `complete_handed_off_process`'s separate SQL writer: use `record_process`,
  including its existing completion and retention rules.
- `WAKE_FD`, `on_child_exit`, `wake_on_child_exit`, and timed polling of the
  child: a blocking wait wakes the same socket as the lifetime policy.

No remaining deletion targets.

Preserved: exit recording (attachment_exit events, AgentProcess completed_at,
terminal.json), takeover notices, `lf resume`, `lf resume --all`, Graph steps,
transport/session lifecycle/launch-contract tests.

## Done when

- [x] Desktop-started Claude conversation: `lf` processes are one owner and one
  viewer; no `lf -i` alive once Claude runs
  (`opening_a_prepared_conversation_runs_one_owner_and_one_relay`).
- [x] Viewer footprint a few MB; before/after numbers in the PR (see Checks).
- [x] `cargo test -p loopflow --test transport_tests`, `--test session_lifecycle_tests`,
  `--test session_launch_contract` pass.

## Checks

2026-11-02: with inherited `LF_*` cleared and stdin closed, `cargo test -p loopflow --test transport_tests` passed 23 tests; `cargo test -p loopflow --lib harness::agent_process` passed 18; the focused interactive Graph regression passed again after making its declined handoff quiet; `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed. Gate owns lifecycle/launch-contract, startup-cost and journal acceptance checks after compression (implementation baseline: lifecycle 28, launch-contract 25, focused unit tests 111 passed).

The macOS lib-test linker warned that its unwind section exceeded 16 MB; the tests passed.
Footprint before compression (release build, isolated home, Claude stand-in, `footprint`):
owner `lf session serve` 20 MB (it now runs the launch's planning sync),
relay `lf session attach` 2.5 MB, nothing else; before, the same conversation
held owner 16 MB + inner connect 17 MB + `lf -i` 58 MB. Claude's parent is the
owner.
