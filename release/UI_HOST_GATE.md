# Required host UI gate

The ordinary gate (`uv run python scripts/test.py --all`) **compiles** the
macOS app and its UI-test runners but never *runs* a hosted UI test. Running one
launches the real app under a test runner, which needs macOS UI-automation
permission the runner cannot grant itself. On a host without it, the runner
hangs before it connects and Xcode exits 65 — the failure that used to make the
gate look green while proving nothing.

So the real hosted run lives here, as a separately named **required** gate:

```bash
uv run python scripts/test.py --ui-host
```

It is never pulled in by `--all`. It runs `LoopflowUITests` for real
(`xcodebuild test -only-testing:LoopflowUITests`), bounded by the same per-phase
budgets as every other phase, and writes an `.xcresult` bundle plus phase logs
under `.lf/tmp/gate/run-<pid>/ui-host/` on failure.

Because UI automation is a machine-global facility, the suite also:

- holds a machine-wide lock (`/tmp/lf-ui-host.lock`) for the whole run, so two
  hosted runs — from any worktree — can never interleave. A held lock is a
  named failure, not a queue.
- verifies after the run (pass or fail) that macOS Automation Mode returned to
  disabled. A still-enabled mode is reported as `AUTOMATION MODE LEAKED` with
  the repair steps below — never silently left to haunt the operator.

## What a pass proves

`LoopflowUITests` (currently `ScreenshotPipelineTests`) actually executed on a
permissioned host — the app launched, the runner connected, and the UI
assertions ran. This is the only signal `--all` cannot give.

## The maintained host

The maintained host is the Mac mini cron host (W2-78 bootstraps it), not the
operator's daily driver. Hosted UI automation takes over the machine it runs
on — the app steals focus, input pauses the automation, and a leaked session
plants an unkillable "Automation Running" banner (see below). On the mini
those are log lines; on a laptop someone is using, they break the human.
Until the mini is live, runs on the daily driver are the interim fallback and
carry exactly those risks.

Run this on a Mac with UI automation granted to the test runner. To grant it:

1. System Settings → Privacy & Security → **Automation**: allow the test
   runner (and the terminal launching it) to control the app.
2. System Settings → Privacy & Security → **Accessibility**: add the same.
3. Re-run `uv run python scripts/test.py --ui-host`.

Target: **5/5** clean runs on the maintained host.

## Stuck Automation Mode ("Automation Running" banner)

Symptom: the macOS pill "Automation Running — press ⌥⌘. to stop" persists with
no test visibly running, dodges the cursor, and greys out while you use the
machine. The advertised ⌥⌘. does nothing — it targets the test host process,
which is already dead.

Cause: testmanagerd disables Automation Mode only when its in-memory client
count reaches zero. A runner that dies unobserved (SIGKILL, crash, or a hung
run's cleanup) leaks a client, and every later run counts 1→2→1 without ever
reaching zero. Verified on this repo 2026-07-20/21: three overlapping hosted
runs leaked one client; the banner then survived ~18 hours and three clean
runs.

Do not fight SIP — these all fail, even as root:

- `rm /var/db/com.apple.dt.automationmode/automation-enabled` → not permitted
- `launchctl kickstart -k system/com.apple.dt.automationmode-writer` → not
  permitted
- `automationmodetool` has no disable subcommand (it only manages the
  authentication requirement)

Repair (no reboot needed):

```bash
killall testmanagerd     # user-owned; launchd respawns it with client count 0
uv run python scripts/test.py --ui-host   # any session ending cleanly disables the mode
```

The state file under `/var/db/com.apple.dt.automationmode/` is deleted by the
system itself during that clean teardown. A reboot also clears it, but is
never required.

## Absent capability is a failure, never a silent skip

- On a non-macOS host, or when UI automation is missing, the gate fails and
  prints `MISSING CAPABILITY: …` naming the permission and the next action.
- A **runner-bootstrap** failure (the runner never begins executing, exit 65,
  Apple-events/TCC denials, or a hung control session that never connects) is
  classified as the same capability gap — not a red test — so a permission
  problem never reads as a broken test. On macOS 26 / Xcode 26 the denial
  surfaces as `The test runner hung before establishing connection` /
  `Timed out … initiating control session with daemon`, so those signatures
  count as the gap too.
- A genuine test assertion failure stays a raw `FAILED`, with its log preserved.

## Simulating the bootstrap failure

To prove the classification path without a real permission change (used in CI /
by workers on any host):

```bash
LF_UI_HOST_SIMULATE_NO_PERMISSION=1 uv run python scripts/test.py --ui-host
```

This short-circuits before Xcode and exits non-zero with the capability message
and next action — no Xcode, no hang.

## Verification log

**2026-07-15 — Jacks-MacBook-Pro (macOS 26.0.1 / Xcode 26.2), 5/5 NOT met — blocked on permission.**

Two independent real-host attempts both hung: the `LoopflowUITests-Runner`
launched, printed `Running tests…`, then `The test runner hung before
establishing connection` after `Timed out after 120.0s while initiating control
session with daemon` — `** TEST FAILED **`, exit 65, ~710s per run.
`LoopflowUITests` **never executed** (`ui_executed=no`). This is the
UI-automation permission gap, not a red test: the maintained host has not
granted the test runner (and the process launching it) Automation +
Accessibility.

Two real gate defects were found and fixed while attempting the proof:
- `_ui_host_commands` wrote to a **fixed** `-resultBundlePath`; `xcodebuild test`
  exits **64** rather than overwrite an existing bundle, so the 2nd run onward
  died in ~1s. Now pid-scoped (`_run_artifact_root()`), matching this doc.
- The macOS 26 / Xcode 26 hung-control-session signature was **not** in the
  classifier's marker set, so the permission gap misreported as a raw red.
  Now classified as `MISSING CAPABILITY` (verified against the real captured log).

**One operator action to unblock 5/5:** on this host, System Settings → Privacy
& Security → **Automation** and **Accessibility**, grant the terminal/agent that
runs the gate (and the `LoopflowUITests-Runner`) permission to control the app,
then run `uv run python scripts/test.py --ui-host` 5×. It cannot be granted
headlessly — the TCC approval is an interactive dialog. Until then the required
gate honestly reports `MISSING CAPABILITY`; the 5/5 green run stays open.
