# Server protocols elsewhere: what bears on the server-first change

November 10, 2026. Source reading at pinned commits for Paseo (`71439e5`),
T3 Code (`71c2d3a3`) and Paradev (`61645f48`); nothing of theirs was run. Rex
(0.0.1793377904, api revision 8) from its help, its server's `/llms.txt`, its
web docs, read-only calls on the live server, and one isolated probe server
(started and stopped by PID; the live server instance was unchanged). Study
only: no code from any of them is used.

## Holder and upgrade

| | Holds terminals | Server restart or upgrade |
| --- | --- | --- |
| Paseo | Separate terminal worker, re-parented, run from a versioned copy under its home | Terminals survive; older workers are drained, never more than two versions |
| T3 Code | The server (node-pty, SDK agents in-process) | Turns and terminals interrupted; agents relaunched by provider resume id |
| Paradev | Separate tiny PTY daemon | Terminals survive; the daemon upgrades by handing PTY fds to its successor and keeps serving if the hand-off fails |
| Rex | The Rex server | "Stopping the server ends every program running in its sessions". Probe: after SIGTERM the child was dead; restart restored the session layout with a fresh shell and logged `not replaying creation options on restore` |

## Several viewers

| | Typing | Size | Late joiner |
| --- | --- | --- | --- |
| Paseo | Any viewer, no lock | One owner; claim only on real resize or focus; others render the owner's size | Server-side emulator serialized to ANSI, chunked, cut against live output by revision |
| T3 Code | Any viewer, no lock | Smallest attached viewer | Replay of retained output text |
| Paradev | Any viewer, no lock | Smallest visible, responsive viewer | Emulator mode preamble plus ring tail; when position is unknown, send nothing and ask the program to repaint |
| Rex | Any attached client, "Owning a block gives no other rights over it" | Follows the owner; `own_block` has take and request modes with a queue | Stream "starts with a snapshot of the terminal in Ghostty's snapshot format", then live bytes |

## Traps they hit

- Paradev's PTY daemon unlinks its socket path before binding. A CPU-starved
  daemon missed the health probe, a second one took the path, and live agents
  were orphaned (`DaemonSupervisor.ts:1092-1099`).
- Paradev and Paseo both put the server's address into the agent's environment
  for hooks; old agents went silent after a restart changed it.
- T3 Code keeps "this terminal was closed" in memory only, so an attach to an
  unknown id after a restart can spawn a fresh shell.
- Paseo's screen serializer lost cursor state and SGR mouse encoding and
  needed manual fixes.
- Paradev: smallest-viewer sizing let a pocketed phone pin every view until
  hidden and unresponsive clients were excluded.
- Paradev: a daemon-client disconnect is not PTY death; it verifies before
  marking agents dead.
- Paseo: loopback TCP with no password trusts every local OS user.

## Protocol shapes

- Rex: one JSON control WebSocket (`rex.control.v1`, 1 MiB cap) and one
  output-only data socket per block opened with a single-use, user-bound,
  60-second ticket. Input goes through control. Events carry a gapless
  per-connection `seq`; more than 8 MiB behind closes the connection "rather
  than skip events". Data resume by exact byte count within 60 s.
- Paseo: one WebSocket, JSON for RPC and events, binary frames for terminals
  (opcode + slot, 256 slots). A viewer past 4 MiB buffered gets a snapshot in
  place of deltas; a socket past 64 MiB is terminated.
- T3 Code: snapshot then deltas with `afterSequence`; replay capped at 128
  events and 1 MiB, else a fresh snapshot. Commands carry client-allocated ids
  and duplicates return the receipt. Version skew is one integer refused at
  connect.
- Paradev: terminal reconnect by `epoch:n` against a 2 MiB ring, answering
  exact, tail or reanchor. Slow client closed at 8 MB.
- Single instance: Paseo exclusive pid file published only when ready; T3
  Code an exclusive SQLite transaction the kernel releases on death; Paradev
  adopt-if-healthy else an `O_EXCL` spawn lock.
- Hooks: Paseo POSTs with a 500 ms abort and ignores every failure; Paradev
  curls an unauthenticated endpoint. Both install into the person's real
  provider settings. T3 Code has no shell hooks.

## Rex facts for a program in a pane

- Environment seen in the probe pane: `REX_SESSION`, `REX_BLOCK`,
  `REX_BIN_DIR`, `TERM_PROGRAM=rex`. `REX_SERVER` was set in the live pane
  but not the probe pane.
- `rex wait` returned 7 for a `--keep-open` block whose child ran `exit 7`.
- `--shell=none` starts the command directly. The shell block type takes
  `command`, `cwd`, `path_prepend`, `size`, `initial_input`; no per-block
  environment option.
- Defaults: `--max-sessions 64`, `--max-blocks 1024`, `--max-windows 64`,
  `--max-panes 64`.
- Block plugins exist (`--block-dir`) and creators can declare non-PTY blocks
  with their own data refs; the plugin protocol is undocumented.
- Linux and FreeBSD servers ship; redistribution terms unknown; beta.
- `rex whoami` printed `api: revision 0 (supports 0 to 0)` against the probe
  server while `server.status` returned 8; cause unknown.

Rex's own docs as fetched are in the session scratchpad, not this checkout.

## Rex across upgrades

From https://www.superlogical.com/rex/docs/linux/upgrading.md and
`…/linux/server-modes.md`, fetched November 10 (summarized by a fetch tool;
quotes as it returned them). Not observed: this Mac has had one Rex server
build (1696), installed and started 2026-10-30 17:49.

- Personal server: one a `rex` command started "stops on its own once it's
  idle, so it picks up the new version by itself"; one started with
  `rex server start` keeps the old version until stopped and restarted.
  Sessions survive the upgrade.
- Account servers behind a login server: "The servers the login server starts
  for each account keep running the version they started with." "After an
  upgrade, each one keeps the old version until its sessions end." The login
  server itself restarts on upgrade and "Each account's server keeps running
  when the login server stops, restarts, or is upgraded".
- Shared server: "A package upgrade restarts the service, which automatically
  ends every session on it."
- Any server: "Restarting the shared server ends every session and every
  program in them."
- Protocol side (`/llms.txt`): "an older client keeps working with a newer
  server"; `method_not_found` "happens when the server is older than the
  client"; unknown fields, message types and events are skipped; optional
  behaviours are announced in `features`.

Jack Heart: "i think generally we should be following rex though, how do tehy maintain terminals across rex server upgrades?"
