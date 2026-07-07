# Flowloop gate review

## What was implemented

Added a shared `flowloop` runtime with tier binding for wave, project, and task
passes. The runtime runs bounded `*-pass` flows, tracks pass and wall-clock
limits, and polls deterministic oracles instead of asking the agent whether it
is done.

`lf task <linear-item-id>` now resolves a Linear roadmap item, creates a
run-scoped worktree, runs `task-pass` cycles, records the PR on the run, waits
for GitHub to report the PR merged, then comments the PR link on the Linear item
and closes it. The project tier is available as library code around kr-labeled
Linear items. The live wave resident now runs pass-based `wave-pass` turns from
queued messages, heartbeat, and cron wakes, with `--no-flowloop` /
`--flowloop-only` as the CLI surface.

## Key choices

- Keep tier behavior in builtin flows and step text. Rust only binds each tier
  to its pass flow, run lifecycle, bounds, and oracle.
- Treat "empty KR set" as blocked, not done. A project without KRs is unclear
  state, not completion.
- Wait on clean open task PRs without burning another pass. Human merge latency
  is not agent progress.
- Keep the existing wave listener, journal, supervisor, and resident wire. The
  resident changed how it produces turns; the listener remains the pen-holder.

## How it fits together

`FlowloopRun` creates a registry-backed fresh worktree for task/project loops.
`run_pass` invokes `lf -b <tier-pass> <seed>` with timeout and optional turn cap.
Task/project loops repeat that pass until GitHub or Linear reports terminal
state. Wave residency uses the same pass idea inside the existing listener /
resident split: inbox, heartbeat, and cron wakes open a journaled turn, spawn a
`wave-pass` child, ship its output back through resident deltas, and close the
turn.

## Risks and bottlenecks

- The wave live smoke remains ignored because it spends provider tokens; local
  validation covered unit/integration behavior, not a live Codex-backed wave.
- `MindState` and some legacy "mind"/"worker" vocabulary still exist in wire
  DTOs, journal names, and old channel semantics. This is documented as the DTO
  follow-up rather than hidden.
- Project KRs currently depend on a Linear `kr` label. If teams use another
  representation, the project oracle will report an empty set and fail fast.
- Pass-based waves are coarser than persistent vendor threads: heartbeat is now
  four hours, and live steering degrades to queue-at-boundary behavior.

## What's not included

No `lf project` command, no prod-verification oracle, no CI/review repair loop
inside `lf task`, no budget accounting, and no `MindState` wire rename across
Swift/Python fixtures.

## Validation

Passed:

```bash
git diff --check main...HEAD
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run python scripts/test.py
```

`uv run python scripts/test.py` selected and passed Rust (`cargo nextest
run --all`) plus website (`cd website && uv run python dev.py test`): 1248 Rust
tests passed under nextest, then 61 website tests passed with 3 skipped.

Full matrix coverage:

```bash
uv run python scripts/test.py --all
```

The full matrix passed Python, Rust, website, Swift package tests, and e2e smoke
before failing in Concerto's macOS UI harness. The failure reproduced with an
isolated DerivedData directory:

```bash
cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -derivedDataPath /tmp/loopflow-flowloop-gate-xcode CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

Xcode built the app and reported the non-UI Swift suites passing, then failed
to attach `ConcertoUITests-Runner`:

```text
Early unexpected exit, operation never finished bootstrapping
Underlying Error: Test crashed with signal kill before establishing connection.
```

Skipped:

- Live wave smoke / manual wave demo, because it requires Codex CLI auth,
  network, and token spend.
