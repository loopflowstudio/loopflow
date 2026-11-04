# Loopflow session/terminal/Desktop audit (2026-11-02, research agent; file:line evidence)

## Why the 24h unviewed limit never ended an owner on Jack's Mac

- "Viewed" = a socket that sent `Attach` (transport.rs:584); the owner's own child only opens
  control connections, so it does not count itself. Desktop's pooled pane keeps its viewer
  attached while hidden, so an open app keeps every visited owner "viewed".
- Cause 1 (confirmed on the live store for the LOO-618 chain): `owner_waits_for_person`
  (store/sqlite/sessions.rs:613-638) walks a recursive `graph_ancestors` CTE over every
  `graph_processes` row with no completion filter. Any owner whose agent ever ran
  `lf task run`/`lf ship` is a Graph ancestor forever and can never be ended. 45 live
  interactive owners, 34 unviewed, 18 of those are Graph ancestors.
- Cause 2 (confirmed, 77 owners): the policy thread propagates store errors with `?`
  (lifetime.rs:48-49, :79, :87) and dies ("Session owner policy stopped"). lf.log: 43 lines
  `database is locked` on 2026-10-27, 34 lines `incompatible Loopflow database … lf=0.15.0`
  on 2026-10-30 (0.15.1's migrations). Zero "ending a Session nobody has viewed" lines exist:
  the policy has never ended an owner here.
- Cause 3: owners started from the 0.15.0 binary predate the executable-replacement check
  (#1765) and kept running the old image.
- Same class: `TURN_RUNNING` with a `program_status` row that has no idle/blocked/done record
  evaluates true (sessions.rs:31-36); a provider that stops reporting leaves its owner immortal.

## Per-conversation processes and why

- Viewer (`session resume <id>`, ~32 MB): a terminal must be able to die without the agent.
- Owner (`serve --terminal -- <argv>`): owns PTY, vt100 snapshot, viewer list. `open()` wraps
  whenever no owner answers (ops/human_session.rs:834-836); `delegate_current` re-execs the
  viewer's argv under `serve` (transport.rs:172-195), so the first `connect` becomes the
  launcher inside the owner.
- Launcher (`lf -i` / inner `resume`, ~73-100 MB): spawns claude and waits in a 50 ms
  `try_wait` loop (util.rs:586-593) to record exit, capture finish, account pin
  (util.rs:596-640); heap is the tokio runtime, several store opens, assembled context.
- `lf resume --all` (restore.rs:116) starts a fresh owner per interrupted Session regardless
  of whether anyone viewed it (restore.rs:41-60; Unknown liveness counts as running).

## Desktop

- `GhosttySurfacePool` has no cap or eviction (GhosttyTerminalView.swift:194-205); release only
  on provider switch, reset, tab close, child exit. Hidden panes stop drawing only. One pool per
  repository workspace (SessionsView.swift:392). `scrollback-limit = 10000000` bytes.
- Work watches: `WorkWatches` runs one watch per part (desktop plan --all, monitor ps, desktop
  plans, config planning status, desktop sessions, desktop task, history). On the installed
  macOS app these are served in-process through `loopflow-ffi` (`lf_subscribe`,
  rust/loopflow-ffi/src/lib.rs:3,46), not as child processes. WorkModel.swift:1341's comment
  ("one reader process") is stale.

## Cheaper architecture for the same commitments

1. Filter `graph_ancestors` to running Graph processes, or ask "does this owner hold a Session
   whose current step is a running Graph step".
2. Policy thread: log and retry on store errors; never die.
3. Owner idle loop: `sleep(5ms)` (transport.rs:853) = 200 wakeups/s per idle owner. Use a poll set.
4. Launcher: `exec` into the provider; the owner already `try_wait`s the child (transport.rs:811)
   and can write the exit/capture records.
5. Viewer stream encodes PTY bytes as JSON `Vec<u8>` (transport.rs:132-135, :357): 3-4x
   expansion per output frame. Use length-prefixed binary frames.
6. Pool eviction beyond N non-visible panes; repaint from the owner's snapshot on return.
7. Idle == dead today: the policy's end action is `kill_process(self)` (lifetime.rs:98-100)
   taking the agent with it. Goals 3 and 4 need the middle state (owner gone, snapshot kept).
8. Every `serve` and viewer opens the 497 MB `loopflow.db` with full schema validation
   (journal/mod.rs:791-796); every `--prepare` selection spawns a full `lf` write process.
9. One `lf session serve` per LF_HOME multiplexing N PTYs (Rex's model) would replace N owner
   processes and N idle loops with one; the socket, snapshot and viewer-token primitives exist.
