# LOO-683 — Desktop exits leave no trace

Status: implemented (2026-10-29); gate and live acceptance remain. Jack Heart
requested this Task on 2026-10-29 after Loopflow.app 0.15.0 went down a little
before 07:28 PDT.

## Problem

When Loopflow Desktop goes away, nothing that Loopflow owns says how or why.
The operator found no .ips file, nothing in the unified log, and Desktop log files
under `~/Library/Logs/Loopflow/` that stopped being written in July. Jack can't
tell a crash from a kill, a quit or a self-exit. Nobody can act on an exit nobody
can explain.

## What happened on 2026-10-29 (findings)

- **The app called `exit(1)`.** It was PID 52901, launched Oct 28 16:47 PDT. At
  07:27:25.606, runningboardd logged `[app<application.com.loopflow.mac…>:52901]
  termination reported by launchd (0, 0, 256)`. That is OS-reason namespace 0
  (no kill reason) and wait status 256 (exit status 1). The same burst logged
  `(2, 9, 9)` (SIGKILL) for the app's Open/Save panel service, so the encoding
  is unambiguous. This was not jetsam (runningboardd logs "not memory-managed"
  for this app), not a signal and not a crash. That explains the missing .ips.
  WindowServer logged `Process death` at 07:27:25.578.
- **Its last activity:** at 07:27:24 it created a WKWebView page (a retained
  scratch HTML page) and launched its WebContent process. It closed and destroyed
  that page at 07:27:25.194. It exited 380 ms later. No Loopflow Swift source
  calls `exit`, `abort` or `terminate` on this path. The only `NSApp.terminate`
  is the UI-test snapshot path. Ghostty's quit action is not handled. Statically
  linked libghostty (Zig) and the WebKit/AppKit stack remain the candidates.
  **The caller of `exit(1)` is unidentified;** an atexit stack would have named it.
- **The operator's unified-log search was empty because of a shell trap:** in zsh,
  `log` is a builtin. `/usr/bin/log show` has everything. The relevant query:
  `/usr/bin/log show --start … --end … --predicate 'process == "runningboardd" AND eventMessage CONTAINS ":<pid>] termination reported by launchd"'`.
  Over a 5-minute window it takes about 4 s; over 15 hours, about 70 s.
- **A read storm ran all night.** From about 21:00 PDT until the exit, Desktop
  started about 6,000 `lf task diff <task> --files --base parent --json` reads an
  hour. They targeted two manabot Tasks (`task_87e0…` for
  `manabot.87e012-tla-t2-casting`, and `task_6875…`). Of 56,033 such Processes,
  56,004 have no completion: Desktop SIGKILLed each one when the next file event
  cancelled the read (`MacLocalWaveAgentLauncher.collect` kills its child on
  cancellation).
  The loop is in `TaskFileDocument.invalidate`. A write to `.git/…/index` marks
  the comparison changed. `lf task diff` runs `git diff`, and `git diff` rewrites
  a stale index even with `GIT_OPTIONAL_LOCKS=0` (verified with git 2.55; only
  `git status` honors the variable). So each read can trigger the event that
  cancels and restarts it.
  The storm ended with the app at 07:27:22. syspolicyd was logging
  `qtn_proc`/MIG errors at the moment of exit (see LOO-678). The evidence does not
  tie the storm to the `exit(1)`, but it is the largest anomaly in the window. Its
  fix is a separate outcome; see Exclusions.
- **It happened again at 08:00:11.622 PDT:** PID 9446, the 07:28 relaunch, with
  the same `(0, 0, 256)` (`exit(1)`). Again a WebKit helper of the app
  (`com.apple.SafariPlatformSupport.Helper`, pid 11957) was SIGKILLed in the same
  burst. The app wrote nothing to the unified log in the preceding minute. Desktop
  stayed down until 08:05:18, when an unstamped `"app":"1.0"` bundle launched; 0.15.0
  relaunched 6 s later. The journal also holds an earlier `1.0` launch at Oct 28
  16:48 PDT. Two `exit(1)`s 33 minutes apart, each beside WebKit teardown, point
  at the WebKit/AppKit path more than at the storm.
- Desktop already keeps a durable, bounded launch journal:
  `<LF_HOME>/desktop-cache/timings/launches.ndjson` (`LaunchJournal`). It showed
  the 07:28:12 relaunch, plus an unexplained second 0.15.0 launch at 07:29:44
  while PID 9446 stayed alive (see questions).
- Dead writer: `swift/Loopflow/Services/LoggingService.swift` has no callers.
  The `lfd.log`, `ui.log`, `model.log` and `wave.log` files under
  `~/Library/Logs/Loopflow/` are its leftovers. `lf install` schedule's launchd
  agent still writes `refresh.log` there.

## Demo

Jack quits Loopflow with ⌘Q, relaunches it, then Force Quits it and relaunches
again. `lf doctor` then prints:

```
⚠ desktop-exit  Loopflow 0.15.1 (pid 9446) was killed 09:14:02 by SIGKILL (Force Quit, kill -9, or the system), 1h12m after launch; last vitals 09:10: 412 MB, 61 lf reads/5 min
                Loopflow 0.15.1 (pid 8811) quit 09:01:40: Quit requested by Dock (pid 612)
```

Had this shipped before 07:27, doctor would have printed: `exited with status 1
from <caller frame>`. The heartbeat before it would have shown roughly 1,000 lf
reads killed in the last 5 minutes (`task diff` ×1,000).

## Approach

Each fact has one home:

| Fact | Writer | Home | Reader |
|---|---|---|---|
| Launch identity, exit and reason | Desktop `LaunchJournal` | `desktop-cache/timings/launches.ndjson` | `lf doctor` check `desktop-exit`; `timings.py` |
| Diagnostics (vitals, hangs, memory pressure, lf failures, exception or exit stacks) | Desktop `DesktopLog` | `<LF_HOME>/logs/lf.log`, same file as every lf process | people; doctor reads the last vitals line per pid |
| How the OS ended an unrecorded launch | macOS (runningboardd) | unified log | `lf doctor` on demand |

### Desktop (Swift)

1. **`DesktopLog`** (new, `LoopflowMac/Services/DesktopLog.swift`). It appends a
   line to `<LF_HOME or ~/.lf>/logs/lf.log` in lf's line shape:
   `<RFC3339Z>  WARN loopflow::desktop: <message> k=v … pid=<pid> desktop=<version> rev=<short>`.
   It opens the file per write with `O_APPEND` on a serial utility queue. No
   stdout, no banners. Its home resolution matches `WorkCache.homeDirectory` and
   Rust's `lf_home_dir`. The three error/warning `os.Logger` sites move to it:
   `DesktopServer` (API unavailable), `LoopflowApp` (resume interrupted failed),
   and `TaskFileDocument` (ignore check failed). `SessionsLatencyMetrics` stays
   as an os_log metric.
2. **Launch identity:** the `launch` event gains `pid`, `bundle`
   (`com.loopflow.mac` or `.dev`; both write the same journal), `rev` and `os`.
   `rev` is a new Info.plist key, `LoopflowSourceRevision`, which
   `scripts/bundle_version.py` stamps (from `GITHUB_SHA` on CI, otherwise
   `git rev-parse HEAD`). Release (`release-loopflow.py`) and dev
   (`install.py`) bundles both carry it.
3. **Exit recording** in `LaunchJournal` (each launch records at most one `exit`;
   a process-wide flag dedupes):
   - `NSApplication.willTerminateNotification` → `reason:"quit"`, written
     synchronously. If the current Apple event is `kAEQuitApplication`, the
     record adds its sender pid and process name, plus the
     `kAEQuitReason`/`kAEShutDown`/`kAERestart`/logout reason.
   - `atexit` → `reason:"exit"` with `caller`: the first frame of
     `Thread.callStackSymbols` outside libsystem/libdyld. The full stack goes to
     DesktopLog as one line. atexit cannot see the status; doctor recovers it
     from the system record.
   - Signal handlers for SIGTERM, SIGHUP, SIGINT, SIGQUIT, SIGABRT, SIGSEGV,
     SIGBUS, SIGILL, SIGTRAP and SIGFPE write a line that was preformatted at
     launch, using only `open(O_APPEND)`, `write` and `close` (async-signal-safe).
     The path is not a pre-opened fd, because trimming replaces the file. The
     handler then restores the previous handler and re-raises; fault signals
     return, so the fault re-triggers. Ghostty's crash reporter may take the
     fault signals after us. That is accepted: crashes also leave a .ips and a
     `.ghosttycrash`.
   - `NSSetUncaughtExceptionHandler` → `reason:"exception"` with name and
     reason. The stack goes to DesktopLog.
4. **Vitals heartbeat** every 5 minutes, written to DesktopLog at INFO:
   `phys_footprint` MB (`task_info TASK_VM_INFO`), peak footprint, open fds,
   threads, and the lf read count, failures and kills since the last beat, with
   the top three verbs. Counters live beside `LaunchJournal.read`; `collect`
   reports a kill. When more than 100 reads are killed in a window, the line is
   WARN (`lf reads cancelled at storm rate`). That line would have exposed
   tonight's loop within 5 minutes. The heartbeat also bounds doctor's
   system-log search.
5. **Memory pressure:** a `DispatchSource.makeMemoryPressureSource([.warning, .critical])`
   event writes WARN with the footprint.
6. **Main-thread hangs:** a watchdog thread pings the main queue once a second.
   A hang of 2 s or more logs WARN with its duration when it ends. A hang still
   running at 10 s logs immediately (`main thread unresponsive 10s`), because
   force-quits follow hangs. Each line names the pending `Perf` interval, if any,
   as context.
7. **lf subprocess failures:** a non-zero exit from `queryLf` logs the verb (the
   first two non-flag words, as `LaunchJournal.read` does), the status and the
   first stderr line, capped at 200 characters. Spawn failures and the 30 s
   timeout also log. Arguments are never logged. Watch observations
   (`LocalLineObservation`, `LocalWorkObservation`) log their unexpected
   termination.

### Rust (`lf doctor`)

New check **`desktop-exit`** in a new module, `rust/loopflow/src/desktop_exits.rs`.
`doctor.rs` adds one line to call it.

- Parse `launches.ndjson`, skipping torn lines. A launch has ended when it has an
  `exit` event, a later launch of its bundle exists, or its pid is gone. Per
  bundle, newest first: "running since" for a live latest launch, then up to 3
  abnormal ends within 24 h ("… and N earlier" beyond), the latest end, and the
  last quit within 24 h.
- Recorded `quit`, `exit`, `signal` or `exception`: report it.
- Not recorded: find the last `desktop=` line in lf.log for that pid (the vitals
  bound, else `launch.at`). Always run `log show --style ndjson` over [bound,
  min(next launch or now, bound + 300 s + 60 s)], with a 20 s timeout. Parse
  `termination reported by launchd (ns, code, status)` and the record's
  offset-carrying `timestamp` into `Termination.at`. Decode namespace 2 as a
  signal name and namespace 1 as jetsam; for namespace 0, `WIFEXITED` gives the
  status. Other namespaces print as numbers. The line names that time
  (`was killed 09:14:02 by SIGKILL …`) and `lived` measures to it; only a
  time-less record falls back to `before <window end>`. Without a match: "ended
  without a record (SIGKILL before the first heartbeat, power loss, or the system
  log rotated)".
- Every ended line carries `, <span> after launch`;
  abnormal ends append `; last vitals HH:MM: <MB> MB, <reads> lf reads/5 min`.
- Status: ok for `quit` or a still-running launch; warn for any other end within
  24 h. Older ends are ok, with their detail. The check never fails, so doctor's
  exit status does not change.
- Inject the system-log lookup as a closure parameter in production code. Tests
  pass a fake closure; no trait.

### Delete — do not maintain

- `swift/Loopflow/Services/LoggingService.swift`: whole file, no callers.
- `scripts/loopflow-dev.py:399`: the `Logs: ~/Library/Logs/Loopflow/` line points
  at `~/.lf/logs/lf.log` and `lf doctor` instead.
- `published.rs` `schedule`: the `refresh.log` stdout/stderr moves to
  `lf_home_dir()/logs/refresh.log`, and `~/Library/Logs/Loopflow` is no longer
  created. Installed agents keep the old path until the next `lf install`
  schedule. The plist test (if any) follows.
- The three `os.Logger` warning/error sites listed in Desktop item 1.
- Nothing deletes Jack's existing `~/Library/Logs/Loopflow/*` files. They are
  user data; the PR notes say they can be removed.

### Forbidden outcomes

- A second Desktop log file (`desktop.log`), a revived LoggingService, or lines
  written to both os_log and lf.log.
- Exit facts written to both the journal and lf.log; lf.log carries only stacks
  and diagnostics.
- Doctor writing to the journal, or running the unified-log query without a
  bounded window.
- Any banner or stdout output (quiet-terminal rule).

## Absent and error states

- No journal, an unreadable journal or no launches: the check is ok with "no
  Desktop launches recorded".
- Journal written by an older app, whose `launch` lacks `pid` or `bundle`:
  report the version and time, without a system-record lookup.
- `/usr/bin/log` missing or timing out: say "system record unavailable" and
  keep going.
- An lf.log write that fails is dropped silently; logging never takes the app
  down.

## Exclusions

- **Fixing the read storm** (the Task-file comparison feedback loop, plus SIGKILL
  churn on cancellation): a separate outcome. Proposed follow-up Task, with the
  evidence above: comparison reads stop triggering themselves. Either ignore index
  events that land while or shortly after Desktop's own comparison read runs, or
  make lf's comparison read leave the index alone, and coalesce rather than
  cancel in-flight reads. Jack decides whether to file it; see questions.
- MetricKit diagnostics (delivered a day late), app-wide `NSApplicationCrashOnExceptions`,
  and a new `lf desktop exits` command. Doctor is the one reader.
- Log rotation for lf.log; the heartbeat adds about 60 KB a day.

## Done when (gate, headless)

- `swift test --filter LaunchJournalTests` / `DesktopLogTests` (via the repo's
  Swift test runner): journal `launch` carries pid, bundle, rev and os;
  quit/exit/exception each record one `exit`; the preformatted signal line is
  valid NDJSON naming the signal; DesktopLog writes lf-shaped lines under a temp
  `LF_HOME`; the heartbeat reports kills and flips to WARN above the storm
  threshold.
- `cargo test -p loopflow --lib desktop_exits::`: on fixture journals, quit →
  ok, exit with caller → warn naming the caller, unrecorded with
  `(0, 0, 256)` → "exited with status 1", `(2, 9, 9)` → SIGKILL, no record →
  "ended without a record", a night-long gap searches one heartbeat past the
  last vitals line, quit then SIGKILL lists both, more than three abnormal ends
  list three plus "… and N earlier", Dev and release bundles reported separately.
- `uv run pytest python/tests/test_desktop_timings.py` still passes with `exit`
  events present.
- `grep -rn "LoggingService\|Library/Logs/Loopflow" swift rust scripts` → no hits.
- App build: `uv run python scripts/test.py` Desktop suites and
  `cargo clippy --all-targets -- -D warnings`.

Live acceptance (Jack, after merge): quit the app once and force-quit it once;
`lf doctor` names both exits.

## Implementation notes (2026-10-29)

- All Desktop items 1–7, the doctor check, and the delete list shipped as
  planned, with these deviations:
  - The hang watchdog does not name the pending `Perf` interval. `Perf.pending`
    is main-actor state, and the main thread is the one that's hung.
  - Exit lines carry `at` with seconds precision. Signal lines are stamped
    inside the handler with integer arithmetic only.
  - The `exit()` caller keeps the frame's address
    (`LoopflowMac 0x… symbol + n`). A stripped frame names only the nearest
    symbol; `atos` resolves the address.
  - Doctor stops reporting journals written before `bundle` existed once any
    launch records a bundle. Until then they report "its Desktop predates exit
    records".
- `DesktopLog` and `LaunchJournal.home` write nothing until `LoopflowApp` calls
  `begin` (not in AppTestMode), so tests stay silent.
- A throwaway harness verified the signal path: SIGTERM and SIGSEGV each wrote
  one stamped line, then died with status 143 and 139. A real `exit()` stack
  has the shape `exitCaller` parses.
- On Jack's journal today, branch doctor prints `warn desktop-exit Loopflow
  1.0 ended without a record before 08:05:24 (its Desktop predates exit records)`.

## Remaining work

- Gate compile suites (deferred to CI, 2026-10-29): `scripts/test.py --reuse-passing
  --rust-filter 'test(desktop_exits::) | test(doctor::) | test(install::)'` refused
  at resource preflight (6.7 GiB free, below the 32 GiB reserve; `resource_envelope.py
  --recover` reclaimed nothing because every build is active or recent). CI's
  `rust-test`, `rust-lint` and Swift jobs own Rust nextest, clippy and
  `scripts/test_desktop.sh` (LaunchJournalTests, DesktopLogTests).
- Live acceptance (Jack, after merge): ⌘Q once and Force Quit once; `lf doctor`
  names both.

## Concept reviews (2026-10-29)

Three headless passes kept the model: the journal holds exit facts, lf.log holds
diagnostics, `lf doctor` is the one reader. Their gaps (listing every abnormal
end, no 15-minute refusal, lifetime and vitals, the system record's own time) are
folded into the Rust approach above. The read-storm follow-up stays an exclusion;
filing it is Jack's call (questions.md).

A fourth headless pass (2026-10-29) checked the code against the design and found
nothing to change. The journal has one exit line per launch, deduped by a single
flag across quit, `atexit`, signals and exceptions. Doctor reads the system
record's own time from `--style ndjson`. Store frontier is read when doctor runs
(an assumption in questions.md). The design raises no open product choice. Filing
the read-storm follow-up is a separate Task and doesn't block this one.

Check (realign, 2026-10-29): `cargo test -p loopflow --lib desktop_exits::` 12 passed; fmt and `clippy --all-targets -D warnings` clean.
Check (gate, 2026-10-29): `cargo fmt --all -- --check` ok; `pytest python/tests/test_desktop_timings.py` 2 passed; architecture, Swift boundary and Rust test-inventory checks ok; grep for `LoggingService|Library/Logs/Loopflow` in swift/rust/scripts empty; compile suites deferred to CI (disk below reserve).
