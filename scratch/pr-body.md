## Try it!

```bash
lf task <linear-item-id> --wave designer
lf task <linear-item-id> --wave designer --max-passes 4 --wall-clock-secs 3600 --max-turns 20
lf wave designer --no-flowloop
lf wave designer --flowloop-only
```

`lf task` resolves the Linear item from the wave roadmap, creates a run-scoped worktree, runs bounded `task-pass` cycles, waits for the PR to merge, then comments the PR link on the Linear item and marks it done.

`lf wave <name>` now keeps the listener/resident split but the resident opens pass-based `wave-pass` turns instead of a persistent vendor thread. `--no-flowloop` serves only the listener, and `--flowloop-only` attaches the resident to an existing listener.

Project flowloops are built as a library tier for the next command/wave-spawn slice. They read kr-labeled Linear items, run `project-pass` while any KR is open, and refuse to start when the KR set is empty.

Validation:

```bash
git diff --check main...HEAD
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run python scripts/test.py
```

Changed-aware validation selected Rust and website suites: 1248 Rust tests passed under nextest, then 61 website tests passed with 3 skipped.

Full matrix validation with `uv run python scripts/test.py --all` passed Python,
Rust, website, Swift package tests, and e2e smoke before failing in Concerto's
macOS UI runner. Re-running Concerto with isolated DerivedData built the app and
reported the non-UI Swift suites passing, then failed to bootstrap
`ConcertoUITests-Runner` with `Early unexpected exit, operation never finished
bootstrapping`.

## Intent

This turns flowloop into the shared substrate for long-running agent work. Task, project, and wave tiers now use the same bounded pass pattern: the agent chooses moves inside each pass, while GitHub, Linear, or the wave's never-stop rule decide whether the loop is done.

## Assumptions

Waves are registered in the local run registry before task/project flowloops start. The PM provider is Linear, roadmap items expose labels, and GitHub CLI plus Linear auth are available locally. Human-gated merge remains the default for `lf task`: the runner exits only after GitHub reports the PR merged. Live wave smoke still requires Codex CLI auth, network, and token spend.

## Key decisions

Tier behavior lives in builtin step text, not Rust branching. Rust binds each tier to a pass flow and oracle, then enforces bounds and records state.

The project KR oracle uses a `kr` Linear label for this slice. Empty KR sets fail fast because "no KRs" is an unclear project, not a completed one.

Clean open task PRs wait without consuming another pass. Waiting for a human merge should not look like failed agent progress.

The wave listener, journal, supervisor, and resident wire stay in place. Only the resident's turn producer changes: inbox, heartbeat, and cron wakes spawn a bounded `wave-pass` child and publish its output back through the existing resident deltas.

## Not included

This does not rename the `MindState` wire surface, add `lf project`, implement prod-verification oracles, add budget accounting, or add CI/review repair loops inside `lf task`.
