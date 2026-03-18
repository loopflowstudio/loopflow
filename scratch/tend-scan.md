# Tend Scan — 2026-03-17

## Wave: chord-model
### Config
- flow: `ship-wave`
- mode: `manual`
- direction: `clarity`, `care`
- area: `rust/loopflow/src/lfd/`, `rust/loopflow/src/lfd/http/`, `rust/loopflow/src/engine/`, `python/loopflow/`, `rust/loopflow/src/engine/builtins/steps/`, `rust/loopflow/src/engine/builtins/flows/`

### Runtime
- Defined on disk at `wave/chord-model/`, but live registration in lfd could not be verified
- `lfq show chord-model --json` returned `LoopflowError: invalid token`
- No verified live `status`, `iteration`, `open_pr_count`, or `stack_count` from lfd
- `active_run` could not be read because `lfq show` did not return JSON

### Progress
- On-disk roadmap is still centered on algedonic signals, the first live tend proof, then VSM / Letta / mutation follow-ons
- Main has recent area commits including `cfd74283 redesign: chords are waves, tend flow, wave configs (#560)`, `ddb70e5e chord-model: algedonic signals — repair lineage, error classification, and escalation (#569)`, and `2573c2ac flow: rename fork/branch to and/or, add router support and silence paths (#565)`
- Active worktree `../loopflow.chord-model` is on branch `jack-heart.chord-model.20260317_2324`; working tree is clean; branch is `ahead 8, behind 1` vs `main` and aligned with `origin/jack-heart.chord-model.20260317_2324`
- Local commits beyond `main` include `408b7213 chord-model: design for algedonic signals live demo`, `c28782e1 chord-model: algedonic signals — LF_HOME, PR sync, retry limit, demo`, and `362a2e13 wave: ship algedonic signals, update chord-model roadmap`
- Current worktree scratch has been cleared back to `scratch/.gitkeep`; there is no active `scratch/questions.md`

### Items
| # | Title | Status |
|---|-------|--------|
| 01 | Algedonic Signals | in-flight |
| 02 | Tend Flow Steps | blocked |
| 02 | VSM Flow | queued |
| 03 | Wave Discovery and Root Chord | queued |
| 04 | Letta Integration | queued |
| 05 | Chord-Wave Area Model | queued |
| 06 | Wave Mutation | queued |
| 07 | DAG and Nested Chords | queued |
| 08 | API Expansion | queued |

### Blocks
- Live runtime is still unreadable from lfd because `lfq show` fails with `invalid token`
- No open PR exists for the active branch
- The branch still carries unlanded shared-infra changes under `rust/loopflow/src/lfd/*`, `rust/loopflow/src/ops/pr.rs`, `rust/loopflow/tests/support/mod.rs`, and demo tooling in `scripts/`

### Open PRs
- None for `jack-heart.chord-model.20260317_2324`
- Recent merged PR in this lane: `#569` — `chord-model: algedonic signals — repair lineage, error classification, and escalation`

## Wave: clear-the-deck
### Config
- flow: `ship-wave`
- mode: `manual`
- direction: `simplicity`
- area: `pyproject.toml`, `python/`, `rust/loopflow/src/bin/lfd.rs`, `rust/loopflow/src/engine/`, `rust/loopflow/src/lf/`, `rust/loopflow/src/lfd/config.rs`, `rust/loopflow/src/lfd/executor/`, `rust/loopflow/src/ops/`, `swift/Concerto/`, `swift/LoopflowCore/`

### Runtime
- Defined on disk at `wave/clear-the-deck/`, but live registration in lfd could not be verified
- `lfq show clear-the-deck --json` returned `LoopflowError: invalid token`
- No verified live `status`, `iteration`, `open_pr_count`, or `stack_count` from lfd
- `active_run` could not be read because `lfq show` did not return JSON

### Progress
- On-disk README now frames this wave as post-collapse cleanup only; numbered items on disk start at `03`
- Main has recent area commits including `89cc70fd lfd auth: collapse to local and studio modes (#566)` and `518fbdd3 clear-the-deck: collapse lfd deploy docs and harden pr copy (#571)`
- Active worktree `../loopflow.clear-the-deck` is on branch `jack-heart.clear-the-deck.20260317_1840`; working tree is clean; branch is `ahead 19, behind 5` vs `main` and `ahead 20, behind 19` vs `origin/jack-heart.clear-the-deck.20260317_1840`
- Diff vs `main` still carries older deployment / sandbox / doc work, including `deploy/*`, `docker/docker-compose.yml`, `docs/lfd.md`, `rust/loopflow/src/lfd/config.rs`, multiple Rust tests, and the old roadmap files `wave/clear-the-deck/01-deployment-collapse.md` and `02-sandbox-pause.md`
- No `scratch/` directory exists in the active worktree

### Items
| # | Title | Status |
|---|-------|--------|
| 03 | Rust Boundary Cleanup | queued |
| 04 | Daemon Surface Cleanup | in-flight |
| 05 | Client Surface Cleanup | queued |

### Blocks
- Live runtime is still unreadable from lfd because `lfq show` fails with `invalid token`
- No open PR exists for the active branch
- Active branch is diverged from origin (`ahead 20, behind 19`)
- Active diff still includes pre-collapse roadmap files `01` and `02` even though the on-disk roadmap in this repo starts at `03`

### Open PRs
- None for `jack-heart.clear-the-deck.20260317_1840`
- Recent merged PR in this lane: `#571` — `clear-the-deck: collapse lfd deploy docs and harden pr copy`

## Wave: agent-embedding
### Config
- flow: `ship-wave`
- mode: `manual`
- direction: `care`, `clarity`
- area: `swift/Concerto/`, `swift/LoopflowCore/`

### Runtime
- Defined on disk at `wave/agent-embedding/`, but live registration in lfd could not be verified
- `lfq show agent-embedding --json` returned `LoopflowError: invalid token`
- No verified live `status`, `iteration`, `open_pr_count`, or `stack_count` from lfd
- `active_run` could not be read because `lfq show` did not return JSON

### Progress
- On-disk README still sequences this wave as attention queue first, then terminal embedding, then portfolio / lifecycle / calibration / composition follow-ons
- Main already shipped `be3b761a attention queue: surface items that need human action (#563)` in this area
- Active worktree `../loopflow.agent-embedding` is on branch `jack-heart.agent-embedding.20260317_1347`; branch is `ahead 31, behind 0` vs `main` and `ahead 35, behind 31` vs `origin/jack-heart.agent-embedding.20260317_1347`
- Active worktree is dirty: `rust/loopflow/src/lfd/types/event.rs` and `rust/loopflow/tests/config_tests.rs` are modified locally
- Open PR `#567` — `agent-embedding: terminal sessions, attention kinds, wave workspace routing` — is `OPEN`, not draft, created `2026-03-18T00:13:25Z`, updated `2026-03-18T06:37:23Z`, with `mergeStateStatus: BLOCKED`
- Current PR head checks on `4b761c411e80ee68d6479f3171090194eca6561b`: `rust-test` failed, `python-test` succeeded, and `e2e-smoke`, `docker-smoke`, `swift-test`, and `concerto-ui-test` are still in progress
- PR / local diff spans both Swift UI files and shared backend surfaces under `python/loopflow/` and `rust/loopflow/src/lfd/*`; scratch artifacts include `scratch/agent-embedding-terminal-embedding.md` plus PR copy files

### Items
| # | Title | Status |
|---|-------|--------|
| 01 | Attention Queue Completion | in-flight |
| 02 | Terminal Embedding | in-flight |
| 03 | Portfolio View | queued |
| 04 | Wave Lifecycle UI | queued |
| 05 | Calibration View | queued |
| 06 | Window Composition | queued |
| 07 | Beat Synthesizer | queued |

### Blocks
- Live runtime is still unreadable from lfd because `lfq show` fails with `invalid token`
- PR `#567` is currently blocked by a failing `rust-test` check while several other checks are still running
- Active branch is diverged from origin and the worktree is dirty

### Open PRs
- `#567` — `agent-embedding: terminal sessions, attention kinds, wave workspace routing`
  - state: `OPEN`
  - draft: `false`
  - created: `2026-03-18T00:13:25Z`
  - updated: `2026-03-18T06:37:23Z`
  - merge state: `BLOCKED`
  - checks: `rust-test` = `FAILURE`; `python-test` = `SUCCESS`; `e2e-smoke`, `docker-smoke`, `swift-test`, `concerto-ui-test` = `IN_PROGRESS`
  - URL: <https://github.com/loopflowstudio/loopflow/pull/567>

## Wave: signals
### Config
- flow: `ship-wave`
- mode: `manual`
- direction: `clarity`, `simplicity`
- area: `rust/loopflow/src/lfd/`, `rust/loopflow/src/engine/`, `python/loopflow/`

### Runtime
- Defined on disk at `wave/signals/`, but live registration in lfd could not be verified
- `lfq show signals --json` returned `LoopflowError: invalid token`
- No verified live `status`, `iteration`, `open_pr_count`, or `stack_count` from lfd
- `active_run` could not be read because `lfq show` did not return JSON

### Progress
- On-disk README frames this wave as the parallel Phase 1 track next to `chord-model/02`
- Active worktree `../loopflow.signals` is on branch `jack.signals.20260316_1856`; working tree is clean; branch is `ahead 1, behind 5` vs `main` and `ahead 8` vs `origin/jack.signals.20260316_1856`
- Local branch has one visible commit beyond `main`: `7868b62e signals: block taxonomy design doc`
- Diff vs `main` is only `scratch/signals-block-taxonomy.md`
- Scratch artifact present: `scratch/signals-block-taxonomy.md`; there is no `scratch/questions.md`

### Items
| # | Title | Status |
|---|-------|--------|
| 01 | Block Taxonomy | in-flight |
| 02 | Self-Healing Cascade | queued |
| 03 | Stall Detection | queued |
| 04 | Quality Signals | queued |
| 05 | Signal Memory | queued |

### Blocks
- Live runtime is still unreadable from lfd because `lfq show` fails with `invalid token`
- No open PR exists for the active branch
- No code diff beyond the scratch design doc is present on the active branch
- Branch is behind `main` by 5 commits

### Open PRs
- None for `jack.signals.20260316_1856`

## Cross-Wave
- `lfq show <wave> --json` failed for all four member waves with `LoopflowError: invalid token`; no member wave returned live JSON runtime state
- Port `127.0.0.1:2486`, the default `lfq` target, is currently held by `com.docke`, not a visible `lfd` process from this worktree
- Area overlap on disk remains heavy:
  - `chord-model` and `signals` both own `rust/loopflow/src/lfd/`, `rust/loopflow/src/engine/`, and `python/loopflow/`
  - `clear-the-deck` overlaps both `chord-model` and `signals` across `python/`, `rust/loopflow/src/engine/`, and `rust/loopflow/src/lfd/*`
  - `clear-the-deck` and `agent-embedding` both include Swift surfaces
- Active diff overlap is also real:
  - `chord-model` and `agent-embedding` both modify `rust/loopflow/src/lfd/attention.rs`, `rust/loopflow/src/lfd/executor/helpers.rs`, multiple `rust/loopflow/src/lfd/http/*` routes, `rust/loopflow/src/lfd/queue.rs`, store files, and `rust/loopflow/tests/support/mod.rs`
  - `chord-model` and `clear-the-deck` both modify `rust/loopflow/src/ops/pr.rs` and `rust/loopflow/tests/support/mod.rs`
  - `clear-the-deck` and `agent-embedding` both modify `rust/loopflow/tests/support/mod.rs`
- Open PR `#567` from `agent-embedding` touches `python/loopflow/*`, `rust/loopflow/src/lfd/*`, and Swift UI files, so it crosses into `chord-model` and `clear-the-deck` territory in addition to `agent-embedding`'s own area
- The on-disk docs still encode dependency ordering:
  - `wave/redesign/README.md` and `wave/signals/README.md` place `signals/01` in parallel with `chord-model/02`
  - `wave/redesign/README.md` says `clear-the-deck` should stay quiet until the shared `lfd/` + `python/loopflow/` area settles
  - `wave/chord-model/04-letta-integration.md` explicitly says to finish the first live tend cycle before Letta work
  - `wave/agent-embedding/01-attention-queue-completion.md` and `05-calibration-view.md` still depend on tend / attention outputs that live in the chord / signals backend space

## Raw Signals
- Only `agent-embedding` currently has an open PR
- None of the inspected member-wave worktrees currently has an active `scratch/questions.md`
- `../loopflow.clear-the-deck` still carries old roadmap files `01-deployment-collapse.md` and `02-sandbox-pause.md` even though this repo's `wave/clear-the-deck/` directory starts at `03`
- `../loopflow.signals` is still a design-doc-only branch while `../loopflow.chord-model` and `../loopflow.agent-embedding` both carry substantial code / backend changes
