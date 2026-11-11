# LOO-824: hold managed Sessions' PTYs beside the server

Stacked on LOO-819 (#1875). The direction is `docs/dev/server-first.md`; the
Task brief's "Decided, do not reopen" list is not repeated here. LOO-819's
scratch (quotes, measurements, receipts) is in git at `e65ddaed7:scratch/`.

## Outcome

Ten Task Sessions open: `ps` shows one `lf server run`, one `lf holder run`,
ten agents and no `lf session serve --terminal`. `lf install` mid-turn: the
turn keeps streaming, Desktop panes and `lf resume --task` terminals say
Reconnecting… and come back repainted. `kill` the server: agents keep running
(and keep writing into the holder's ring); launchd restarts it; views return.

## Said, relevant here

Jack Heart, LOO-819 conversation (full set at `e65ddaed7:scratch/server-model.md`):

- On several old holders staying alive: "im ok with that"
- Emulator in the server, holder keeps only raw bytes: "Yea the second"
- Size: "Owner sets the size. I would say that we switch owners on viewer open close; any new viewer becomes the owner riht away?" (later Task, not this cut)

## Probes, November 10 2026

**Holder survives the server's service (macOS).** A launchd agent in
`gui/$UID` whose child called `setsid()` and slept. `launchctl bootout`: the
setsid child survived both with and without `AbandonProcessGroup`; with it,
the job's own non-setsid child survived too (reparented to 1). The shipped
`com.loopflow.server.plist` already sets `AbandonProcessGroup`; the systemd
unit already has `KillMode=process`. Not probed: SMAppService unregister
(same launchd job underneath), Linux (left to the Linux QA rental at gate).
Logout and reboot end holders, as they end owners today.

**Size nudge repaints the whole visible screen.** Harness: agent idle in a
40×120 PTY, full stream into a pyte emulator; then resize to 119 cols (and
separately 39 rows), 0.4 s, back to 40×120, 3 s; a fresh emulator fed only the
post-nudge bytes compared row by row with the full one.

| Agent | Screen | Post-nudge bytes | Non-blank rows matched |
| --- | --- | ---: | ---: |
| Claude Code 2.1.322 | idle start | 1,631 | 9 / 9 |
| Claude Code 2.1.322 | after a 30-line answer | 1,519 | 36 / 36 |
| codex-cli 0.167.0 | idle start | 3,995 | 23 / 23 |
| codex-cli 0.167.0 | after a 30-line answer | 1,718 | 34 / 34 |

Cursor position matched in every case. Scrollback above the screen is not
repainted (both print history inline), and pyte does not model terminal
modes, so whether either re-sends bracketed paste, focus or Kitty keyboard
modes on resize is unknown. Script: re-creatable from this description.

## Chosen approach

### Processes

```text
lf resume --task / Desktop pane      lf server run (launchd/systemd)          lf holder run (setsid, per build)
  relay ── session.attach ticket ──>   owner loop per held Session  ──read──>   PTY + child + 2 MiB ring
                                        (viewers, Ghostty, policy,  ──write─>   exit.json on child exit
                                         input retention, settle)    resize
```

The owner loop that `lf session serve` runs today (`transport::serve`:
viewers, spool, Ghostty terminal, lifetime policy, input retention, the
child's control requests, exit settlement) runs **inside the server**, one
thread per held Session, still listening on `endpoint(owner)`. Only the PTY,
the child and the child wait move to the holder. The viewer protocol,
Desktop's bridge (`ClientMessage::Attach` → `endpoint(owner)`), relays and
their recovery stay as they are; the later "every view types" Task replaces
them. This is the cheapest cut that removes the per-Session process without
touching the viewer surface.

`transport::serve` gets its child as an enum:
`Child::Local { pty, process }` (bare launches, pipes, Desktop shells) or
`Child::Held(HeldPty)` (a holder stream). `Local` is deleted with the owner
process by LOO-820 (bare direct) and the CLI-cleanup piece; it is not a second
implementation, it is the bare path this Task must not change.

### The managed rule

`transport::managed(launch: &Launch) -> bool`, one function, called from
`delegate` and from the three direct `transport::start` callers:

| Launch | Managed |
| --- | --- |
| `lf task run`, `lf ship` interactive step, `lf resume --task`, Task primary conversation (`ops/human_session/primary.rs`), Wave session commands | yes |
| Desktop pane (`LF_SERVER=1` on the prepared command) | yes |
| `lf resume SESSION` of a Session bound to a Task or Wave | yes (assumption, `questions.md`) |
| `lf <skill>`, `lf graph …`, `lf -i`, any `-b`/piped execution | no |
| Desktop Task shells (`desktop::shells`) | no: they run without a server on headless Linux; they keep `Local` |

A managed launch asks the server (`session.start`, below) and becomes a
server-bridged view of the returned owner. Server unreachable: the launch
fails with `Loopflow server is not running; run lf server status`. No local
owner fallback.

### Holder

`lf holder run` (hidden), the same `lf` binary. It opens no store, links no
tokio, parses no Sessions.

- **Identity.** One per executable SHA-256 (the server already computes
  it). Socket `<private dir>/holder-<sha12>.sock`, lock beside it, state
  under `<LF home>/holders/<sha12>/<pty>/`.
- **Claim.** `try_lock_exclusive` on the lock, held for life; only the lock
  holder unlinks and binds the socket. Failing the lock means a holder of this
  build is alive: exit 0. A CPU-starved holder keeps its lock, so a second
  spawn exits and nothing is displaced. The server's "is it there" check is
  connect-with-deadline; on timeout with the lock held it waits, never
  respawns into the path.
- **Start.** The server spawns it with `setsid` and a reaper thread (the
  `transport::start` pattern) and reads one ready line. launchd and
  `KillMode=process` then leave it alone when the server's service stops.
- **Lifetime.** Exits 60 s after its last child exits with no `start`. Old
  builds drain naturally; the server sends new starts only to its own build.
- **Ring.** 2 MiB of raw output per PTY, positions are absolute byte counts.
  The holder drains every PTY continuously, so a dead server never blocks an
  agent's writes.
- **Exit.** On child exit the holder writes `exit.json`
  `{code, signal, at, written}` atomically, then reports it to readers.
  Nothing else is persisted by the holder.

Protocol, revision 1, framed like the viewer wire (`tag u8 · len u32 BE ·
payload`), JSON control payloads:

| Message | Answer |
| --- | --- |
| `hello {revision}` | `welcome {revision, build, pid}` or `incompatible` |
| `list` | `[{pty, pid, rows, cols, written, ring_start, exit}]` |
| `start {pty, argv, cwd, env, rows, cols}` | `{pid}`; `pty` is the owner LfProcessId, `env` is complete (no inheritance from the holder) |
| `read {pty, from}` | the connection becomes that PTY's stream: `gap {ring_start}` if `from` is older, `out` frames, `exit {…}` last |
| `write {pty}` + input frames on a stream connection | — |
| `resize {pty, rows, cols}` | — |

Signals (policy exit, abandon) are sent by the server to the child's process
group directly: same uid, pid from `list`. No `signal` message.

A dropped holder connection marks nothing: the owner loop retries `read`
from its last position with backoff and only a `exit` frame or a successful
`list` without the PTY (plus `exit.json`) ends the Session.

### Server side

- `session.start {argv, cwd, env, rows, cols, session}` → `{owner_lf_process_id}`
  in protocol revision 5, shared with the other server-first pieces.
- The server writes `<pty>/meta.json` (`session`, control key, last size,
  owner id) before `start`, so a restarted server can rebuild the loop and
  answer the child's control requests (`ExitRecord`, `Status`, `Directory`,
  `NativeStarted`, `InputHint`) with the same key. `PARENT_ENV` is the
  holder's pid, so `Handoff::take`'s direct-child check holds.
- An accepted `ExitRecord` is written to `<pty>/exit-record.json` before the
  `Accepted` reply. Server down when the launcher hands it over: the request
  fails, the launcher does not exec and waits beside the provider as it does
  today when an owner is busy; that launcher settles its own exit.
- The owner Process row's pid is the holder's pid. It completes when the
  PTY's `exit` is settled, not when a server dies.
- **Settlement.** Live: the owner loop gets `exit`, settles as now
  (`native_exit::settle` with the stored record, else plain Process end),
  then deletes `<pty>/`. Server down at exit: on startup the server scans
  `holders/*/*/exit.json` and settles each the same way. Settled is the
  deletion of the directory, so a crash mid-settle repeats it (settle is
  idempotent on an ended Process; implement checks this).
- **Startup.** For each `holder-*.sock`: `hello`, `list`, and per live PTY
  start an owner loop with the same owner id. The emulator is rebuilt from
  `screen.ghostsnp` + ring when present, else ring + one nudge.
- **Graceful stop (SIGTERM from install).** Each owner loop writes
  `<pty>/screen.ghostsnp` (the existing snapshot format, header extended with
  the byte position it covers) and stops. Restart loads it and replays the
  ring from that position: exact screen and modes, no nudge. A crashed server
  has no snapshot; ring replay plus a nudge (cols−1, 0.4 s, back) repaints the
  visible screen per the probe; earlier scrollback and modes set before the
  ring's start may be absent.

Relays of held owners always take the server route (`session.attach`
ticket), including CLI `lf resume --task`, because that route already
reconnects with backoff and refreshes when its spool position expired. The
direct-socket CLI route stays for `Local` owners.

## Risks and answers

| Risk | Answer |
| --- | --- |
| Service manager kills the holder with the server | Probe: setsid child survives `bootout`; plist and unit already abandon children |
| Second holder steals the socket | Kernel lock; unlink only under it; spawn is idempotent |
| Server's launchd environment lacks the person's PATH/auth env | `start` carries the caller's complete environment; server-initiated starts (`primary.rs` under served `task.run`) use the environment the served operation already uses for Graph launches; implement confirms which |
| Output while server is down blocks the agent | Holder drains into the ring; ring loss is only scrollback |
| Lost modes after a crash restart | Snapshot on SIGTERM covers installs; crash path unknown, see probe |
| One panicking owner loop takes the server down | Each loop thread runs under `catch_unwind`; the Session stays held and the loop restarts from the ring |
| 95 loops in one process | Today's loops idle in `poll`; threads, not processes. Measure `ps` RSS/CPU at gate against today's 95 × `session serve` (4.4 GB, 55%) |

## Alternatives set aside

- **Pass the PTY master fd to the server (SCM_RIGHTS).** No byte protocol,
  but a dead server leaves the PTY undrained and the agent blocks; and no ring
  for repaint.
- **Server-native views on `machine.sock` now.** Right end state, but it is
  the next Task's (every view types, size follows the newest view); doing it
  here rewrites relays twice.
- **A launchd/systemd job per holder.** Survives on its own but needs a plist
  per build and SMAppService registration; setsid already works.

## Scope

In: holder process and protocol; `Child::Held` in the owner loop; owner loops
hosted by the server; `session.start`; the managed rule; restart rebuild;
offline settlement; docs (`transport/README.md`, `server/README.md`,
`docs/dev/server-first.md` Open list).

Out: bare launches (LOO-820), size ownership and multi-typing (next Task),
Desktop shells and piped/`-b` executions (stay `Local`), retiring the 95
running owners (leaked-process Task), `session serve`/`session attach` CLI
removal (cleanup piece).

## Delete in this Task

- Managed callers of `transport::start` / `delegate*`:
  `ops/human_session/primary.rs:200`, `restore.rs:73`, the managed branches of
  `human_session.rs:1086,1209,1306`, `lf.rs:2315` for `task run`. They go
  through `managed` → `session.start`.
- Direct-socket attach for held owners (`attach_existing` on a held owner).
- "Background starts stop at sixteen running Task conversations" stays (a
  policy, not a process limit) unless gate measurements say otherwise.

## Forbidden

- A managed launch that silently falls back to a local owner.
- Unlinking a holder socket without its lock.
- The holder reading or writing the store, or interpreting Sessions.
- A server restart ending a Session because a holder connection dropped.

## Slices

1. **This slice: holder.** `lf holder run`, lock/claim, start/read/write/
   resize/list, ring, `exit.json`. Focused test: spawn two holders of one
   build (second exits 0), start `sh -c 'printf x; sleep 1; exit 3'`, read
   from 0 gets `x` and `exit {code:3}`; SIGSTOP the holder and spawn again:
   the stopped one keeps the socket.
2. `Child::Held` in `transport::serve`, owner loop hosted as a server thread,
   `session.start`, the managed rule and its callers.
3. Restart: `meta.json`, rebuild from list/ring/snapshot, nudge, offline
   settlement, SIGTERM snapshot.
4. Docs, revision bump with the other pieces, `server_schema.json`.

## Acceptance at gate (headless)

```sh
uv run python scripts/test.py --reuse-passing --rust-filter 'test(holder::) | test(transport::) | test(server::)'
```

Integration test `held_session_survives_server_restart` (in
`tests/integration.rs`, isolated `LF_HOME`, fake agent script printing a
counter every 100 ms):

1. Start a server fixture; `session.start` the fake agent; attach a viewer;
   see counter lines. `ps` of the fixture's process group: one server, one
   holder, the agent, no `session serve`.
2. SIGTERM the server; agent pid still alive; counter keeps advancing in the
   holder (`read` position grows).
3. Restart the server; viewer reconnects; its screen contains the newest
   counter and a counter value printed while the server was down.
4. SIGKILL the server; the agent exits 3; restart: Session ended with code 3,
   `<pty>/` gone.

Linux: same test on the RunPod rental with the user unit's `KillMode=process`.

## Done when

`ps` with ten open Task Sessions shows one server, one holder, ten agents and
no `session serve`; an `lf install` mid-turn and a server kill both leave the
turn running and every view repainted; a Session whose agent exits while the
server is down reads ended with its real code once the server is back.
