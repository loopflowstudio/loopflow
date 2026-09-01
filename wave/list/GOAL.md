---
pm:
  linear_initiative: f6d2c0bd-c9bd-490a-a18e-8ed03216e6d4
---

## Objective

Loopflow's Task control plane keeps work safe, recoverable, and available while
process ownership, provider sessions, telemetry, and releases change
independently.

## Projects

Projects and tasks live in Linear and sync into the local SQLite registry.
Projects do not own memory, cadence, or child projects.

## Bounds

- Resolve control authority and release coexistence semantics before changing
  Task execution, recovery, or promotion.
- Keep provider history and telemetry useful without silently granting them
  operational authority.

## Process

Read the synced Project and its first open Task. Settle architectural forks in
one explicit design artifact before implementation, then use focused behavioral
proofs to keep Task control safe under missing processes, provider state, and
telemetry.
