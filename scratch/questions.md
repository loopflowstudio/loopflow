# Choices made without anyone here (LOO-1110 kickoff)

Each is reversible and marked "chosen for review" in `plan.md`.

- **Custom-home servers are never registered with launchd/systemd** and exit
  when idle or when their home is gone. Assumes no custom `LF_HOME` is used as
  a durable Machine that must come back after reboot. Is any (a rented Linux
  host, the cron host) running a non-default home on purpose?
- **A holder whose `LF_HOME` is gone closes its PTYs and exits**, so its child
  gets SIGHUP. This is the holder ending its own children, which the docs say
  it never does. Assumed fine because no server can adopt it again.
- **Job budget by environment** (`CARGO_BUILD_JOBS` and friends = cores ÷
  running Graph steps, nice 5) instead of a build-slot queue. Assumes a soft
  cap is wanted over a hard one.
- **`processes` rows are not pruned here**; LOO-965's staged drop reclaims the
  271 MB. Same for the LOO-938 tables.
- **No `gix`.** Idle forks go to zero by not polling; forks after real edits
  remain.
- **Thread budget** starts at 4 async workers and 12 blocking threads; "under
  20 threads" is the quiet number, 24 under write load.
- **One PR.** The Task covers six areas; the plan keeps them together in four
  passes. If pass 3 or 4 slows the perf work, they split cleanly as stacked
  Tasks (lifetimes; job budget and file bounds).

# Unverified, for implement to confirm first

- The exact `open_sqlite` call under `Hosted::pass` (the sample's frame is
  inlined; `hosted.rs:571` uses `process_ledger()`, which may be re-opening
  after a failed init).
- Whether server-started recovery and `session.continue` launches carry the
  caller environment (matters for the job budget reaching held work sessions).
- Old `com.loopflow.server.<hash>` jobs marked disabled in
  `launchctl print-disabled` after bootout.

# Left on the Machine

- 21 orphan `lf holder run` processes for deleted homes and 3 for
  `/tmp/lf-demo-1063` and two `/private/tmp/work-cli-*` homes. Not stopped:
  they were not started by this run.
