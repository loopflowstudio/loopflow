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
clients (Desktop, `lf open`, `lf resume`, browser review pages from
`lf page open`, the phone) reach Work and held
conversations on this Machine, and the Tasks that run on your other Machines:
it opens a Task's files, changes, shells and Sessions on the Machine that owns
it, over SSH connections it keeps open for a minute after use. Lists and the
board read only this Machine. Ordinary CLI commands work without the server;
opening a Task or Wave conversation is the exception because the server starts
its held agent.

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

## Browser review pages

Beside the private Unix socket, `lf server` listens on a loopback-only TCP port
(`127.0.0.1`, the same port each start when it is free) with a 32-byte random
token held only in memory. Running
`lf page open scratch/ship-review.html` outside Desktop asks the server over
the local Unix socket for a capability URL
(`http://127.0.0.1:<port>/<token>/f/<path>`) and opens it in your browser so
`⌘↩` and action buttons speak directly to `lf server` over WebSocket.

- `lf server status` never prints the token, and the token is never written to
  disk.
- Restarting or stopping `lf server` (including during `lf install`) rotates
  the token. Any open browser tab shows a banner
  (`Loopflow server disconnected — run lf page open <path>`) and falls back to
  copying replies to the clipboard until reopened. The port stays the same, so
  the reopened page keeps your unsent notes and the receipts of actions it ran.
- Loopback serving covers checkouts on this Machine. For a Task owned by
  another Machine, open the page in Loopflow Desktop.

See [Review pages](surfaces.md#review-pages) and
[Security](security.md#local-client-server).

## Phone access

```bash
lf server mobile            # approve this Machine and print its HTTPS address
lf server mobile --status   # show the saved approval and current readiness
lf server mobile --off      # close phone connections
```

A phone signed into the same Tailscale account opens your Tasks with four
views: **Session**, **Files** (the whole tree, changed or not), **Changes** and
**Shell**. One pairing covers every Machine: a Task running on mini or a rental
opens through this server, with **Not started** before it has a checkout and
an error naming the Machine, with **Try again**, when that Machine doesn't
answer. When a Task moves, its views reload from the new Machine. A Shell
already open keeps running on the old one; reopened after the move, it shows
ended and offers **New shell**.

In Desktop, open **Settings → Mobile** (**⌘,**) to toggle phone access, check
readiness, or scan the pairing QR code with your phone. The green sidebar dot
and pairing code follow the same status while Settings is open.

Setup requires a user-owned, untagged Machine with MagicDNS and HTTPS
certificates enabled. For a custom daemon socket, set `TS_SOCKET` for the
setup command and for an `lf server run` you start yourself. Service
installation does not forward the setup command's `TS_SOCKET`.

The server checks Tailscale directly through its LocalAPI. If Tailscale stops,
signs out, or changes owner, phone connections close and local Sessions keep
running. When the approved account returns, the saved QR address works again.
Retries start after five seconds and back off to once every 30 seconds.

Local server access and SSH Machines work independently of phone access.
Without phone access enabled, the server listens only on the private Unix
socket and loopback `127.0.0.1` (never on a non-loopback network interface).

Next: [Surfaces](surfaces.md) · [Connection troubleshooting](troubleshooting.md#desktop-cannot-connect).
