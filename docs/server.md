# Connect clients

```bash
lf server status   # check local client server readiness and any connection error
lf install         # update Loopflow and keep its server registration in sync
```

Loopflow Desktop and held Task conversations connect through a local client
server on your Machine. `lf install` registers and updates it automatically:
no manual daemon setup or Tailscale configuration needed for local use.
`lf server status` is read-only: it reports readiness, the socket path, and the
running build as JSON without installing or modifying anything. `ready` means
the server answers at a protocol revision this `lf` speaks, whichever build is
asking; `executable_sha256` names the server's own executable.

## Pick the right command

| You want to… | Use |
|---|---|
| Run work on another computer | `lf machine add mini`, then `lf --machine mini …` |
| Check Desktop's connection to Loopflow | `lf server status` |
| Install or update Loopflow and its server | `lf install` |
| Focus a Desktop window or script its panes | `lf desktop` |

Machine commands choose **where work runs**. The client server lets local
clients (Desktop, `lf open`, `lf resume`) reach Work and held conversations on
this Machine. Ordinary CLI commands work without the server; opening a Task or
Wave conversation is the exception because the server starts its held agent.

Connected views share the server's live observations of processes, Git state,
PR/CI status, and repository files, updating automatically as things change on
disk or on GitHub.

## Keep working through a connection loss

- **Lists and edits.** When Desktop reconnects, live Work subscriptions resume
  automatically from the last reading (or refresh cleanly if the server
  restarted). Task edits (Complete, Claim, Move) show immediately and retry
  under the same request ID so each runs at most once. After ~10 seconds
  without the server, edit controls disable until it returns
  ([Changes show at once](surfaces.md#changes-show-at-once)).
- **Conversations.** Agents run in a **holder** process beside the server, not
  inside it. When `lf install` restarts the server or a connection drops,
  agents keep running, Desktop shows **Reconnecting…**, input pauses, and the
  pane catches up from buffered output once reconnected. Selection, copy, Find,
  and scrollback stay usable throughout. Click **Retry connection** if automatic
  retries stop, or **Refresh session** in the pane strip if the status
  connection is lost; neither restarts the agent or replays uncertain
  keystrokes.
- **Closing Desktop.** Quitting Desktop leaves the server, holders, and running
  Graphs untouched. After a logout or reboot, the server automatically restarts
  active Task work sessions and runs
  ([after a kill or reboot](sessions.md#auto-recovery-after-a-kill-crash-or-reboot)).

## Recover a submitted creation

Desktop saves Task creations, **Create Task from this conversation**, and
new-thread requests locally before sending them. If Desktop quits before seeing
the reply, reopening checks the server's receipt on the original Machine and
offers **Show result** once confirmed (never duplicating the Task or sending it
to a different repo).

- **Task created; start unconfirmed:** the Task exists, but launch confirmation
  was lost. Inspect its status and Graph history before starting anything.
- **Conversation change unconfirmed:** a new-thread request was interrupted
  while stopping the previous provider. Use **Inspect** to check its receipt;
  recovery never repeats the stop.

CLI scripts get the same guarantee with `lf task create --request-id <id>` and
`lf server receipt <id> --json`.

## Updates and disabled background access

`lf install` updates `lf` and restarts the server without ending any running
conversations (existing agents keep running on their build; new conversations
use the updated build). `lf server run` is the supervisor entrypoint, not a
command you need to run by hand.

If macOS disabled Loopflow's background item, re-enable it in System Settings
(or re-enable the user service on Linux). Status checks and Retry respect that
setting and never re-register it behind your back.

## Phone access

```bash
lf server mobile
# {"enabled":true,"endpoint":"https://mac.tailnet.ts.net:51842","owner":{"node":"…","user":12345}}

lf server mobile --status
# {"configuration":{"enabled":true,"endpoint":"https://mac.tailnet.ts.net:51842","owner":{…}},"ready":true,"error":null}

lf server mobile --off
# Mobile access is off
```

A phone signed into the same Tailscale user can attach to running Sessions on
this Machine. Setup requires a signed-in, user-owned Tailscale node (not a
tagged device) with MagicDNS and HTTPS certificates enabled on the tailnet.

`lf server` talks directly to Tailscale's LocalAPI without launching the
`tailscale` app or CLI. It discovers the macOS App Store app, Standalone macOS
Tailscale, or `tailscaled` over its Unix socket (`/var/run/tailscaled.socket` on
macOS, `/var/run/tailscale/tailscaled.sock` on Linux, or `TS_SOCKET` when set in
the server's environment).

If Tailscale stops, signs out, or changes owner, the server closes active phone
connections and marks mobile access unavailable while local Sessions keep
running. Once Tailscale returns on the approved account, the saved endpoint and
QR address come back automatically (retrying at 5, 10, 20, then every 30
seconds); `lf server mobile --status` answers immediately from the last state
without waiting on reconnection.

Enabling local server access or adding SSH Machines does not enable phone
access. For local-only use, no network port is opened; keep the local Unix
socket private.

Next: [Surfaces](surfaces.md) · [Connection troubleshooting](troubleshooting.md#desktop-cannot-connect).
