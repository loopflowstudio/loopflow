# Open choices (made headless, for review)

- A steer from anyone but the Run's own Graph does not wake a finished Run
  (non-interactive Session); it is saved and reported unread. Passed over:
  any named steer wakes a finished Graph step.
- The detached waker is a hidden `lf -b session wake ID`, started by
  `engine::session::steer` so `lf steer` and Desktop's `session.steer` share
  it. Passed over: re-running the caller's argv detached (no argv on the
  server path).
- `lf steer` exits 0 when the steer saved but the wake failed to start, with
  one line saying so. `lf -b steer` exits with the turn's failure.
- A woken turn's prompt carries each steer's full text, not the hook's
  400-byte excerpt.
