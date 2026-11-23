LOO-1080 concept-review choices recorded for review in `scratch/concept-review.html` and `scratch/plan.md` (all reversible, no wire/Swift/schema change):

1. **LocalAPI only (no CLI subprocess fallback):** Support macOS App Store `IPNExtension`, macOS Standalone `/Library/Tailscale/ipnport`, Unix sockets (`/var/run/tailscaled.socket`, `/var/run/tailscale/tailscaled.sock`), and `TS_SOCKET`; delete CLI subprocess probing completely. Keeping a CLI fallback on macOS still spawns `Tailscale.app` and triggers `nehelper` whenever Tailscale is quit/stopped, and doubles health/backoff complexity.
2. **Direct `libproc` on macOS (no `lsof` child):** Read same-UID `IPNExtension` open vnodes in-process via `PROC_PIDLISTFDS` / `PROC_PIDFDVNODEPATHINFO`, matching `os_process/macos.rs` with zero subprocesses and proof tokens kept strictly in memory.
3. **Cancellable async restore + single `5 → 10 → 20 → 30s` backoff:** Run restoration `bind()` off the `select!` loop so `--status` and `--off` never stall behind a slow bind; cap backoff at 30s instead of 60s since in-process discovery costs <0.5ms and triggers zero `nehelper` churn.
4. **Full `/localapi/v0/status` at bind, `status?peers=false` every 5s:** Preserves `User[user].LoginName` validation at bind while keeping 5-second maintenance O(1) on large tailnets.

5. An explicit `TS_SOCKET` selects one daemon for setup and restoration, even while that socket is unavailable. This lets a private Machine exercise restart against its own daemon. Automatic discovery still tries the platform candidates in order.
6. Exercise the normal CLI and saved phone address through a Unix-socket daemon. Exercise macOS port and proof rotation against private loopback listeners and a temporary Standalone directory. The App Store descriptor check uses an open fixture file in the test process.

7. For the restore timing fixture, leave the selected socket missing on startup, then listen at that same path. Holding and failing each request makes the delay measurable from completion and lets the daemon observe cancellation.

8. Keep overlapping setup and restore checks in the CLI transport fixtures. The virtual-clock manager fixture still owns precise backoff timing and in-flight request cancellation.
