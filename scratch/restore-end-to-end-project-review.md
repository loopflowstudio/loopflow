# Restore end-to-end Project and Task controllers

## What was implemented

Local release promotion now hands every live Project and Task controller from the prior install to the target install. The switch receipt captures the exact startup attempt, quiesces its birth-validated OS owner before store advancement, restarts the same Work through the selected target artifact and store, and settles only after a distinct target attempt is authoritative.

The same controller authority joins startup receipts, Exec receipts, OS process birth identity, and bounded tmux pane evidence for status, control, promotion, rollback, and recovery. A parked Task receipt also names its exact human Flow position, so it stops blocking relaunch after that boundary has advanced.

## Key choices

- Stop and restart controllers instead of allowing adjacent release generations to coexist. This preserves one owner per Work and prevents the prior binary from writing the prior store after selection moves.
- Persist every transition in `SwitchReceipt`. `captured`, `quiesced`, `parked`, and `restarted` are monotonic evidence, and receipt validation rejects advancement or settlement with incomplete handoffs.
- Signal only an exact Work-linked owner whose PID and process birth still match its Exec receipt. Missing identity or an unrelated tmux pane is unverifiable, not absence.
- Hold the promotion coordinator exclusively while normal controller launches hold its shared side through their startup outcome. Recovery bypass is scoped to the active receipt and lock owner.
- Treat tmux as transport evidence, not process authority. Both pane and session probes are bounded; tmux's successful empty pane response is disambiguated with an explicit session probe.
- Keep parked state position-specific. A historical parked receipt becomes inactive once durable Flow position moves beyond that human node and iteration.

## How it fits together

Ordinary Project and Task launchers reserve an attempt under the shared promotion lock and publish a startup receipt only after the child proves its Run and Exec owner. Promotion takes the exclusive lock, discovers those same owners, and persists each handoff around quiescence, store advancement, and target restart. Rollback and interrupted-switch recovery call the same receipt-driven convergence functions, so there is no second recovery authority model.

## Risks and bottlenecks

- Controller discovery and validation probe registered Project and Task transports serially. A Home with a large controller registry will add bounded per-controller promotion latency.
- Graceful stop has a bounded fallback to TERM and KILL, but only after revalidating the exact owner before each signal. Provider descendants remain outside promotion authority and depend on normal controller shutdown.
- The production-shaped controller proof is intentionally slow: it compiles public commands and exercises SQLite stores, OS processes, signals, switch receipts, rollback, and recovery. Tmux, provider, and Linear are local doubles.
- The changed-aware gate wrapper could not enter its test plan because the active main worktree's build directory was 18.4 GiB against a 12 GiB preflight ceiling. Bounded recovery correctly refused to remove an active worktree. The exact Rust, website, lint, formatting, and architecture commands selected by that plan were run directly instead.

## What's not included

- Wave listeners and the Home keeper retain their separate service-replacement contract.
- Generic Runs, provider processes, terminals, and other unregistered processes are not promoted.
- The change adds no second-generation lease, controller namespace, UI, or compatibility authority.
- It does not start a controller that was absent before promotion or automatically resume a controller parked at its still-current human boundary.

## Validation

- `uv run python scripts/materialize_rust_tests.py -- cargo nextest run --all --build-jobs 4 --test-threads 4 --no-fail-fast` — 1,913 passed, 6 skipped in 344.486 seconds.
- `cargo test -p loopflow --test controller_startup_tests public_project_and_task_controllers_prove_startup_and_resume -- --test-threads=1` — focused Done When proof passed; the same test passed on the final materialized tree in 100.907 seconds.
- `cd website && uv run python dev.py test` — 78 passed, 3 skipped in 56.08 seconds.
- `cargo fmt --all -- --check && cargo clippy --all-targets --jobs 4 -- -D warnings` — passed.
- `uv run python scripts/check_architecture.py` — all eight ownership and boundary maps passed with zero unexplained or stale entries.
- `git diff main --check` — passed.

The gate review tightened three observable contracts before the final run: terminal handoff receipts cannot settle incomplete, parked receipts expire with their exact Flow boundary, and successful-but-empty tmux pane output cannot masquerade as either ownership or failure.
