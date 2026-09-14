## Evaluate

Run the production-shaped controller proof:

```bash
cargo test -p loopflow --test controller_startup_tests public_project_and_task_controllers_prove_startup_and_resume -- --test-threads=1
```

It starts public Project and Task controllers, promotes to a fresh local artifact, proves the prior owner absent before store advancement, observes one distinct target attempt, and recovers two forced interruptions from the switch receipt. Missing Exec identity and an unrelated tmux pane both reject promotion before advancement without signaling either process.

Run the affected suites and static checks:

```bash
uv run python scripts/materialize_rust_tests.py -- cargo nextest run --all --build-jobs 4 --test-threads 4 --no-fail-fast
(cd website && uv run python dev.py test)
cargo fmt --all -- --check
cargo clippy --all-targets --jobs 4 -- -D warnings
uv run python scripts/check_architecture.py
```

Final results: 1,913 Rust tests passed with 6 skipped; 78 website tests passed with 3 skipped; formatting, clippy, and all architecture ownership maps passed.

## Why it matters

A local release switch previously replaced the installed binary and store selection without owning active Project and Task controllers. An old controller could be stranded on the prior store or overlap a newly launched generation. Promotion now makes that execution transition durable, exclusive, and recoverable.

## What changed

- Added one controller authority contract backed by startup receipts, Exec receipts, OS birth validation, and bounded tmux transport probes.
- Added monotonic controller handoffs to the machine switch receipt and enforced capture, quiescence, target restart, rollback, and recovery through that receipt.
- Serialized ordinary controller startup against promotion through the shared/exclusive coordinator lock.
- Made parked startup evidence exact to its human Flow node and iteration so historical receipts do not block legitimate resume.
- Added a production-shaped integration fixture covering Project and Task startup, control, failure, promotion, and interrupted-switch recovery.

## Risks / Not included

Controller probes are serial and add bounded latency per registered Work. Wave listeners, the Home keeper, generic Runs, providers, and terminals keep their existing lifecycle contracts. Promotion never starts a controller that was absent and never resumes a controller parked at its current human boundary.
