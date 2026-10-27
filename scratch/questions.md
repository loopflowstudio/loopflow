# Open questions / assumptions

- 2026-10-27 (kickoff): Same-Machine, same-login takeover adds a notice
  ("This conversation moved to another terminal on this Machine.") after the
  transport's "Left …" line, for Claude and Codex alike. The brief only names
  the account and Machine forms; the steer asks to cover same-account takeover.
  Simpler alternative if unwanted: print nothing extra for `Departure::Client`.

- 2026-10-27 (implement): same-login `connect --takeover` with a live transport
  owner hands the *view* to the new terminal and never stops the agent, so the
  `Client` notice only appears when the old launch has no live owner. The
  design's same-login demo therefore does not exercise this path; the account
  move does. Left as is; whether the old viewer should also print something on
  a viewer handoff is a separate question.
