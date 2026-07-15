# W2-170 — Verify the required `--ui-host` gate 5/5

## Goal
Prove the shipped required gate (`uv run python scripts/test.py --ui-host`, PR
#905) runs the hosted `LoopflowUITests` for real, 5/5 clean, on the maintained
permissioned macOS host.

## Host
- Jacks-MacBook-Pro, macOS 26.0.1 (25A362), Xcode 26.2 (17C52), user `jack`.
- CORRECTION (evidence, 2026-07-15): Automation/Accessibility is NOT effective
  for the agent-launched runner. Two real runs hung — `The test runner hung
  before establishing connection` (exit 65, ~710s), `LoopflowUITests` never
  executed. 5/5 is blocked on an interactive TCC grant; see the outcome below
  and release/UI_HOST_GATE.md.

## What the proof exposed — a real gate bug (fixed here)
`_ui_host_commands` wrote `xcodebuild test` results to a **fixed**
`-resultBundlePath` (`.build/xcode-derived-data/ui-host.xcresult`). `xcodebuild`
refuses to overwrite an existing result bundle — it exits **64** with
`error: Existing file at -resultBundlePath`. So the *first* `--ui-host` run
created the bundle and every run after it died in ~1s before launching a single
test. A "5/5" proof is impossible against that path.

Both release docs already said the `.xcresult` should land under
`.lf/tmp/gate/run-<pid>/` (GATE_BUDGET.md:38, UI_HOST_GATE.md), so the code
contradicted its own documented convention.

**Fix:** route the bundle to the per-run pid-scoped artifact dir via a new
`_run_artifact_root()` helper (`.lf/tmp/gate/run-<pid>/ui-host/ui-host.xcresult`),
used by both `run_plans` and `_ui_host_commands`. Fresh path per invocation →
back-to-back runs never collide. Regression test:
`test_ui_host_result_bundle_is_per_run_not_a_fixed_path`.

## Source of truth / consumers
- Source of truth for the proof: the gate's own exit code + the xcodebuild log
  showing `Test Suite 'LoopflowUITests' ... passed` and `Executed N tests`.
- Consumers of `_run_artifact_root()`: `run_plans` (failure artifacts) and the
  ui-host result bundle. No programmatic reader of the old fixed path exists
  (grep-verified), so nothing downstream breaks.

## Proof
`scratch/ui-host-runs/run5.sh` drives the gate 5x, recording per-run
gate-exit + whether the hosted UI test actually executed to
`scratch/ui-host-runs/summary.tsv` (durable across a session teardown).
Target: 5/5 `gate_exit=0`, `ui_executed=yes`.

## Absent/error states
- No permission → runner-bootstrap markers → classified `MISSING CAPABILITY`
  (already proven in CI/tests). Not the case on this host.
- Non-macOS → precheck fails fast. Not the case here.

## Deliverable
Gate fix + regression test (product code) and a recorded 5/5 verification in
`release/UI_HOST_GATE.md`. `scratch/` is wiped on land; the durable proof lives
in the release doc.
