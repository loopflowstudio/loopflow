# Questions and assumptions — LOO-349

- **Pilot, not a powered study.** Replays spend Jack's subscription accounts and
  implement turns take 5–30 minutes, so the cohort is every recoverable
  loop-decide record plus a few implement/compress records, n≈1–5 per cell.
  Verdicts that rest on n=1 are labelled directional, and budgets only move
  where the census and replays agree.
- **Resumed history is outside replay.** All 95 replayable launches are fresh
  (`op: initial`), and `lf replay` never resumes a native session. The study
  reports what the census can say and leaves resumed-history ablation unmeasured.
- **Installed lf 0.12.28 ignores `LF_HOME` for its store.** The first pilot pass
  ran with it; nested `lf` calls inside replays read the real store and Linear
  (`lf task comment LOO-348`, `lf task status`). One replayed loop-decide ran
  `lf flow decide iterate`, which failed with "this checkout has no Task". No
  write reached Linear or a Task. Those replays' Session rows may remain in
  `~/.lf/loopflow.db` pointing at deleted `/tmp` records. The study now uses the
  branch-built lf (main #1386 keeps one database per Home) and puts it first
  on the replay's `PATH`.
- **Launch commit.** Manifests do not record HEAD. The collector uses the source
  worktree's reflog, then a first implement step's `Base commit:`. For three
  loop-decide records whose worktree is gone, the commit is the last PR #1354
  head commit before launch, passed with `--commit`; uncommitted state is lost.
- **Scratch on disk is held constant.** Every arm writes the prompt's scratch
  files into the checkout, so `no-scratch` measures the prompt copy, which is
  what the budget controls; agents can still read the files.
- **Scratch budget lowered to 12,000 tokens / 96 KiB.** The only budget the
  pilot supports moving. It covers the Task's own notes at p90 (12.4k) and the
  replays found no outcome change beyond noise when inline scratch was removed.
  Compress without inline scratch produced the smallest diff at n=1, so this is
  the cautious end of "trim"; a repo or Wave `context_budgets` entry restores
  16,000. Memory, goal and total input are unchanged: LOO-362 raised memory to
  16,000 on 2026-10-01 and nothing here contradicts that.
- **Pilot spend.** 31 replays plus four smoke/aborted passes; Claude reported
  $21.57 for the 11 retained Claude replays. Disposable Homes and checkouts are
  under `/tmp/lf-ablation*` and can be deleted.
- **Metric proposal for the Wave.** Outcome: launches stay small relative to the
  turn. Measure: launch tokens ÷ peak input per step, from `census`. Decision
  value: tells whether to spend effort on launch budgets or in-turn reads.
  Cheapest producer: `scripts/context_ablation.py census` in `telemetry-daily`.
  Not wired here; LOO-348 owns the weekly context report.
