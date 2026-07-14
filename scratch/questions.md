# Open questions — W2-130

## Assumed, proceeding

- **`lf commit: wave_pursue` committed to canonical `main`.** That commit
  (`4fbc980f`, `wave/infrastructure/MEMORY.md`) is the source of the task-base
  contamination in failure 8, and it contradicts `ensure_clean_main`'s own error
  text ("Wave and Project turns never edit repository files"). W2-130 makes it
  *harmless* by basing Tasks on `origin/main`; it does not stop a wave turn writing
  to `main`. **Assumption:** that is a separate task under Loopflow API, not W2-130
  scope. Folding it in would double the blast radius of a recovery change.

- **This branch is itself contaminated.** `jack-heart/w2-130`'s base is `4fbc980f`,
  which is in no remote branch but `origin/jack-heart/w2-132`. It must be rebased
  onto `origin/main` before the PR opens, or W2-130 ships the bug it fixes.

- **Debug-build guard (move 2) is a hard error.** If it turns out to break tests that
  rely on the default `~/.lf/loopflow.db` path, it downgrades to a `lf doctor` check
  rather than blocking the PR.
