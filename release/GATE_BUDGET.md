# Full local gate budget

`uv run python scripts/test.py --all` is the full pre-land gate. Every phase
runs under a wall-clock budget; an overrun is killed and reported as a
`TIMEOUT`, so the gate always terminates. Budgets are the source of truth in
`scripts/test.py` (`PHASE_BUDGETS`); this file records what they are and what a
real run on this repo measured against them.

## Per-phase budgets

| Suite | Phase | Budget | What a pass proves |
|-------|-------|-------:|--------------------|
| python | python | 600s | `pytest python/tests/` |
| rust | rustfmt | 120s | `cargo fmt --check` |
| rust | clippy | 900s | `cargo clippy --all-targets -D warnings` |
| rust | rust | 1200s | `cargo nextest run --all` (or `cargo test --all`) |
| website | website | 900s | `website/dev.py test` |
| swift | swift | 1200s | `swift test` |
| swift | swift-boundaries | 120s | multiplatform boundary check |
| loopflow | xcodegen | 180s | project generation |
| loopflow | xcodebuild | 1200s | app + UI runners **compile** (not run) |
| e2e | e2e-smoke | 600s | CLI smoke |
| ui-host *(required, not in `--all`)* | ui-host | 1200s | hosted `LoopflowUITests` **execute** on a permissioned host |

`--all` total budget: **~7020s** (~117 min ceiling). The ceiling is deliberate
headroom over the measured run below — a healthy suite never trips it, a hung
one always does. The `ui-host` gate is separate and never counts toward `--all`.

## Reproduce

```bash
uv run python scripts/test.py --all        # bounded full gate; prints elapsed / budget per phase
uv run python scripts/test.py --list --all # the plan and every phase budget, run nothing
uv run python scripts/test.py --history 30 # judge the durable 30-day budget window
uv run python scripts/test.py --ui-host    # the required hosted UI gate (permissioned host)
```

The summary prints each phase and suite's `elapsed / budget` and a total line.
On failure the phase log (and any `.xcresult`) is kept under
`.lf/tmp/gate/run-<pid>/`.

## Durable evidence

Every selected phase is checkpointed under:

```text
<git-common-dir>/loopflow/pre-land/runs/<kind>/<run-id>.json
```

The Git common directory is shared by linked worktrees and survives Task
worktree pruning, `.lf/tmp` reaping, and machine restart. Records contain only
the run identity, branch, commit, timestamps, phase status, elapsed time, and
the budget that governed that run. Logs, commands, paths, output, diffs, and
environment values stay out of the durable record.

`--all` records are the 30-day authority. Losing one would bias the evidence,
so a write failure stops the full gate with `MEASUREMENT FAILED`. Changed-aware
and required-host records are diagnostics; their write failures print one
`MEASUREMENT WARNING` without changing the test result. `--history 30` reads
only `full` records and reports:

- `IN PROGRESS` while clean full-run history is younger than 30 days;
- `NOT HOLDING` for any over-budget, incomplete, or unreadable full record in
  the trailing window;
- `HOLDING` once the full window has at least one complete run and every full
  run in it stayed inside its captured budgets.

## Measured (this repo)

Headless measurement from this branch's CI run (GitHub `macos-15` / `ubuntu-latest`
runners, run 29396456085) — the living repository, no local warm caches. CI runs
each phase as its own parallel job; the serial `--all` wall-clock is roughly the
sum, and cold local builds run longer, which is why the budgets sit well above
these figures.

| Phase (CI job) | Measured | Budget | Headroom |
|----------------|---------:|-------:|---------:|
| python-test | 19s | 600s | 32x |
| rust-lint (fmt + clippy) | 45s | 1020s | 22x |
| rust-test | 97s | 1200s | 12x |
| website-test | 57s | 900s | 16x |
| swift-test | 50s | 1320s | 26x |
| e2e-smoke | 63s | 600s | 10x |
| loopflow (xcodegen + build-for-testing) | 59s | 1380s | 23x |

Every phase finished an order of magnitude inside its budget on a clean runner.
The budgets are hang-catchers, not tight SLAs: a healthy phase never trips one, a
wedged runner always does. Re-measure with `uv run python scripts/test.py --all`
(the summary prints `elapsed / budget` per phase) when a phase's real time drifts
toward its budget.
