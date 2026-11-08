# LOO-434 open choices (kickoff, headless)

- **"Keep the repository healthy"** has no matching builtin Graph. Candidates: a new
  `maintain` graph (`reduce` survey → interactive pick → pursue → deliver), or leave the
  outcome out of New Task. The plan leaves it out until chosen.
- **"Fix a bug" → `code`** assumes implement handles a bug as well as `debug` would.
  Alternative: a `fix` graph starting with `debug`. Simpler option taken.
- **Moves implementation home.** LOO-733 found the same move gaps. Assumed: LOO-434
  ships `TaskGraph.moves`; LOO-733's panel consumes it. No comment was posted on
  LOO-733.
- **Taskless Continue/Stop prompt** defaults Enter to Continue. Alternative: no default,
  require `c` or `s`. Enter-continues keeps today's flow for people who exit cleanly.
- **Instruction storage**: content-addressed `skill_texts` table over storing full text
  per run (~65 KB × every run for `feature`).
