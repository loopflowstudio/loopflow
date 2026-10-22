# Open questions

- 2026-10-21 sync onto main (#1647): kept both bounds. Main's 5-minute PR
  budget for the Swift test step fires before `swift_test_limit.py`'s 300 s
  per-test limit, so on a PR the limiter never names a test; the failure report
  step does. On the queue and main the limiter still ends a stuck run inside the
  20-minute guard. Lowering the per-test limit below the PR budget is a
  measured call, not a merge call.
- The branch's `planning status` assertion inside
  `every_recipe_builds_a_world_lf_reads` left with the test main removed
  (LOO-564 restores the world suites). Carry it back with that restoration.
