# Desktop vanished twice — 2026-11-02

Companion to `scratch/desktop-database-mismatch-2026-11-02.md` (same day, different failure).

## Symptom

Jack reported production Loopflow Desktop broken or crashed around 13:20 PST. No process was
running from `/Applications/Loopflow.app`; the bundle (0.15.1, rev a101621e4, Oct 30 08:51)
was intact, `lf` 0.15.1 linked into it, Sparkle.framework present, LaunchServices resolving
`com.loopflow.mac` to `/Applications/Loopflow.app/`.

## What actually happened (evidence)

The installed Desktop exited silently, twice today, the same way both times:

| pid   | launched (PST) | ended (PST) | launchd report      | journal exit line |
|-------|----------------|-------------|---------------------|-------------------|
| 99798 | 10:37:03       | 11:19:22.44 | `(0, 0, 256)` = exit status 1 | none |
| 91660 | 11:40:25       | 13:05:39.72 | `(0, 0, 256)` = exit status 1 | none |

Sources: runningboardd `termination reported by launchd` lines (`/usr/bin/log show`, not
zsh's `log` builtin), `~/.lf/desktop-cache/timings/launches.ndjson`, and
`~/.lf/logs/lf.log` vitals (`pid=91660` last at 21:05:25Z; `pid=99798` last at 19:17:03Z).

- Not a crash *as macOS records crashes* (corrected below in Chain A: it was a WebKit crash that libghostty's Breakpad caught and ended with `_exit(1)`): no `.ips` for the `/Applications` path. Today's Loopflow crash reports are
  other copies (`~/Applications/Loopflow Dev.app` 13:17, pytest temp bundles 11:03–13:04),
  all dyld "Library missing @rpath/Sparkle.framework", plus one Dev launch-constraint kill
  at 11:04. Unrelated to the installed app.
- Not a quit, SIGTERM, or libc `exit()`: 0.15.1 contains #1771 (LOO-683), which records all
  three in the launch journal (`willTerminateNotification`, `atexit`, async-signal-safe
  handlers). Earlier records prove the hooks work (quit 2026-10-30, SIGTERM 2026-10-30 and
  2026-11-01). Neither of today's launches got an `exit` line and lf.log has no
  "exit() called" line. #1771 was written for exactly this signature ("two exit(1)s on
  2026-10-29, each alongside WebKit teardown; an atexit stack would have named it"). It
  did not, so the exit path **bypasses atexit**: `_exit`, an `exec`, or an exit from a
  context where handlers cannot run.
- Both exits followed the same in-app browser sequence on the main thread, 230–350 ms
  before the process was gone: a WKWebView navigation (`canPerformAuthorizationWithURL`,
  `didFinishLoadForFrame`), then `stopLoading` → `WebPageProxy::close` → destructor.
  At 13:05: load done .380, stopLoading .441, close .486, gone .717. At 11:19: load done
  :21.062, stopLoading :22.061, close :22.092, gone :22.440. Page counts were high
  (pageProxyID 43 after 85 min, 101 after 2h40m), so WKWebView creation and release is
  routine in Desktop (`BrowserPages.view(for:)` / `release`), and this is correlation with
  navigation activity, not a proven cause.
- Not an install or update: the ledger has no `lf install`, `lf self`, or `lf open` at
  either time; `/Applications/Loopflow.app` is unchanged since Oct 30; Sparkle's last check
  was 18:37:04Z (`SULastCheckTime`), interval 86400 s, no Sparkle log lines around either
  exit. `lf install`'s `quiesce_app_processes` (AppleScript quit, then SIGTERM/SIGKILL)
  would have left a quit or signal record and a different launchd status.
- Not the morning's LaunchServices cleanup: `path to application id "com.loopflow.mac"`
  still resolves to `/Applications/Loopflow.app/`, and the relaunch below worked through
  LaunchServices.
- Desktop-spawned reads at the time: `lf task diff task_e1cb37d0… --files` (pid 96736,
  cwd `/`) started 21:05:14Z and never completed; it is gone now. Last vitals before the
  13:05 exit: footprint 953 MB, peak 1085 MB, fds 225, threads 59, reads 35 (4 failed,
  3 killed), after three main-thread hangs of 2.6–3.5 s in the previous hour. Before the
  11:19 exit: footprint 511 MB, fds 127, one 3.6 s hang.
- Running Sessions and Graphs are not implicated and were left alone (LOO-500, LOO-691,
  LOO-705, LOO-708–711, MTG-235–238, INF-123, Dev app pid 5201 for LOO-673).

## Repair

Relaunch only. `/Applications/Loopflow.app` was launched at 13:22:28 PST (pid 37705; the
journal shows the launch a minute before this run's `open -a /Applications/Loopflow.app`,
which activated the same instance). No files, store, LaunchServices registrations, Sessions
or Graphs were changed. `loopflow.db` untouched.

## Verification

- `ps`: `/Applications/Loopflow.app/Contents/MacOS/Loopflow` pid 37705 running.
- Journal: launch BACA253A… 0.15.1 rev a101621e4, `restored` (cache hit) 448 ms,
  `first_frame` 567 ms, `fresh` 2794 ms.
- `lf desktop state` answers "Desktop reading" with the loopflow repository window and the
  manabot Task view; socket `~/.lf/run/desktop/com.loopflow.mac.sock` recreated 13:22.
- lf.log vitals at 21:27:28Z: `pid=37705 desktop=0.15.1 footprint_mb=1039 fds=107
  reads=77 failed=18`.

Recovery holds as long as the unknown exit does not fire again; it has now fired four times
across two days (2026-10-29 07:27 and 08:00, 2026-11-02 11:19 and 13:05).

## Open

(Superseded by Causal analysis, Chain A: the caller is Breakpad inside libghostty, after a
WebKit EXC_BAD_ACCESS. Original items kept below.)

- What calls `_exit(1)` (or execs) in the Desktop process ~250 ms after a WKWebView is
  released on the main thread. Candidates not yet excluded: libghostty (a Zig
  `std.process.exit` through libc would have hit atexit, so it would have to be `_exit`
  or a non-libc exit), a forked child path running in the parent, WebKit UI-process
  teardown. Swift code in LoopflowMac has no `exit`, `_exit`, `execv` or `posix_spawn`
  calls; the Rust library's `process::exit` sites are bin-only or exit with 0/127/130.
- The atexit record promised by #1771 cannot catch this path. The next instrumentation
  needs to sit below libc `exit`: an interposer on `_exit`/`__exit` in the app, or a
  DTrace/`proc_exit` probe while Desktop runs, logging a stack and the last navigation.
- `lf doctor` from a main build (`desktop_exits.rs`) should already describe both launches
  as "exited with status 1 … last vitals …"; not run here because the installed 0.15.1
  carries the same check and the raw evidence above was read directly.
- 18 of 77 reads failed in the first five minutes of the relaunched instance
  (`machine list×38`); unrelated to the exit but visible in vitals.

## Causal analysis (5whys, 2026-11-02 afternoon)

Three separate chains. No evidence joins them; each is kept on its own.

- **A:** three of the four "silent exits" were crashes (the fourth matches by signature only). A WebKit UI-process null dereference in
  `RemoteLayerTreeDrawingAreaProxyMac::displayLink()` after a WKWebView was released, caught
  by libghostty's Breakpad, which then called `_exit(1)`. Minidumps are in
  `~/.cache/ghostty/sentry/`.
- **B:** today's Sparkle dyld crashes are a deliberate pytest control test, plus a Dev app
  built from LOO-673's checkout before #1794. LOO-699 holds on main.
- **C:** the first 5whys run died because a weekly-limit rejection was classified as a
  transient rate limit (resume the same thread) while the account router switched accounts.

### Chain A — the silent `exit(1)`s were WebKit crashes that libghostty's Breakpad turned into `_exit(1)`

Status: **cause identified** (a WebKit UI-process null dereference) for three of the four
exits. The trigger in Desktop and a fix are still open. Recovery still holds only until the
next one.

**Evidence.** libghostty starts sentry-native with the Breakpad backend in every process that
calls `ghostty_init` (`vendor/ghostty/src/crash/sentry.zig`; `-Dsentry` defaults to true on
macOS and Loopflow's builds never turn it off; Desktop calls `ghostty_init` in
`GhosttyManager.swift:160`). Its database is `~/.cache/ghostty/sentry/<run>.run/`. Run
directories were last written at exactly the exit times:

| run dir | mtime (PST) | event timestamp (Z) | code_file | exception |
|---|---|---|---|---|
| `f7580d49…` | 10-29 07:27:25 | 14:27:25.519 | `/Applications/Loopflow.app/…/Loopflow` | EXC_BAD_ACCESS / KERN_INVALID_ADDRESS |
| `6ba4efc5…` | 11-02 11:19:22 | 19:19:22.415 | same | same, addr `0xfffffffffffffff8` |
| `8cff24f8…` | 11-02 13:05:39 | 21:05:39.670 | same | same, addr `0xfffffffffffffff8` |

Each holds a `level: fatal` event with a minidump. Parsing the minidumps (exception stream,
main thread `0x103`, frame-pointer walk) and symbolicating with `atos` against this Mac's
WebKit (same OS build 25A362) gives the same stack all three times:

    WebKit::RemoteLayerTreeDrawingAreaProxyMac::displayLink() + 32        <- pc, reads [nil - 8]
    WebKit::RemoteLayerTreeDrawingAreaProxyMac::scheduleDisplayRefreshCallbacks() + 112
    WebKit::RemoteLayerTreeDrawingAreaProxy::commitLayerTree(...)
    WebKit::RemoteLayerTreeDrawingAreaProxy::didReceiveMessage(...)
    IPC::MessageReceiverMap::dispatchMessage → WebProcessProxy::didReceiveMessage
    IPC::Connection::dispatchIncomingMessages → WTF::RunLoop::performWork
    CFRunLoop … -[NSApplication run] … LoopflowMac_main

At 13:05, `x0 = 0x1581b0220`. The unified log names that same object as
`RemoteLayerTreeDrawingAreaProxyMac 0x1581b0220 [pageProxyID=43, webPageID=44, PID=5430]`,
the page whose `WebPageProxy::destructor` ran at 39.487. WebContent 5430 had a layer-tree
commit in flight (`Created rendering backend … didFirstMeaningfulPaint` at 39.381). It
arrived about 180 ms after the page was destroyed, and the surviving drawing-area proxy
dereferenced its now-null page or display link. This is a use-after-close in the WebKit UI
process (macOS 26.0.1 25A362, WebKit 623.1.14), triggered by releasing a WKWebView while its
first paint is still being committed.

**Why it looked like `exit(1)` and #1771 missed it** (each step checked against code or
upstream source):

1. Breakpad on macOS takes the crash on the **Mach task exception port**
   (`task_set_exception_ports`). The kernel delivers EXC_BAD_ACCESS there before it becomes
   SIGSEGV, and before the host-level port that ReportCrash uses.
2. Breakpad writes the minidump, then sentry-native's callback returns `succeeded`, and
   Breakpad runs `if (exit_after_write) _exit(exception_type);`
   (`breakpad src/client/mac/handler/exception_handler.cc`; sentry-native 0.7.8
   `sentry_backend_breakpad.cpp`). EXC_BAD_ACCESS is 1, so **`_exit(1)`**: launchd reports
   `(0, 0, 256)`, no BSD signal is delivered, no `.ips` is written, and atexit never runs.
   The binary's `__exit` import is this call.
3. So none of #1771's paths could fire. `ExitSignals` waits for a SIGSEGV that never comes.
   Its own header even says a fault "re-faults into … Ghostty's crash reporter", which
   models Ghostty's reporter as a signal handler, but it is a Mach port handler that runs
   first. `atexit` is skipped by `_exit`, `willTerminate` is not sent, and the uncaught
   exception handler only covers ObjC exceptions.
4. #1771 read "exit status 1, no signal, no .ips" as "someone called `exit()`". A crash
   reporter already in the process that exits with the exception number fits the same
   signature, and its output was already on disk. `lf doctor`'s `desktop_exits.rs` and the
   Swift journal never look in `~/.cache/ghostty/sentry` (grep finds no `sentry`,
   `breakpad` or `minidump` references), so nothing tied the dump to the exit.
5. The dumps were not even forwarded where Ghostty normally files them. Ghostty's transport
   writes `~/.local/state/ghostty/crash/*.ghosttycrash` when a later init processes old runs.
   Only the Dev app's 13:33/13:53 crashes reached that directory; the three production runs
   are still in the sentry database. Why they were not processed was not traced (lock files
   in that directory were all touched at 14:22:37).

Not explained by this: the 2026-10-29 08:00 exit has no surviving run directory, so it is
attributed to this cause by signature only. `t-2026-10-29-080013.ips` (a `/private/tmp/*/t`
SIGSEGV at 0x8) is a different program.

Copies of the three envelopes, the frame walks (`w*.txt`) and the two parsing scripts are in
`~/Library/Logs/Loopflow/desktop-exit-minidumps-2026-11-02/`, outside the repository because
minidumps contain process memory. sentry-native may delete the originals once it processes
old runs.

**Answers to the questions asked.**

- *What instrumentation would identify the `_exit(1)` caller?* None is needed for these three
  exits: the minidumps already carry the faulting thread. Going forward, two proportionate
  options (proposed, not done):
  (a) **Build libghostty with `-Dsentry=false`.** The kernel then delivers EXC_BAD_ACCESS as
  SIGSEGV, #1771's handler writes `signal SIGSEGV`, the fault re-raises, and ReportCrash files a
  normal symbolicated `.ips` next to the other Loopflow reports. One build flag deletes a
  second, invisible crash pipeline that Loopflow never reads or ships. Proof: a Dev build
  with a debug-only forced null dereference shows a `signal` journal line and an `.ips`
  instead of `(0, 0, 256)`. Check first that nothing in Desktop relies on Ghostty's crash
  files.
  (b) If Breakpad stays, `desktop_exits.rs` should read fatal events in
  `~/.cache/ghostty/sentry/*.run/*.envelope` whose `code_file` matches a Loopflow bundle, and
  report "crashed (EXC_BAD_ACCESS, minidump at …)" for a launch ending at that time.
  A `_exit` interposer or DTrace `proc:::exit` probe would also have pointed at Breakpad, but
  (a) or (b) answers the question directly and costs less.
- *Can it be reproduced by loading and releasing a page in the browser pane?* Not attempted
  here: this run has no display session, and the cause needs a window on screen
  (RemoteLayerTree commits and the display link only run for a visible web view). The
  evidence points to one window: release the WKWebView while WebContent's first layer-tree
  commit is in flight, roughly 0–200 ms after `didFinishLoad` or first paint. A private
  `LF_HOME` does not matter, because no Loopflow state is involved. The smallest repro is a
  standalone AppKit harness, with no Loopflow, Dev app or rebuild: one window, a loop that
  creates a WKWebView, loads a page and drops it N ms after `didFinish`, sweeping N. It needs
  a logged-in display session and must not be run against Jack's live Desktop. No CPU load is
  needed or wanted; the race is with IPC from WebContent, not with scheduling.
- *What in Desktop releases a page that fast?* Open. `BrowserPages.release` runs only from
  `closePane` → `.browserTab` (`SessionsView.swift:231`), and a closed tab keeps its page for
  Undo "until another close replaces it". So the page torn down at 13:05 was either just
  closed or displaced by the next close. pageProxyID 43 after 85 min and 101 after 2h40m
  show heavy churn. A WKWebView freed through any other path (SwiftUI identity change,
  workspace release) would also bypass `release`. Next step: log `BrowserPages.view(for:)` and
  `release` with the tab id and URL in lf.log, and find the caller.

**Prevention (proposed).** (1) Remove the masking with (a), so the next crash of any kind is
self-describing. (2) Work around the WebKit race at the one release site: before dropping a
WKWebView, `stopLoading()`, take it out of the window, and keep it alive until WebContent
goes quiet (for example, release on the next `didFinish` of `about:blank`, or after a
short delay). Proof: run the standalone harness above, with and without the workaround.
(3) Report the stack to Apple (Feedback Assistant) with the minidumps; also check whether a
newer macOS 26.x WebKit has already fixed it.

### Chain B — today's Sparkle dyld crashes (separate from the silent exits)

Status: no defect in current main; both sources are explained.

1. **Pytest temp bundles (10:32, 11:03, 11:53, 12:21, 13:04): deliberate.** All five `.ips`
   files name `pytest-*/test_bare_swiftpm_binary_in_a_0/Loopflow.app`, slice UUID
   `647F6486…` each time, parent `python`. That is
   `python/tests/test_app_bundle.py::test_bare_swiftpm_binary_in_a_bundle_cannot_load_sparkle`,
   the control test #1796 added: it copies the bare SwiftPM binary into a bundle and asserts
   it aborts in dyld. Every run of that suite files a real-looking "Loopflow" crash report.
   Not a LOO-699 regression; it is noise in the crash-report evidence that a later incident
   reader (this one, earlier today) can mistake for one.
2. **`~/Applications/Loopflow Dev.app` at 13:17:36: built from a checkout older than the fix.**
   - The crash shows `Contents/MacOS/Loopflow` and dyld trying only
     `Contents/MacOS/Sparkle.framework` (the SwiftPM `@loader_path` rpath). Current
     `loopflow-dev.py` names the executable `Loopflow Dev` (#1792) and
     `install_app_executable` adds `@executable_path/../Frameworks` and embeds Sparkle
     (#1794/#1796). The 13:17 binary had neither, so it was not built by current code.
   - The Dev app is LOO-673's (`--repo …loopflow.loo-673-desktop-polish-fix`). That
     worktree's reflog: at `cf8000104a` (Oct 30 13:32 PDT) until `rebase (start): checkout
     origin/main` at **13:18:17**, 41 s after the crash. `cf8000104a` contains #1776
     (Sparkle became a linked dynamic framework, Oct 30 12:05Z) and lacks #1794 (22:56Z):
     exactly the window in which `loopflow-dev.py` copied a bare binary.
   - After the rebase the bundle was rebuilt at 14:08–14:11 with `Contents/Frameworks/
     Sparkle.framework` and `MacOS/Loopflow Dev`; pid 483 has run from it since 14:11:09.
   - Why LOO-699 "didn't hold": it did, for every builder on main. The builders are
     scripts inside each checkout, so a Task branch based before the fix keeps the broken
     builder until it syncs. 17 of 34 worktrees still predate #1794 (e.g. `loo-669` detached
     at the 0.15.1 release, `loo-698`, `loo-701`); any of them running `loopflow-dev.py` or
     `install.py local` produces the same crash. LOO-699's outcome was stated for "every way
     of building" but delivered per-revision; nothing tells a stale checkout its builder is
     known-broken.
   - Who launched the Dev app at 13:17 (parent `launchd`, so via `open`) was not traced.
   - Prevention (proposed, not approved): (a) none needed for main; (b) optionally name the control test's bare executable something other
     than `Loopflow` (for example `BareSwiftPM`), so its expected dyld abort stops filing
     `Loopflow-*.ips` reports that look like product crashes; (c) for stale branches, the Dev-app
     launcher could run `verify_rpath_libraries` before `open`, which catches any bundle
     regardless of who built it. Proof for (c): launch from a checkout at `cf8000104a` and
     see a build-time error instead of a dyld crash.

### Chain C — why the first 5whys run died (`session_record.rs:1914`)

Status: explained from the store; affects every Claude run that hits a weekly limit while
another account is eligible. Not related to Desktop.

1. The run (`lf -b 5whys`, LfProcess `5bf2baa1…`, pid 85481, Session
   `session_8fa0bcae…`) started 21:35:15Z on Claude account `primary` (jackstah@gmail.com),
   provider thread `edd6adc7…`.
2. At 22:01:03Z Claude rejected the turn: `provider_rate_limited: rejected:
   seven_day_overage_included` (session_events 2829383). `primary` was put in cooldown until
   2026-11-04T12:00:05Z with that reason (`provider_accounts`).
3. `_classify_agent_failure` called it `provider rate limit`, a transient failure, not
   `AccountSubscriptionLimit`: `_classify_subscription_limit` matches phrases such as "usage
   limit reached" and none matches `seven_day_overage_included`. On the transient branch the
   retry keeps the account unset and **resumes the same thread**
   (`agent/mod.rs` ~1027: `resume_token = result.agent_session`; lf.log 22:01:04Z
   `resumed=true account_failover=false`).
4. On the retry, `resolve_account_route_blocking` correctly skipped the cooling `primary` and
   chose `loopflow` (jack@loopflow.studio). It also recorded
   `provider_session_accounts: edd6adc7… → loopflow` at 22:01:06Z.
5. `retry_agent_process` then saw `account_changed` (primary → loopflow) together with a
   resume thread, which its contract forbids, and bailed. That bail is the
   `AgentProcess retry must retain its thread unless the selected account changes` error.
   The guard was right. The retry loop and the account router disagreed about whether the
   account was changing: the loop decided from error text, the router from the cooldown it
   had just written.
6. It was systemic. Seven runs that hit the same rejection between 22:00:25Z and 22:01:41Z
   (pids 24956, 40036, 60954, 74398, 30228, 85481, 61016) all failed with this error. Later
   ones (36573, 96865) failed earlier, with "no eligible managed claude account", once both
   accounts were cooling.

Side effect to check: the `edd6adc7 → loopflow` row binds a thread that never ran on
`loopflow`, so a later resume of that Session may route to the wrong account.

Prevention (proposed): make one owner decide. When the provider's rate-limit signal puts the
account in cooldown, the attempt is an account limit: take the failover branch (fresh thread,
`LIMIT_FAILOVER_PROMPT`, `provider_account_id = None`) rather than matching text. Or, per
AGENTS.md's Flexibility rule, have `retry_agent_process` adapt when the router changes the
account (drop the resume and start fresh) rather than refusing. Proof: a fake-claude test
with two accounts whose stream emits `rate_limit_event` `rejected: seven_day_overage_included`
on the first; the run continues on the second account and succeeds, and no
`provider_session_accounts` row binds the old thread to the new account.
