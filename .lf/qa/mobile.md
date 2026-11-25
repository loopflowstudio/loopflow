Read when the change touches phone access over Tailscale.

# Phone access against the Mac's real Tailscale

Use a fresh private `LF_HOME` and the same branch binary for all commands.
Put a `tailscale` script first on `PATH` that appends one line per invocation
to a private log. Start `lf server run` in that environment and wait for
`lf server status` to report ready before running `lf server mobile`.
This keeps the server under the test's control. Capture its PID for cleanup.

## Idle phone connection

Paste the reported HTTPS address into Loopflow on an iPhone signed into
the same Tailscale account. On a single-account Mac, an iOS Simulator can
use the same address after `touch "$LF_HOME/mobile-same-device"`. Open the
Task list and leave it connected for at least two minutes. Record readiness
and the trap's invocation count at both ends of the window.

Use local timestamps from `date '+%F %T'` for the start and end. On a quiet
Mac there should be zero Tailscale configuration records in this window.

```sh
/usr/bin/log show --info --start "$start" --end "$end" \
  --predicate 'process == "nehelper" AND eventMessage CONTAINS[c] "Tailscale"'
```

In a separate window, invoke the installed Tailscale CLI once by its full
path with `status --json` and discard stdout. Run the same log query for
that window to confirm it shows the configuration activity. Keep unrelated
Tailscale activity out of both measurements.

Run `lf server mobile --off`. The phone disconnects and the local server
keeps its instance. Stop the private server by its captured PID.

## Daemon outage and recovery

The `server_mobile_changes_preserve_local_connections_and_saved_disable`
transport fixture exercises an outage without interrupting the Mac's tailnet.
It holds a phone connection and local subscription open, changes the daemon
owner, stops the private socket, and brings the approved owner back at the
same address. The virtual-clock test in `server/remote.rs` holds requests
open to exercise status and cancellation while the daemon is unresponsive.
