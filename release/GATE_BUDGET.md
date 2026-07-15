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
uv run python scripts/test.py --ui-host    # the required hosted UI gate (permissioned host)
```

The summary prints each suite's `elapsed / budget` and a total line. On failure
the phase log (and any `.xcresult`) is kept under `.lf/tmp/gate/run-<pid>/`.

## Measured (this repo)

<!-- MEASURED -->
