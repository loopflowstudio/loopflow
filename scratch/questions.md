# Open questions

- **Row speed target.** Read as met per decision (rows ~0.4 s adopting,
  under 0.1 s refusing or binding; family tests 1.3–3.3 s); see "Next
  implement direction" in `decisions-in-process.md`. Still open outside this
  Task: whether `prepare`'s foreground origin fetch and planning push (~0.7 s
  of git on every `lf task create --session/--worktree`) should move off the
  foreground.
