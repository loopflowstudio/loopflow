Read when the change touches `lf server mobile`, the Tailscale LocalAPI client or the phone listener.

# Phone access against the Mac's real Tailscale

The App Store Tailscale runs as `IPNExtension`; the branch finds it in-process.
A private `LF_HOME` reaches the real tailnet: `lf server mobile` saves an
approval for this Mac's node and binds the tailnet addresses at a new port.
Put a trap `tailscale` script first on `PATH` that appends to a log; it must
stay empty.

Starting a server from `lf server mobile` (or any command) in a private home
registers a KeepAlive launchd job `com.loopflow.server.<hash>` whose plist is
`$LF_HOME/server.plist`. Killing the PID respawns it, and its environment
carries `LF_HOME` only, not `TS_SOCKET`. To run the server yourself, first
`launchctl bootout gui/$(id -u)/com.loopflow.server.<hash>` (hash from
`lf server status`'s socket path), then start `lf server run` detached with
closed stdin. Boot the job out again when done.

## A phone from this Mac

A connection from this Mac is the owner's own node, which admission refuses
(TLS completes, then `curl` reports an empty reply). `touch
$LF_HOME/mobile-same-device` admits it. Drive the WebSocket with
`uv run -q --with websockets python -I phone.py wss://<endpoint>`: send
`{"type":"hello","revision":9,"min_revision":8,"client":{"kind":"ios","platform":"ios","name":"QA","version":"0.16.0"}}`
to `/v1/machine`, expect `welcome`, then a `plan.list` request.

## Stopping the daemon without touching the real tailnet

Quitting Tailscale cuts the Machine's other connections. Instead start the
server with `TS_SOCKET=<path>` and serve that path with a small Python proxy
that adds `Authorization: Basic base64(":<token>")` and forwards to
`127.0.0.1:<port>`; read `sameuserproof-<port>-<token>` from
`lsof -nP -p $(pgrep -x IPNExtension)`. Log each request line: bind reads
`status` and `cert/<dns>`, maintenance reads `status?peers=false` every 5 s.
Kill the proxy for an outage, restart it for recovery. A socket that accepts
and never answers stands in for a hung daemon.

## nehelper churn

Bracket an idle window with `date '+%F %T'` and count
`/usr/bin/log show --info --start … --end … --predicate 'process == "nehelper" AND eventMessage CONTAINS[c] "Tailscale"'`.
One `"/Applications/Tailscale*.app/Contents/MacOS/Tailscale" status --json`
in a separate window is the control: it logs a configuration delegate and two
removals.
