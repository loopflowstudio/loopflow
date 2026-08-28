## Evaluate

```bash
cargo test -p loopflow project_runner_control_resumes_task_across_phase_and_process_recovery -- --nocapture
uv run python scripts/check_architecture.py
uv run pytest python/tests/test_architecture.py -q
uv run python scripts/test.py --reuse-passing
```

The post-rebase behavior fixture passes on `ee72398d5`: a Project controller
resumes its parked child through the ordinary Task path, repeats the resume
without creating another Task Work, advances authority with the Project phase,
replaces it after controller-process loss, and denies stale or superseded
callers.

The architecture checks pass with all 35 live SQLite owners mapped, including
the new scoped capability row.

The changed-aware gate selects full Python plus release-materialized Rust. Its
latest local run stopped before product tests because three unrelated active
worktrees exceeded the repository resource envelope. The rebased head's CI
passed Rust tests and lint, Swift, UI compile, E2E, migrations, and website; its
only product failure was the architecture-map omission now fixed locally and
proven by the focused checks above. Require the final exact-head CI matrix
rather than treating the local host-pressure stop as a product result.

## Why it matters

Project pursuit could previously reach a healthy provider process without the
optional execution evidence required to resume a parked child. Recommended work
then remained stranded even though the controller was live. Child control is
now explicit durable state owned by the immediate Project, so process recovery
cannot silently separate Project liveness from Task-resume authority.

## What changed

- Issue an opaque Project capability before provider work and store only its
  hash with the exact controller Run, phase, and Steer frontier.
- Rebind authority before phase transitions, replace it during controller
  recovery, and release it when control ends.
- Validate authority before resume reconciles PR state and again at child
  launch, without adding a bypass or a second public command.
- Preserve direct User resume while denying missing, unrelated, stale, and
  superseded in-Run authority.
- Scrub Project authority from child execution and shell contexts, and keep the
  behavior fixture valid after draft-migration materialization.
- Register the capability row in the checked architecture ownership map.

## Risks / Not included

The raw capability exists only in the Project provider environment, so the
shared execution-identity scrub list remains security-critical. This PR scopes
exact authority to resuming an existing parked Task; broader Wave-to-child
controls such as Task steer and Task run remain unchanged pending a coherent
Wave-owned capability design.
