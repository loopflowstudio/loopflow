# Prove less, and only where a machine can check it

Jack Heart's 2026-09-30 directive (LOO-357) sets the scope: sanity checks during
implementation; affected verification once at gate; no automated Flow blocked
on a display session or a person's judgment. One PR; no delivery requested here.

## Approach

- Rewrite implement, compress, realign, loop-decide, design, kickoff and gate,
  plus universal/repo authoring guidance. Keep one check-result line in scratch.
- Gate Desktop with an app build and Swift view/interaction tests without a
  window, app launch, Automation permission or person. Native window/terminal
  integration remains an explicit optional diagnostic for demo/review.
- Measure September 30 Run records by skill and compare source context before
  and after. Keep aggregate measurement at performance/, not in repeated notes.
  Compare later completed implement Runs using the same collector; do not
  present fewer instruction words as measured runtime improvement.

## Delete — do not maintain

- Required-check stop clauses in implement/compress/realign and their promotion
  into loop-decide's Blocked outcome.
- Window-launch snapshot capture from default Desktop gate/CI (retain the
  opt-in capture utility for demo). Preserve the four visible states with
  headless inspection of the production view.
- Orphan `tests/goldens/with_direction.md`: no YAML case or reader remains.
- Required permissioned UI-host policy and its five-run acceptance quota.
  Keep explicit hosted diagnostics and failure classification.
- Repeated display-test opt-in conditions and the headless test's handwritten
  selection callback. Use one test trait and the production navigator control.

## Done when

A focused runner test selects headless Desktop checks for app changes; Swift
checks inspect populated/loading/unavailable/empty production views and invoke
an actual selection control without creating a window. Gate and CI run this
path. Guidance leaves machine checks to their phase and judgment to review/demo;
an unavailable check alone cannot block an implementation loop. A reproducible
by-step baseline and later comparison expose verification time and context.

## Current state and remaining work

Guidance now assigns sanity checks to implement/compress and acceptance to gate;
loop-decide does not block solely on an unavailable check. Desktop's default
runner denies WindowServer connections, checks production views and leaves
native diagnostics opt-in. The benchmark opts in explicitly. Baseline and
comparison commands live in `performance/check-cost.md`.

The audit found a repeated stop condition in three skills plus loop-decide, and
a window-launch capture described as headless. Check-time classification must
ignore quoted commands and label heredoc batches unclassified. The frozen
baseline includes both classification and missing-repository coverage.

Command accounting now lives separately from cohort selection. Comparisons omit
Runs with missing command start times as well as missing command records. The
headless selection check invokes the workspace navigator's own action; hosted
diagnostics share one opt-in trait. Runner coverage checks Xcode's compile-only
command instead of matching a sentence in its summary.

Remaining: affected-suite gate/CI, normal Flow adoption, and a later completed
implement cohort for runtime comparison. No later cohort is manufactured by
rerunning implementation. The seven selected skills have almost identical word
counts; changed obligations and smaller future scratch, not source-size savings,
are the intended improvement.

Checks: `uv run pytest python/tests/test_check_cost.py python/tests/test_gate_bounded.py -q` — 49 passed; `scripts/test_desktop.sh -Xswiftc -gnone --jobs 4 --filter 'DesktopHeadlessTests|AppAppearanceTests|TaskFilesTests|TaskMonitorTests'` — build and 16 tests passed, 4 native diagnostics skipped with WindowServer denied; Ruff, `cargo fmt --check`, and all-target Clippy passed; prior prompt-contract/golden results remain applicable; affected suites belong to gate/CI.
