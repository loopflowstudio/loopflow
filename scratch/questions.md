# Open choices (made headless, for review)

- A steer from anyone but the Run's own Graph does not wake a finished Run
  (non-interactive Session); it is saved and reported unread. Passed over:
  any named steer wakes a finished Graph step.
- The Machine server's wake watcher starts a stopped interactive Session in a
  held terminal (`session.continue`'s launch); the steering command only
  saves. Passed over: the kickoff's detached `lf -b session wake ID` started
  by the steering command, with its own lock and loop (a second waker).
- Each steer starts its Session at most once; existing undelivered steers are
  marked as already started, so a server restart or the upgrade wakes nothing
  old.
- With no Graph step running, a Task steer wakes only the primary
  conversation. Passed over: holding it for the next step; waking the last
  finished step.
- `lf -b steer` to a running Session saves and returns.
- `lf steer` exits 0 once saved. `lf -b steer` exits with the turn's failure.
- A foreground turn's prompt carries each steer's full text, not the hook's
  400-byte excerpt.
