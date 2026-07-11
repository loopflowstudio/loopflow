# Review: Linear-backed Task Sessions

## What was implemented

Loopflow now gives concrete work one formal lifecycle: a Linear task resolves
through the owning Wave's SQLite PM snapshot, reserves one durable Task Session,
creates one immutable sibling worktree from `main`, and runs one provider
harness under structured, persisted control. The CLI exposes create/run,
status, follow-up, steer, interrupt, wait, resume, attach, and abandon; `lf status --json`
and the Mac project the same Task Session state.

The competing lifecycle was removed rather than bridged: generic `lf loop`,
`lfq`, generic exec routes, stack/queue/combine delivery, Wave rotation, and
local Project mirrors are gone. Linear owns Project definitions, KRs, and
tasks; SQLite owns one atomic read snapshot and the durable Task Session,
command, and event records.

The gate pass fixed the state-machine failures it could exercise locally:

- process capacity is now reserved transactionally on resume, not only on the
  first run;
- merged sessions always attempt Linear completion, including merge observed
  by the runner, and a pending writeback first reconciles before safely retrying
  the mutation;
- every failure/merge path records a status transition, and attachment rejects
  inactive processes with a resume instruction;
- a process attributed to another Wave is refused before command persistence
  instead of being relabeled as a human.

The steering benchmark then split provider-dependent `send` into explicit
`follow-up` and `steer` operations. Commands now carry persisted, claimed,
accepted, failed, or superseded state plus their actual effect; interrupt with
a replacement supersedes older input transactionally; and unresolved commands
relaunch an inactive nonterminal session after a turn-boundary race.

Migrations 065–066 repair dogfood databases that recorded the Task Session
migration before the `agent` column or command receipts joined the schema.
Migration 066 also converts already-accepted commands and legacy events so an
upgrade cannot replay acknowledged input. Fresh databases converge through the
same migration chain.

## Key choices

- Linear identity is mandatory before placement, but an acceptable cached PM
  snapshot can launch already-identified work without a live SaaS round trip.
- The Task Session—not tmux or a provider thread—is the durable identity. Tmux
  owns process lifetime; provider session ids remain resumable implementation
  state.
- Every Task targets `main` and owns at most one PR. Deferred dependency and
  multi-task integration modes do not appear as speculative enum variants.
- Commands are durable and generation-claimed so an unacknowledged command is
  reclaimed after process death.
- The Wave remains the human-facing mind. Task commands and terminal results
  mirror into its journal without copying raw child tool chatter.
- Swift consumes `lf --json` and the shared PM snapshot rather than owning a
  second mutation or lifecycle model.

## How it fits together

`lf pm` writes Linear and atomically refreshes `PmShowResult`. `lf task run`
copies one task/Project launch receipt into SQLite, reserves capacity, creates
the worktree, and starts `lf __task` in tmux. The runner resumes the configured
harness, claims durable commands, emits Task events, observes PR state, and
folds consequential state back into the owning Wave; GitHub hooks provide the
independent merge/cleanup path.

## Risks and bottlenecks

- Provider-control errors now resolve the claimed command to a durable
  `failed` receipt before the Task Session fails.
- Boundary recovery exists, but crash stages and live redirect/replacement
  behavior have not passed the planned Codex/Claude/OpenCode black-box suite.
- Typed Task observations, Task-to-Wave decisions, atomic boundary settlement,
  waitable receipts, and the three provider capability profiles are now
  implemented. The complete 10-scenario × 3-adapter scripted-peer matrix and
  live Linear/provider/PR dogfood remain parity evidence; keep the PR draft
  until one of those gates closes the black-box coverage gap.
- Live Linear/provider create→run→steer→merge was not executed during this
  headless gate because it creates external records, worktrees, provider spend,
  and a PR. The deterministic store, migration, parser, PM, and lifecycle tests
  pass.
- The diff is intentionally broad: 173 files, 7,444 non-scratch additions and
  9,347 deletions. Review state ownership and failure paths before UI polish.

## What's not included

- Multi-task integration into one PR or dependency edges between tasks.
- Remote Task Session transport or a task-specific server.
- Rich direct Task transcript/steering UI; the Mac projects Task Sessions into
  its existing run surface.
- Automatic cleanup for failed/blocked sessions.
- The side-effecting live lifecycle dogfood and the complete scripted-peer
  conformance matrix named above.

## Validation

- `uv run python scripts/test.py --all` — all six suites pass: 53 Python tests;
  Rust format/clippy plus 1,284 nextest passes (3 intentional skips);
  59 website tests (3 intentional skips); 298 Swift tests plus the
  multiplatform boundary check; CLI/API e2e smoke; signed macOS
  `build-for-testing`.
- The focused migration 066 repair test passes after removing the trailing
  whitespace caught by `git diff --check`.
- Focused regressions prove atomic resume capacity, forward repair of the
  dogfood Task Session and command schemas, receipt/supersession persistence,
  parser separation, and cross-Wave command refusal.
- `bash -n scripts/demo_wave.sh` and `git diff --check` pass.
- The final branch is net-negative outside `scratch/`: 1,903 fewer lines.
