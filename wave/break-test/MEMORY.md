# break-test wave memory

## Learnings

- 2026-04-22: `lf debug` headless + empty input happened twice
  (branches `...20260422_1307` and `...20260422_1308`). First run
  logged the issue to `scratch/questions.md` and exited. Second run
  added `.lf/steps/debug.md` override that short-circuits on headless +
  empty input with a one-line message. Interactive flow unchanged.
  If this recurs with the override in place, the issue is the
  short-circuit isn't firing — check how the harness surfaces
  "headless" to the step and whether clipboard emptiness is detectable
  from within the step prompt.
