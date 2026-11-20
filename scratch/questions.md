# From qa, 2026-11-19

- After the tmux fix a provider found only on the caller's PATH (not the tmux
  server's, not a tool directory) fails with "'claude' is not installed". Should
  that message name the PATH it searched, or is the tool-directory list enough?
- `lf open --focus` names the last view opened; with that pane closed and an
  earlier view still attached it fails "can't find pane". Not from this diff.

# From human-review, 2026-11-19 (nobody present)

- Went on to delivery: no promised behavior missing or broken. Questions 1-4 on
  `scratch/human-review.html` are unanswered.
- PR #1994's CI run on `f6df3cc94b` skipped `rust-test`, `rust-lint`, `rust-build`
  and `macos`; the suites have not run on this diff. Gate or the merge queue runs them.
