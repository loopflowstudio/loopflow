# Measure which context changes outcomes (LOO-349)

Status: implemented 2026-10-01 by the supervising session; pilot run, budgets adopted. Jack Heart requested
the study on 2026-09-30 and is not reviewing this Task.

## Outcome

Jack can read, for each implicit context source, whether to keep, trim or drop
it, with the evidence behind the verdict and its limits, and the launch budgets
from LOO-346 carry those verdicts.

## What the records allow (observed 2026-10-01, `~/.lf/runs`, lf 0.12.28)

- 768 records; 95 are implement / compress / loop-decide launches carrying a
  replayable request, all with `context.json` asset ranges and token weights.
- All 95 are `op: initial`. No replayable record is a resumed turn, and
  `lf replay` never resumes, so **resumed history cannot be ablated by replay**.
- `<lf:steers>` appears in 3 of 95; agent progress markers in 0 of 95. Both are
  already excluded by LOO-346 (#1362). Their cost is measured, not replayed.
- Memory (43 records) and scratch (83 records) are the two sources large and
  frequent enough to ablate.
- The manifest does not record HEAD at launch. The source worktree's reflog
  recovers it while the worktree exists; uncommitted state at launch is lost.

## Approach

No change to `lf replay`, which keeps meaning "launch the exact recorded request".
The study stages a *variant* record in a disposable Home and replays that.

`scripts/context_ablation.py` (stdlib, no provider code of its own):

- `census` reads every replayable step record and reports tokens per source,
  share of submitted input, presence, and the original turn's time and usage.
  Zero spend.
- `replay` takes one record and one arm. It clones the repository to a
  disposable checkout at the launch commit with no remote, writes the prompt's
  scratch files to disk, creates a Home holding only the variant record and a
  link to provider accounts, strips ambient `LF_*`, and runs `lf replay` with a
  wall-clock limit. Disk state is identical across arms; only the prompt differs.
- `report` compares each arm with the baseline replay and the original record:
  outcome, decision (`lf flow` verb and final text), changed files, check
  commands, turn time, usage.

Arms: `baseline`, `no-memory`, `trim-memory` (4,000 tokens, both ends kept),
`no-scratch`, `own-scratch` (only the Task's design note and questions),
`no-steers`.

Isolation: the disposable Home has no `credentials/`, so Linear and Doppler are
unreachable; the clone has no remote, so nothing can be pushed.

## Pilot cohort

Provider spend is real and comes from Jack's subscription accounts, so the
cohort is a pilot: every loop-decide record whose launch commit is recoverable,
plus one implement and one compress record, each across the arms its prompt
supports. The findings state n per cell and never promote n=1 to a rule.

## Delivery

- `scripts/context_ablation.py` and `python/tests/test_context_ablation.py`.
- `performance/context-ablation.md`: findings, verdict per source, limits, and
  the commands to extend the cohort.
- Budget defaults in `engine/context_budget.rs` and their docs adopt the
  verdicts the evidence supports.
- Wave memory records the verdicts and the evidence boundary.

## Delete — do not maintain

Nothing is slated for removal: this adds a collector and changes constants.

## Remaining

- Extend the cohort: several implement and compress records per Wave, each
  with a repeat baseline, and a reviewer's judgment of the ablated diffs.
- Resumed history has no instrument. It needs turn-level evidence, not replay.
- Record HEAD in the Run manifest at launch so replays do not depend on a
  surviving worktree reflog. Not built here; proposed to the Wave.
- Once a release carries #1386, drop the "build this checkout's lf" step from
  `performance/context-ablation.md`.

## Checks

`cargo test -p loopflow --lib context_budget` and `engine::exec` (47 passed), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `ruff`, `pytest python/tests/test_context_ablation.py` (3 passed): pass, 2026-10-01. Affected suites: gate.
