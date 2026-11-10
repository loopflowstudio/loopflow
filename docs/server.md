# Connect clients

```bash
lf server status   # local server readiness and any connection error
lf install         # update the selected Loopflow installation
```

Open Desktop and connect to your Work. Installation prepares its local server
automatically; you do not need to start it in another terminal or set up
Tailscale. `lf server status` reports readiness, the socket and the running server
instance as JSON. It observes without installing or repairing anything.

## Pick the command for the job

| You want to | Use |
|---|---|
| Run work on another computer | `lf machine add mini`, then `lf --machine mini …` |
| Check Desktop's connection to Loopflow | `lf server status` |
| Install or update Loopflow and its server | `lf install` |
| Focus a Desktop window or arrange its panes | `lf desktop` |

Machine commands manage execution destinations, their connections and capacity.
The server lets clients reach Work and existing Sessions on a host. Installation
owns its registration and updates. Desktop's window automation stays with the app.
Desktop uses this connection for all shared Work reads, changes and live updates.
Ordinary CLI commands can run without the client server.

## Keep editing through a connection loss

Keep a Task list open while Desktop reconnects: live Work updates resume
automatically from the last retained reading. An unchanged list does not need
to be downloaded again. If Work changed or the server restarted, Desktop receives
a complete current reading. If recovery fails, the window reports unavailability
and retries. Task and Session creations recover through their saved request and
server receipt. Other changes submitted before the connection dropped are never
repeated automatically; check the current Work before submitting them again.

Desktop reconnects automatically to the same Session owner when the server is
replaced or its connection drops. Its native status row shows **Reconnecting…**;
the screen stays visible and input pauses until the connection is acknowledged.
Selection, copy, Find and scrolling remain available.

If another viewer has control, recovery leaves that viewer in control. The
read-only pane offers **Move here** to request control explicitly. **Retry**
repairs the connection after automatic retries are exhausted. It does not take
control, restart the provider or repeat a command. Uncertain keystrokes and
unacknowledged commands are never replayed automatically.

An unavailable connection does not mean the Session has ended. Closing Desktop
leaves the server and existing execution owners independent of the window.
Session lifetime rules still apply: drafts, pending requests, active turns and
running Graphs retain their owners; settled, unviewed Sessions can end after
their configured grace. See [Sessions](sessions.md#come-back-to-a-conversation).
Logout or reboot ends live processes; [resume from saved history](sessions.md#after-a-crash-or-reboot)
afterward.

## Recover a submitted creation

Submit once. When the Task or conversation appears, Desktop clears the saved
submission. If Desktop closes before showing the result, reopening recovers it
on the original Machine. **Show result** displays the recovered Task or Session
and clears the submission after it appears. Switching repositories or opening
another window does not submit it somewhere else.

This recovery also covers **Create Task from this conversation** and starting a
new thread in an existing Session. Desktop saves each submitted request on this
device before sending it. If saving fails, the form stays open with the error
and nothing is sent. This saves submitted creations, not an editable offline
queue; submissions do not sync to other devices.

The server's receipt determines what happened. Desktop checks it after reconnect
or app restart, and sends the identical creation again only when the server
confirms it is absent or safe to continue. An unavailable server leaves the
submission unresolved. Once Desktop presents a confirmed result, it removes the
saved request.

**Task created; start unconfirmed** means the Task exists but its launch has not
been confirmed. Open the Task and inspect its status and Graph history before
starting anything. Desktop keeps this submission available for inspection and
does not launch it again. Recovering a receipt for a removed Task does not
recreate it.

**Conversation change unconfirmed** means a new-thread request was interrupted
while stopping the earlier provider. **Inspect** shows the conversation and its
receipt; recovery does not repeat the stop. Newer input also prevents finishing
an older unpublished conversation change.

Other changes have no automatic retry contract. If their reply is lost, refresh
the affected Work to see what happened before making another change.

## Updates and disabled background access

Use `lf install` to update the selected build. Installation reconciles the server
registration; Desktop reconnects through the update. `lf server run` is the
foreground entry used by the OS supervisor, not a routine setup step.

If macOS has disabled Loopflow's background item, enable it in System Settings.
On Linux, disabled or masked user services likewise require explicit enablement.
Status checks and Retry respect that choice. They do not repeatedly register a
helper. For a mismatched build or another startup error, read the diagnosis from
`lf server status` and check the installation with `lf doctor`.

## Phone access

This version provides local client access. Authenticated remote access and the
native phone client are separate work in LOO-718; local server readiness alone
does not mean a phone can connect. Pairing a phone will grant client access to a
host, independently of adding a Machine as an execution destination.

Local use needs no network listener. Keep the local socket private; remote
clients require their own authenticated endpoint.

Next: [Desktop and other surfaces](surfaces.md) · [Connection troubleshooting](troubleshooting.md#desktop-cannot-connect)
