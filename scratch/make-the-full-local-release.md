# W2-116 — Make the full local release gate trustworthy and bounded

## Problem, as it actually is

`uv run python scripts/test.py --all` is the "full local gate." Two defects
make it untrustworthy:

1. **Unbounded.** `_run_suite` calls `subprocess.run(cmd.argv, cwd=cmd.cwd)`
   with no timeout. Any command — most notoriously an `xcodebuild` test-host
   run — can hang indefinitely. There is no wall-clock budget, no phase-level
   progress while a phase is live, and a hang looks identical to slow success.

2. **Dishonest about UI.** The `loopflow` suite runs only `xcodegen` +
   `xcodebuild build-for-testing`. It **compiles** the app and its UI test
   runners; it never *runs* a hosted UI test. The one real hosted UI test
   (`swift/LoopflowUITests/ScreenshotPipelineTests.swift`) is executed nowhere
   in the gate. The summary prints `PASS loopflow` with no hint that hosted UI
   behavior was not exercised. CI is the same: `loopflow-ui-test` is
   compile-only (committed comment argues the hosted run is redundant with
   un-hosted `swift-test`). The reasoning may be sound, but the gate never
   *says* it, so "PASS" silently over-claims.

The hosted run was dropped because `LoopflowUITests-Runner` hangs before it
establishes its test-host connection and Xcode exits 65 (wave-memory gotcha).
That hang is the thing this Task must classify, bound, and split off — not
resurrect inside the ordinary gate.

## User-visible outcome

A human or worker runs **one** command and gets a definitive, timely signal:

- `uv run python scripts/test.py --all` **always terminates** inside a named,
  printed wall-clock budget. Each phase (rustfmt, clippy, rust, python,
  website, swift, swift-boundaries, xcodegen, xcodebuild, e2e-smoke) has its
  own budget; a phase that exceeds it is killed, reported as a **timeout
  failure with the phase name and its budget**, and the run exits non-zero.
- The summary states **what each suite proved**. The `loopflow` line reads as a
  compile check and explicitly names that hosted UI behavior is owned by the
  separate host gate (below) — never a bare `PASS` that implies UI ran.
- A separately named **required-host UI gate** runs the real hosted UITest on a
  permissioned host. On a host without macOS UI-automation permission it does
  not hang: it fails fast, **names the missing capability and the next action**.

## Source of truth

- `scripts/test.py` — suite/phase table plus a new per-phase budget and the
  bounded runner. Single source for phase names, budgets, and the honest
  summary text.
- The required-host UI gate — a new explicit mode of `test.py`
  (`--ui-host`, its own suite entry) that runs
  `xcodebuild test -only-testing:LoopflowUITests` bounded, OFF by default and
  never pulled in by `--all`. It reuses the derived-data + signing constants
  already in the file. (Screenshot capture in `scripts/generate_screenshots.py`
  already drives `-only-testing:LoopflowUITests/ScreenshotPipelineTests` — the
  gate shares that invocation shape, it does not reinvent it.)
- Release evidence: a committed `release/GATE_BUDGET.md` recording the measured
  full-gate budget (per phase + total) from a real `--all` run on this repo,
  and how to reproduce. This is the "budget visible in release evidence."

Derived views: `TESTING.md` (documents the bounded gate and the split), CI
`ci.yml` loopflow comment (already compile-only — cross-reference the host gate
so the split is documented in both places, no behavior change to CI required).

## End-to-end proof

1. **Bounded, honest, repeatable.** Run `uv run python scripts/test.py --all`
   ten times headless. Every run terminates with an honest `PASS`/`FAIL`
   summary and a printed total-vs-budget line; none hangs. Assert via a harness
   (`tests/e2e/test_gate_bounded.sh` or a python test) that injects a
   deliberately-hanging fake phase and confirms it is killed at its budget and
   reported as a named timeout — proving no phase can hang indefinitely without
   waiting on a real multi-minute suite.
2. **Compile-vs-hosted split is visible.** In the `--all` summary the loopflow
   line names compile-only + points at the host gate; `--list` shows `--ui-host`
   as a distinct, not-run-by-`--all` suite.
3. **Required-host gate.** On the maintained permissioned host, run the host
   gate 5 times; it completes 5/5 with the hosted UITest actually executing.
4. **Bootstrap-failure classification.** A simulated runner-bootstrap failure
   (e.g. `LF_UI_HOST_SIMULATE_NO_PERMISSION=1`, or a forced non-permissioned
   invocation) makes the host gate exit non-zero with a message that names the
   missing capability ("macOS UI-automation / Automation permission for the
   test runner") and the next action (grant it / see `release/UI_HOST_GATE.md`),
   not a raw exit 65.

Command that proves the outcome:
`uv run python scripts/test.py --all` (bounded honest signal) and
`uv run python scripts/test.py --ui-host` (real UI run + failure classification).

## Affected surfaces and consumers

- `scripts/test.py` — per-phase timeout in the runner, phase progress line at
  start of each command, `TimeoutExpired` → killed process group + classified
  failure, artifact capture on failure, budget summary, honest suite labels,
  new `--ui-host` suite. Keep it stdlib-only (constraint stated in its header).
- `TESTING.md` — bounded-gate section, budget table, the compile-vs-host split,
  one-command reproduction for a red UI signal.
- `release/GATE_BUDGET.md` (new) + `release/UI_HOST_GATE.md` (new) — measured
  budgets and the host-gate runbook / capability requirement.
- Failure artifacts — write phase logs + xcodebuild `.xcresult` (when present)
  under an ignored `.lf/tmp/gate/<phase>-<...>/` so a worker repairs the first
  red signal without opening Xcode. (No `Date.now()`-style nondeterminism in
  committed code; artifact dir names use a run-scoped counter/pid, not a
  wall-clock stamp baked into fixtures.)

## Absent and error states

- **Phase over budget** → SIGTERM the process group, then SIGKILL on grace
  expiry; report `TIMEOUT <phase> (budget Ns)`; overall exit 1.
- **Missing toolchain** (no `xcodebuild`, no `cargo`, no `xcodegen`) → the phase
  fails with an actionable "install X / not available on this host" message,
  not a stack trace. The `--ui-host` gate on a non-macOS host says so and exits
  non-zero (it is a *required* gate — absence is a fail, never a silent skip).
- **UI runner bootstrap failure** → classified as a capability gap (see proof
  4), distinct from a genuine test-assertion failure.
- **Nothing changed** (plain `test.py`, no `--all`) → unchanged current
  behavior; budgets apply only to phases that run.

## Operational boundary

- Every phase carries a finite budget; the total-gate budget is the sum of
  active-phase budgets and is printed. Budgets are generous headroom over the
  measured real-run times (recorded in `release/GATE_BUDGET.md`), chosen so a
  healthy suite never trips them but a hang always does.
- Timeout enforcement uses process **groups** (`start_new_session=True` +
  `os.killpg`) so a hung `xcodebuild` and its child test-host both die — killing
  only the parent leaves the runner alive and the terminal wedged.

## Exclusions

- Not resurrecting a hosted UI run inside `--all`; the ordinary gate stays
  compile-only for the app by deliberate design.
- Not changing CI job topology (compile-only `loopflow-ui-test` stays); this
  Task only cross-documents the split. A CI host-gate job is a possible
  follow-up once a permissioned CI runner exists.
- Not building a generic timeout framework or a parallel test scheduler — a
  per-command budget on the existing serial runner is the whole mechanism.
- Not fixing the underlying `LoopflowUITests-Runner` hang's root cause; this
  Task bounds and classifies it and moves the real run to the host gate.

## Build order (serial PRs, one worktree)

1. **Bounded runner + honest summary + budgets** — per-phase timeout, process
   -group kill, progress lines, artifact capture, honest loopflow label,
   printed budget; injected-hang test. Measure a real `--all`, write
   `release/GATE_BUDGET.md`. Update `TESTING.md`.
2. **Required-host UI gate** — `--ui-host` suite running the hosted UITest
   bounded, bootstrap-failure classification + simulation hook,
   `release/UI_HOST_GATE.md`, 5/5 on the maintained host.

If PR 1 fully lands the bounded+honest+budget proof and the host gate is small,
they may collapse into one PR; keep them separable so the bounding lands even if
the host gate needs the permissioned host to verify.
