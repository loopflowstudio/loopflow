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

## What a pass proves

`LoopflowUITests` (currently `ScreenshotPipelineTests`) actually executed on a
permissioned host — the app launched, the runner connected, and the UI
assertions ran. This is the only signal `--all` cannot give.

## The maintained host

Run this on a Mac with UI automation granted to the test runner. To grant it:

1. System Settings → Privacy & Security → **Automation**: allow the test
   runner (and the terminal launching it) to control the app.
2. System Settings → Privacy & Security → **Accessibility**: add the same.
3. Re-run `uv run python scripts/test.py --ui-host`.

Target: **5/5** clean runs on the maintained host.

## Absent capability is a failure, never a silent skip

- On a non-macOS host, or when UI automation is missing, the gate fails and
  prints `MISSING CAPABILITY: …` naming the permission and the next action.
- A **runner-bootstrap** failure (the runner never begins executing, exit 65,
  Apple-events/TCC denials) is classified as the same capability gap — not a red
  test — so a permission problem never reads as a broken test.
- A genuine test assertion failure stays a raw `FAILED`, with its log preserved.

## Simulating the bootstrap failure

To prove the classification path without a real permission change (used in CI /
by workers on any host):

```bash
LF_UI_HOST_SIMULATE_NO_PERMISSION=1 uv run python scripts/test.py --ui-host
```

This short-circuits before Xcode and exits non-zero with the capability message
and next action — no Xcode, no hang.
