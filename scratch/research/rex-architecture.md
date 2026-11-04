# Rex architecture (2026-11-02, research agent; Rex Beta 0.1.0 build 1696, API revision 8)

Measured on a throwaway server unless noted. Jack's real server and app were only read.

## Process and memory
- One Go server owns every PTY; shells are `/usr/bin/login … rex-login zsh` children. Rex.app
  and the `rex` CLI are clients. Server footprint: 18 MB empty, 20 MB with 1 block, 28 MB with
  9, 38 MB with 29: about 0.7 MB per idle terminal. Shells themselves are ~9.4 MB each. One
  block after 14 MB of output: +76 MB (libghostty pages); scrollback is real memory up to 64 MiB.
- **Idle suspend (inference from symbols and files):** `mux.(*Terminal).Suspend`,
  `armIdleTimerLocked`, `suspendLocked`, `restoreIfSuspendedLocked`, `dropIdleCachesLocked`.
  About a minute after creation with no data connection, 28 of 29 blocks had a sealed
  `state/terminal/<server pid>/<uuid>.snap` (REXSEAL, 777 bytes for an idle shell) and the
  libghostty state is presumably freed; attaching restores it. Deleted on shutdown: an
  in-process memory trick, not restart persistence. Error string: "terminal snapshot unreadable,
  recovering with an empty terminal; scrollback is lost".
- Rex.app with 6 blocks: 631 MB footprint (269 MB graphics, 214 MB MALLOC_SMALL, 58 MB
  IOSurface). Symbols `TerminalReplica`, `LiveTerminal`, `visiblePaneIDs`, `occludedPaneIDs`
  suggest a replica per block and rendering only visible panes.

## Protocol
- Endpoints on one socket: `/api/control` (WebSocket `rex.control.v1`), `/api/data`
  (`rex.data.terminal.v1`), `/api/help` (JSON Schema), `/llms.txt` (the guide for this server).
- Handshake `hello` → `welcome` (`max_message_bytes` 1 MiB, client id, reconnect token,
  `resumed`). Server translates for revisions 0–8. `session.bootstrap` is the app's one first
  call (status, session list, attach, data tickets for visible blocks).
- Events carry gapless `seq`; each response carries the `seq` of the last queued event so the
  client applies the result, drops older events, applies the rest; the view has a `revision`.
- Attach: `data.connect` → 16-byte one-shot ticket (60 s, same uid); binary HELLO; WELCOME
  with reconnect token, grace, 64-bit nonce; then a GHOSTSNP snapshot (1.33 MB for 14 MB of
  scrollback, in 64 KiB messages), an APC `ESC _ rex1;<nonce>;{"type":"resize"…} ESC \`,
  optional `theme`, then raw PTY bytes; `child_exited` APC at the end. Two clients both get
  the snapshot and both see typed text. RESUME = token + bytes_received; the server holds
  the stream 60 s and continues from the exact byte, no snapshot re-sent.
- Flow control: 8 MiB event backlog then close 1008; data clients that fall behind the spool
  get 1013 "data reconnect required" (new ticket, new snapshot). Pings every 120 s.

## Persistence
- `snapshot.db`: meta (`clean_shutdown`, `host_id`, build), sessions (layout JSON), blocks
  (creator, flavor, label, placement, options blob). Per block `state.json`
  `{process_cwd, size}`. Restart after SIGKILL or clean stop: every block back as a fresh
  login shell in its cwd in ~200 ms, empty screen, owner null. SIGKILL killed all 29 shells.

## Ownership
- Any attached client types. `session.own_block mode=take|request` decides only size and
  colors; a `request` queues behind the owner and is granted when the owner disconnects
  (verified). Dropped owners keep blocks 60 s (`lost` → `restore`). Focus is per window,
  separate from ownership.

## Agents
- Block types: `terminal.shell` and `onboarding`; plugin blocks via `--block-dir`.
- OSC 7501 Program Status: states idle|working|done|blocked|error|clear, kinds
  permission|question|auth; working/blocked drop on exit or next OSC 133 prompt. Shown in
  the session picker and tab headers. `rex skill setup` installs a Claude skill and a
  SessionStart hook: REX_SESSION/REX_BLOCK, rename the session after its task, the user may
  be viewing another pane, never type into your own pane.

## Copy / don't copy (for terminals that are mostly idle 200–650 MB agents)
Copy: ownership as a size/colors arbiter with take/request and owner-passing; seq+revision;
snapshot-then-stream with nonce-tagged in-band resize/theme/exit; one-shot tickets, byte-counted
RESUME, reconnect tokens; restore = recreate in cwd, never replay; idle suspend of unwatched
terminals to disk; OSC 7501 as the agent-state signal; `/llms.txt`.
Don't copy: shells dying with the server (agents must survive a Desktop or server restart, so
PTY ownership lives outside anything that restarts often); Rex.app's 631 MB for 6 panes; hard
`--max-*` refusals; persisting 64 MiB terminals (the provider transcript is the record).
