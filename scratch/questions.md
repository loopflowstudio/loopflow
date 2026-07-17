# Assumptions — W2-311

Headless run: recorded and proceeded.

- `lf doctor` may perform one single-ref fetch because freshness against a stale
  remote-tracking ref would recreate the defect. Failure warns and stops the
  comparison.
- `OffMain` is healthy for this check: a feature-branch development build is
  different from a release binary missing merged main.
- `Unprovable` warns rather than fails so running doctor outside a checkout does
  not break a cron gate while still making absence loud.
- Rebuilding remains an operator action. This Task only reports the gap.
