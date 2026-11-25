# LOO-1108 kickoff

- Assumed: nothing replaces verify's post-merge check (post-merge CI for the merge SHA, claims against main, fix forward). Passed over: a post-merge step inside `deliver`; keeping `verify` as an unreferenced builtin.
- Assumed: the outer loop in `code`/`feature` goes with verify (a loop cannot end in `cmd: land`).
- Other repositories on this Machine name the builtin and break on new take-ups after release: `manabot` and `fantasia` (`- interactive: verify` in coverage, engine-performance, experiment-performance Graphs), `etude` and `etude-sdk` (`.lf/skills/verify.md` with `extends: builtin`). Hand edits there; not part of this Task.
