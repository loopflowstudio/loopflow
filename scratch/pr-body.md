## Try it!

```bash
# Build the branch CLI and inspect the two domain runtimes.
cargo build -p loopflow --bin lf
target/debug/lf project --help
target/debug/lf task --help

# With any synced Wave, inspect the cache-only native hierarchy.
target/debug/lf status infrastructure --json \
  | jq '{wave: .wave.name, projects: [.projects[] | {project: .project.name, runtime, directive, next_move, tasks}]}'
```

The status result contains Linear planning records even when no runtime exists,
then joins any Project/Task Session, current directive and incorporation state,
next-move owner, Task delivery, and attention. Project and Task records are not
flattened into generic Runs.

Run the deterministic gate:

```bash
uv run python scripts/test.py --rust --python --swift
tests/e2e/test_smoke.sh
```

Final local results: 1,315 Rust tests passed (3 skipped), 52 Python tests
passed, 59 website tests passed (3 skipped), 297 Swift tests passed, the e2e
smoke passed, clippy was warning-free, and the macOS UI target completed
`xcodebuild build-for-testing`.

## Intent

Replace detached free-text workers and stacked delivery with one coherent
product hierarchy. Humans talk to a Wave; the Wave directs measured Projects;
Projects supervise concrete Tasks; each Task alone owns its worktree, provider
history, PR, review repair, merge, and Linear completion. The same hierarchy is
visible in Wave Chat with durable evidence that direction was persisted,
applied by the provider, and explicitly incorporated by the child.

## Assumptions

- Linear is the durable planning authority. SQLite is a cache and runtime/control
  ledger, not a second Project or Task authoring surface.
- Every Task belongs to exactly one Project before a worktree is allocated.
- Wave and Project turns use a clean canonical `main` checkout as a
  read-and-coordinate surface. Repository mutation belongs to a Task Session.
- MVP Task delivery is zero or one PR targeting `main`.
- Local tmux processes and the shared SQLite registry remain the execution
  substrate; remote Project/Task execution is outside this change.
- Migrations 062–069 are a forward migration for the dogfood database. Migration
  069 backfills sessions that existed before the directive ledger.

## Key decisions

- **Version intent, not transport.** Initial direction is persisted before
  provider launch. Replacement steering advances a monotonic directive version;
  follow-up does not. Provider acceptance proves application, while a child
  acknowledgement proves incorporation.
- **Root authority, local supervision.** A Project normally controls its Tasks,
  but the owning Wave can inspect and override every descendant. Material
  outcomes reach the Wave without copying raw child tool chatter into its
  conversation.
- **Keep one composer.** Wave Chat renders linked child activity and a native
  Project → Task work map. Decisions and “Tell Wave about this” return to the
  Wave composer instead of opening peer child chats.
- **Task owns delivery.** Wave-level branch, diff, PR, land endpoints, DTO fields,
  and keyboard controls were deleted. Project and Task state is no longer
  adapted into fake Swift Runs.
- **Controllers own lifecycle.** Wave, Project, and Task run their
  clarify/pursue/mutate policy flows, but deterministic controllers decide when
  to repeat, wait, block, complete, or abandon. No agent-authored loop bit
  remains.
- **Delete the competing runtime.** Public generic loop, `lfq`, queue, stack,
  `combine`, and `next` surfaces are removed rather than bridged.

## Not included

- Task-internal Workers or a worker scheduler; no placeholder worker count is
  exposed.
- The planned ten-scenario × three-provider crash/decision/observation
  conformance harness.
- A side-effecting live two-Task Linear/GitHub dogfood run.
- Provider approval mapping beyond the durable decision protocol.
- Remote execution, alternate PR targets, stacked delivery, or a generic
  multi-product session framework.
- Rich raw child transcript browsing and UI screenshots. Deterministic Swift
  tests and build-for-testing pass; the known headless UI runner connection hang
  remains unproven.

The detailed architecture and risk review is in
`scratch/infrastructure-review.md`.
