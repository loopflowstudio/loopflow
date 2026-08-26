# Gate review: Make PR landing repair observable and terminating

## What was implemented

Watched PR landing now owns a bounded repair attempt from hosted evidence to
re-arm. It fetches the exact failed GitHub Actions jobs, records their URLs and
digest, launches `ci-fix` at a clean failed head with isolated Loopflow
authority, enforces a ten-minute provider deadline, accepts only one material
uncommitted worktree delta, and settles both the landing and CI incident on
every unusable result.

`lf ci` renders the controller-owned repair receipt as `running`, `repaired`,
`blocked`, or `superseded`, derived from durable incident and landing truth.

## Key choices

- The controller fetches hosted logs before delegation. The repair provider
  never needs GitHub access to discover the failure.
- Inspection and repair share Actions URL parsing and bounded command
  execution, while preserving their distinct public contracts: `lf wt ci
  --logs` returns the full job log and repair receives a bounded failed-step
  tail.
- CI repair uses a narrow exclusive writer preflight. Ordinary agents retain
  the existing dispatch model; the branch does not introduce a general
  worktree lease.
- The provider leaves an uncommitted fix. Loopflow remains the only component
  allowed to commit, rebase, push, and re-arm the PR.
- The repair receipt adds immutable evidence and timing facts, not a second
  lifecycle enum. Outcome derives from the existing incident and landing
  state.

## How it fits together

The landing supervisor observes an exact failed head, claims its CI incident,
acquires the repair writer, fetches and hashes each failed Actions job, and
persists the attempt before running the isolated provider. Provider teardown
completes before the supervisor validates the worktree and either re-arms an
advanced head or atomically blocks the landing and incident.

## Risks and bottlenecks

- GitHub log retention, authentication, or availability can prevent repair.
  That path now blocks before provider launch and names the exact check URL.
- A provider can consume at most ten minutes before process-group teardown.
  Slow valid repairs beyond that boundary are intentionally rejected.
- Writer exclusion is repair-specific. Ordinary agent concurrency continues to
  rely on Task dispatch discipline, consistent with the wave's one-writer
  model.
- The standard changed-aware runner could not pass its shared-host resource
  preflight because three unrelated active worktrees exceed their per-root
  build budgets. No foreign process or build root was mutated. The runner's
  exact Rust and website commands were instead executed through its own
  process-group budget function.

## What's not included

- Generic provider terminal-event settlement remains LOO-265. Landing repair
  now tolerates a missing provider terminal event without changing that model.
- Global reconciliation of orphaned Exec spans is separate work.
- No hosted failure was manufactured for demonstration. The prior immutable PR
  #1237 job supplied read-only evidence; mutation was proved through the real
  store and landing supervisor in tests.

## Validation

- Materialized Rust phase: 2,113 passed, 2 skipped, 0 failed in 1,025 seconds;
  within the 1,200-second repository limit.
- Website/docs phase: 70 passed, 3 skipped, 0 failed in 41 seconds.
- `cargo clippy --all-targets --jobs 4 -- -D warnings`: pass.
- `cargo fmt --all -- --check`: pass.
- `git diff --check`: pass.
- Focused restoration suites passed for landing supervision, hosted log
  parsing, isolated environments, exclusive repair writing, provider process
  group teardown, and `lf ci` rendering.

The first full diagnostic found ten regressions: seven ordinary agent launch
tests were caught by repair-only writer exclusivity, and three `lf wt ci
--logs` tests lost their established full-log contract. Commit `e9e5efe28`
narrows exclusivity to CI repair and separates inspection from repair log mode;
all ten pass in the final materialized suite.
