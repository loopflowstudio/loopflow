# Choices made without anyone here (LOO-1110)

Each is reversible, documented in `scratch/plan.md`, and listed as numbered
questions in `scratch/concept-review.html`.

- **Custom-home servers are never registered with launchd/systemd** and exit
  when idle (10 min with no clients, held terminals, or active Graphs/agents)
  or when their home is deleted (`try_exists() == Ok(false)`). Assumes no
  custom `LF_HOME` is used as a durable Machine that must come back after
  reboot.
- **A holder whose `LF_HOME` is deleted closes its PTYs and exits**, so its
  children get SIGHUP. Never triggers on transient I/O errors, and live homes
  are unchanged (holders still outlive server restarts and installs).
- **Job budget by environment** (`CARGO_BUILD_JOBS`, `NEXTEST_TEST_THREADS`,
  `MAKEFLAGS`, `LF_JOBS` = `max(2, cores ÷ running Graph steps)`, `nice 5`,
  overridable via env or `machine.jobs`) instead of a hard build-slot queue.
- **Runtime diagnostics live under versioned `server.diagnostics`**
  (`lf server diagnostics --json`, protocol revision 10 → 11) rather than
  mutating the unversioned `wire::Status` probe (which uses
  `deny_unknown_fields` and would break older `lf` binaries during upgrades).
- **Minute Git plan exchange scopes to repos with an unfinished Task or active
  Wave on this Machine**; repos with neither still exchange on command.
- **`processes` rows are not pruned here**; LOO-965's staged drop reclaims the
  271 MB. Same for the LOO-938 tables.
- **No `gix`.** Idle forks go to zero via event-driven watching; forks after
  real edits remain.
- **Thread budget** starts at 4 async workers and 12 blocking threads; "under
  20 threads" is the quiet number, 24 under write load.
- **One PR.** The Task covers six areas; the plan keeps them together in four
  passes. If pass 3 or 4 slows the perf work, they split cleanly as stacked
  Tasks (lifetimes; job budget and file bounds).

# Resolved during concept review

- **`Hosted::pass` `open_sqlite` site**: confirmed in source (`hosted.rs:572` →
  `journal::process_ledger()` at `journal/mod.rs:753`), which silently falls
  back to `open_ledger()` because `lf server` has no `ProcessContext`.
- **Interactive held work sessions vs Graph steps**: interactive held work
  sessions launch via `lf server` → `lf holder`, not `RunningGraph::spawn`, so
  setting the job budget in `RunningGraph::spawn` scopes cleanly to autonomous
  Graph worker steps without throttling a person's interactive terminal.
- **Still for implement to check**: old `com.loopflow.server.<hash>` jobs
  marked disabled in `launchctl print-disabled` after bootout.

# Left on the Machine

- 21 orphan `lf holder run` processes for deleted homes and 3 for
  `/tmp/lf-demo-1063` and two `/private/tmp/work-cli-*` homes. Not stopped:
  they were not started by this run.
