# LOO-1080 · Quiet Tailscale access on macOS

An owner can leave mobile access enabled, open a Task from their phone and keep working. Whether Tailscale is running, signed out or quit, `lf server` launches zero Tailscale commands and causes zero `nehelper` configuration saves or removals. The same approval and QR address work after the daemon returns.

## What the running Mac showed

On November 23, macOS 26.7.4 had one same-user `IPNExtension` proof descriptor. An authenticated loopback GET returned 200 for full status and TCP WhoIs. Full status identified the running node and its owner. `status?peers=false` also returned 200, returning `BackendState` and `Self` without the `User` map needed by `owner_from_status`'s `LoginName` check. Unified logs on this Mac included both “Created a new configuration delegate with name = Tailscale” and “Tailscale is removing configuration” from CLI discovery probes. The branch-server churn measurement belongs at gate.

LOO-718 (#1812) introduced the five-second owner check so logout closes **already connected** phones. LOO-809 (#1897) bound terminal and bulk tickets to the device identified on each connection. Merely slowing the existing timer leaves the GUI launch in place. Keeping a macOS CLI fallback still launches `Tailscale.app` and triggers `nehelper` every retry whenever Tailscale is quit or stopped (when no live `IPNExtension` exists), and even a `PATH`-only fallback hits `/usr/local/bin/tailscale -> Tailscale.app`. Removing maintenance leaves admitted connections alive. Both obligations belong in the demo. See `rust/loopflow/src/server/{mobile,remote,ticket}.rs` and `rust/loopflow/tests/transport_tests.rs`.

## One Tailscale LocalAPI transport

Every Tailscale distribution on macOS and Linux exposes the same LocalAPI; the `tailscale` CLI is a client of that API with no separate backend. Replace the CLI subprocess transport in `mobile.rs` with one async LocalAPI client and delete all Tailscale subprocess execution (`TAILSCALE_BE_CLI`, `.app` binary launching, CLI fallback cadences and dual backoff schedules).

1. **Discover the LocalAPI directly, once per bind, with zero subprocesses.**
   - **macOS App Store (`IPNExtension`):** inspect same-UID `IPNExtension` open vnode descriptors in-process via `libproc` (`PROC_PIDLISTFDS` + `PROC_PIDFDVNODEPATHINFO`, matching `loopflow-execution/src/process/os_process/macos.rs`) for `sameuserproof-<port>-<token>`. Proof filenames and tokens never cross a pipe or child process.
   - **macOS Standalone:** read the port from `/Library/Tailscale/ipnport` (symlink target, matching `safesocket_darwin.go`) and the token from `/Library/Tailscale/sameuserproof-<port>`.
   - **Unix socket (`tailscaled`):** check `TS_SOCKET` when set, then `/var/run/tailscaled.socket` on macOS and `/var/run/tailscale/tailscaled.sock` on Linux, using `reqwest`'s native Unix socket support.
   - **HTTP hygiene:** for extension loopback TCP, connect strictly to literal `127.0.0.1:<port>` with `Host: local-tailscaled.sock` and empty-user Basic auth (`:<token>`). For Unix sockets, use `Host: local-tailscaled.sock` with no auth header. Disable proxies (`.no_proxy()`) and redirects (`redirect::Policy::none()`), bound request timeouts (e.g. 10s for status/WhoIs, 50s for certificate issuance inside the 60s CLI timeout), reuse the built `reqwest::Client`, and keep proof names, headers, response bodies and key material out of logs, arguments and disk.
   - **Candidate probing & error wording:** try discovered LocalAPI candidates in order. Unreachable or stale endpoints (connection refused, `ENOENT`, `EACCES`, HTTP 401/403) continue to the next candidate; a `BackendState != "Running"` daemon marks Tailscale as installed and continues looking for a running one. Once a candidate returns `BackendState == "Running"`, it is authoritative: missing MagicDNS, a tagged device or an owner mismatch returns that error immediately and never falls through to another candidate. When no candidate responds, choose `"Open Tailscale and sign in..."` vs `"Install and open Tailscale..."` via a passive filesystem existence check (`/Applications/Tailscale*.app`, `~/Applications/Tailscale*.app`, or `tailscale` on `PATH`) without executing any binary. [Tailscale discovery](https://github.com/tailscale/tailscale/blob/main/safesocket/safesocket_darwin.go) · [Local client](https://github.com/tailscale/tailscale/blob/main/client/local/local.go)

2. **One `Tailscale` client and one `HostStatus` observation.**
   - Replace `Host { binary, dns }` and the back-to-back `discover()` / `identity()` status calls with:
     - `Tailscale`: the async LocalAPI handle (`status`, `status_without_peers`, `permits`, `certificate`).
     - `HostStatus`: `{ owner: TailscaleIdentity, dns: String, addresses: Vec<IpAddr> }`, parsed once from one `GET /localapi/v0/status` response.
   - Explicit CLI setup (`mobile::enable()`) reads LocalAPI once to propose approval; the server's `bind()` reads LocalAPI once to verify owner/DNS, fetch the cert and bind. Saved owner and endpoint in `mobile.yaml` and the `MobileConfigure` wire shape remain unchanged.

3. **Keep ownership, admission and TLS async on the same transport.**
   - **Bind:** full `GET /localapi/v0/status` verifies `BackendState == "Running"`, untagged `Self`, `Self.UserID`, `User[user].LoginName`, `Self.ID`, `.ts.net` `DNSName` and non-empty `TailscaleIPs`. `GET /localapi/v0/cert/<dns>?type=pair&min_validity=24h` feeds the PEM pair straight into the existing in-memory `rustls` parser.
   - **Maintain (every 5 seconds):** async `GET /localapi/v0/status?peers=false` verifies `BackendState == "Running"`, untagged `Self`, `Self.ID == config.owner.node`, `Self.UserID == config.owner.user`, `Self.DNSName == dns` and non-empty `TailscaleIPs` (avoiding O(peers) JSON serialization every 5s on large tailnets; if `User` is present in the response, still require `LoginName` non-empty). Renew the certificate every 6 hours via `GET /localapi/v0/cert/<dns>?type=pair&min_validity=24h`. Lost identity, login, DNS approval, certificate or daemon stops the listener, drops active remote connections and reports unready.
   - **Per-connection admission:** every accepted TCP socket runs async `GET /localapi/v0/whois?addr=<url-encoded ip:port>&proto=tcp` (including bracketed IPv6) and applies the existing `permits_peer` predicate and `mobile-same-device` opt-in. A missing peer or failed WhoIs refuses that connection.
   - Because all three operations are native async on `reqwest::Client`, `listeners.abort_all()` cancels in-flight status, WhoIs and certificate requests immediately on `--off` without blocking worker threads. [Certificate API](https://github.com/tailscale/tailscale/blob/main/client/local/cert.go)

4. **Non-blocking, cancellable restoration with one backoff schedule.**
   - Replace `manage()`'s perpetual 5-second `interval` and inline blocking `bind().await` with a clear state machine: **Off**, **Restoring** (one cancellable async bind task), **Ready** (one running listener), or **Waiting** (one `Sleep` deadline scheduled from completion).
   - Try immediately on startup or explicit setup. After a failed bind or listener exit, schedule the next restoration from completion at **5, 10, 20 and then 30 seconds** (capped at 30s). Successful bind resets the failure history. Rediscover fresh on every restoration so `IPNExtension` port/token rotation recovers automatically.
   - `Control::Status` always answers immediately from `manage()`'s last `Status` — never stalling behind an in-flight bind or certificate fetch.
   - `Control::Configure` (`--off` or explicit setup) immediately aborts any in-flight restoration task and any running listener, cancelling pending retries.
   - Saved-port restore, explicit setup's occupied-port repair, exact tailnet address binding and the independent local Unix service retain their current behavior.

## Choices made for review

Recorded in `scratch/concept-review.html` and `scratch/questions.md`. All are internal to `server/{mobile,remote}.rs` and reversible before release:

- **LocalAPI only (passed over: legacy CLI fallback with 60s throttle):** Every supported Tailscale distribution speaks LocalAPI; any macOS CLI fallback still triggers `nehelper` whenever Tailscale is stopped, while doubling maintenance intervals and backoff logic.
- **Direct `libproc` for macOS App Store discovery (passed over: `/usr/sbin/lsof` child):** Matches `os_process/macos.rs`, spawns zero processes, takes <1ms, works in sandboxes and keeps proof tokens in memory.
- **Cancellable async restore + single 5 → 10 → 20 → 30s backoff (passed over: inline blocking `bind()` and 60s dual-tier backoff):** Prevents `--status`/`--off` stalls during slow binds and halves worst-case recovery latency when reopening Tailscale with negligible (<0.5ms) idle cost.
- **Full `status` at bind, `status?peers=false` every 5s (passed over: full `status` every 5s or `watch-ipn-bus`):** Preserves `LoginName` verification at bind, keeps steady-state checks O(1) on large tailnets and avoids version-sensitive `watch-ipn-bus` masks.

## Build and gate

- **Docs:** updated in place ahead of implementation in `docs/server.md` (Phone access) and `rust/loopflow/src/server/README.md` (remote carrier LocalAPI transport and restoration loop).
- **Implemented:** Setup, saved restore, owner checks, WhoIs and certificate issuance use one async LocalAPI client. A pending bind or retry can be cancelled. The manager finishes a configuration write before handling another configuration. The online `--off` command reaches the manager even while the setup CLI holds the startup lock.
- **Observed in private fixtures:** A phone received a TLS/WebSocket welcome and subscription reading, survived two peer-free checks, disconnected on an owner change and reconnected to the saved address after the daemon restarted. The local subscription answered checkpoints throughout. The trap CLI recorded zero invocations. Separate macOS fixtures exercised the kernel's open-file lookup and authenticated requests before and after a loopback port and proof rotation.
- **Focused fixtures:** A saved approval first found its socket missing. A daemon at the same path then received requests 5, 10, 20, 30 and 30 seconds after each preceding failure. Status answered during the held requests. Turning mobile access off cancelled the last request, saved the disabled approval and left the daemon idle for a further 60 seconds of virtual time. Shortening the cap to 29 seconds made the test fail at the first 30-second interval.
- **Discovery and setup:** A refused Unix socket before two running daemons let setup reach the first running host. Changing its owner or clearing its MagicDNS name refused the saved approval even with the second daemon available. The fixture with no installation reported “Install and open Tailscale”; the signed-out daemon reported “Open Tailscale and sign in”.
- **Headless gate:** `cargo test -p loopflow --test transport_tests server_mobile_ < /dev/null` and `cargo test -p loopflow --lib server::mobile:: < /dev/null` and `cargo test -p loopflow --lib server::remote:: < /dev/null` with inherited `LF_*` cleared. Expect zero CLI invocations, disconnect/reconnect across owner change and port/token rotation, foreign-device rejection, unchanged local server instance and a disabled listener after restart. Desktop's Mobile settings reads the CLI result and iOS uses the same endpoint and tickets; no Swift, wire, schema or QR migration is needed.
- **Mac demo:** run the branch binary's `lf server run` with a temporary `LF_HOME` and a copy of the approved settings using port 0 in that copy. Confirm `lf server mobile --status` against the temporary home reports ready. On a quiet App Store Mac, bracket 120 seconds of idle readiness with timestamps and query `/usr/bin/log show --info --start … --end … --predicate 'process == "nehelper" AND eventMessage CONTAINS[c] "Tailscale"'`. Count delegate, save and removal records (expect zero). Confirm log visibility against a known CLI invocation in a separate control window. Record duration, readiness and counts at gate. Unrelated Tailscale activity makes the window inconclusive. Stop only the branch server by PID. A green fixture alone cannot establish the macOS result.

## Checks from the implementation checkout

- `cargo test -p loopflow --test transport_tests server_mobile_` passed both CLI fixtures. The final logout and recovery assertion passed in the targeted transport test (40.76 s).
- With a private home and a socket answering `NeedsLogin`, the branch binary's `lf server mobile` exited 1 and printed “Open Tailscale and sign in on this Machine, then try again”.
- The focused macOS library runs passed all nine `server::mobile::` tests and all five `server::remote::` tests.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed. Gate owns the quiet-Mac log measurement and CI owns Linux.
