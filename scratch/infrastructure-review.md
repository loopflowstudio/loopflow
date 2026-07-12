# Review: Linear-backed Project and Task Sessions

## What was implemented

Loopflow now gives concrete work one formal lifecycle: a Linear task resolves
through the owning Wave's SQLite PM snapshot, reserves one durable Task Session,
creates one immutable sibling worktree from `main`, and runs one provider
harness under structured, persisted control. The CLI exposes create/run,
status, follow-up, steer, interrupt, wait, resume, attach, and abandon; `lf status --json`
and the Mac project the same Task Session state.

Linear Projects now have the missing pursuit runtime. `lf project run` reserves
one durable Project Session in the Wave home, resumes one provider transcript
across generations, creates and supervises Linear-backed Task Sessions, sleeps
without a process while Tasks run, and wakes from a transactional observation
outbox. It owns no branch, worktree, or PR. Project and Task commands share one
`child_commands` store, `cc_` receipts, control intent, and `cd_` decisions;
their domain events remain distinct.

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

The final Task slice adds explicit receipt reads/waits, typed decision
requests and responses, and idempotent Task observations in the Wave journal.
Turn-boundary settlement now atomically chooses between claiming queued input
and stopping the process, so an instruction cannot land in the inactive gap.

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
- The Wave remains the human-facing mind. Consequential Task events enter its
  journal as typed, idempotent observations without copying raw child tool
  chatter.
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
- Task→Project and child→Wave delivery now use the durable observation outbox.
  Project consumption and acknowledgement are one SQLite transaction; Wave
  journal delivery is idempotent before acknowledgement.
- Live Linear/provider create→run→steer→merge was not executed during this
  headless gate because it creates external records, worktrees, provider spend,
  and a PR. The deterministic store, migration, parser, PM, and lifecycle tests
  pass.
- The diff is intentionally broad: 129 non-scratch files, 7,454 additions and
  7,676 deletions. Review state ownership and failure paths before UI polish.

## What's not included

- Multi-task integration into one PR or dependency edges between tasks.
- Remote Task Session transport or a task-specific server.
- Rich direct Task transcript/steering UI; the Mac projects Task Sessions into
  its existing run surface.
- Automatic cleanup for failed/blocked sessions.
- The side-effecting live lifecycle dogfood and the complete scripted-peer
  conformance matrix named above.

## Validation

- `uv run python scripts/test.py --all` passed Python (53), Rust
  format/clippy/nextest, Swift (298 plus the multiplatform boundary check),
  CLI/API e2e smoke, and signed macOS `build-for-testing`. The website suite
  exposed a pre-existing fixture deadlock: its undrained server log pipe could
  block arbitrary page loads. The fixture now writes to a seekable temporary
  log, preserves startup diagnostics, and the standalone website gate passes
  59 tests with 3 intentional skips.
- The focused migration 066 repair test passes after removing the trailing
  whitespace caught by `git diff --check`.
- Focused regressions prove atomic resume capacity, forward repair of the
  dogfood Task Session and command schemas, receipt/supersession persistence,
  parser separation, and cross-Wave command refusal.
- `bash -n scripts/demo_wave.sh` and `git diff --check` pass.
- The final branch is net-negative outside `scratch/`: 222 fewer lines.

## Project Session operational review

The implementation keeps ownership boring: Linear owns Project/KR truth;
SQLite owns one Project Session, shared child-command receipts, event ledgers,
and the observation outbox; the Wave server alone writes its journal. Project
processes coordinate from a stable Wave checkout but own no branch, worktree,
or PR. Task Sessions remain the only file-writing child.

The failure-path review changed the code in three places rather than just
producing notes:

- `project run` now reconciles a stale active process before deciding that an
  existing session is already running;
- Project decisions interrupt-and-resume non-steerable Claude/OpenCode turns,
  avoiding a blocked decision tool waiting on a turn that could never finish;
- provider interrupt/send failures settle the claimed Project command as a
  durable `failed` receipt and emit the matching typed event before the
  session fails;
- every shared-command query now includes its `target_kind`, so a `cc_…`
  receipt cannot be reinterpreted through the wrong Task or Project API.

The public interface still fits on one screen because Project controls mirror
the proven Task vocabulary; the generic machinery stays private. Status JSON
names process liveness, state reason, iteration, cursor, and pending
observations, so a stopped process or stuck delivery is visible without
reading tmux output. The main residual risk is evidence, not another state
owner: the complete ten-scenario scripted-peer matrix and live two-Task
Linear/PR dogfood remain explicit gates.
