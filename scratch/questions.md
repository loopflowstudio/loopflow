# LOO-434 open choices

Explore round 1: [explore.html](explore.html). No choice recorded.

1. Organize the Task around **a** its route, **b** its next decision, or **c** a
   past delivery to reuse? Recommendation: b; history still supports reuse.
2. Taskless Continue: **a** require `c`, or **b** Enter continues? Recommendation:
   a; provider exit alone never advances in either proposal.

## Kickoff implementation questions (still drafts)

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
