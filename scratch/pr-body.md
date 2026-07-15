## Try it!

Run the shared projection and fixture checks:

```bash
cargo test -p loopflow lf::commands::waves
cd swift && swift test
```

The Rust suite proves the eight-state attention decision table and that
`lf roadmap` prints the shared reason verbatim. The Swift suite decodes the
same fixture and proves NOW membership, controls, and accessibility text.

For the full local gate:

```bash
uv run python scripts/test.py --all
```

## Intent

Give terminal and Mac surfaces one explainable Task attention signal. Rust
projects PM, Session, process, PR, and local-progress evidence into green, red,
black, or unknown with one reason and a set of safe controls. Swift consumes
that contract directly instead of maintaining another lifecycle state machine.

## Assumptions

- Durable Task Sessions own workspace identity.
- One machine-wide tmux snapshot is the process-liveness boundary.
- An active PR's immutable base commit is the comparison point for unsettled
  Task-authored commits.
- Missing evidence must remain visible as unknown rather than defaulting to a
  clean state.

## Key decisions

- Keep PM completion independent from attention.
- Carry constituent evidence in the DTO so explanation UI never reruns Git.
- Derive legal lifecycle controls in Rust and share them with every consumer.
- Bound Git work to one cleanliness probe and, for an active PR, one HEAD read
  per Task with a durable Session.

## Not included

Project/Wave attention, history and Audit presentation, Project blocker
preservation, and joining the interactive-handoff store contract into Task
attention remain follow-on slices.

The CI-equivalent local matrix passes. Two separate hosted UI attempts were
stopped by macOS UI-automation authentication before the test runner
initialized; package tests and the signed app/UI-runner build pass.
