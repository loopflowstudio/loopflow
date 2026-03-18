# Tend Scan — 2026-03-17

## Wave: chord-model
### Config
- flow: `ship-wave`
- mode: `manual`
- direction: `clarity`, `care`
- area: `rust/loopflow/src/lfd/`, `rust/loopflow/src/lfd/http/`, `rust/loopflow/src/engine/`, `python/loopflow/`, `rust/loopflow/src/engine/builtins/steps/`, `rust/loopflow/src/engine/builtins/flows/`

### Runtime
- `lfq show chord-model --json` returned `LoopflowError: invalid token`
- No verified live `status`, `iteration`, `open_pr_count`, or `stack_count` from lfd
- `active_run` could not be read because `lfq show` did not return JSON

### Progress
- Main shipped chord-related work on 2026-03-17, including `cfd74283 redesign: chords are waves, tend flow, wave configs (#560)`, `2573c2ac flow: rename fork/branch to and/or, add router support and silence paths (#565)`, and `ddb70e5e chord-model: algedonic signals — repair lineage, error classification, and escalation (#569)`
- Active worktree `../loopflow.chord-model` is on branch `jack.chord-model.20260316_1856` with five local commits beyond `main`, including `408b7213 chord-model: design for algedonic signals live demo` and `362a2e13 wave: ship algedonic signals, update chord-model roadmap`
- Active worktree is dirty: `rust/loopflow/src/engine/config.rs`, `rust/loopflow/src/ops/pr.rs`, and `rust/loopflow/tests/support/mod.rs` are modified locally
- Scratch artifacts present: `scratch/chord-model-algedonic-signals.md`, `scratch/questions.md`, plus carryover PR-copy/review files

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
- Live runtime could not be read from lfd because `lfq show` failed with `invalid token`
- No open PR on the active branch
- `scratch/chord-model-algedonic-signals.md` still lists live-demo infra gaps: LF_HOME/dev-token isolation, PR state sync for `check-ci`, and demo harness work
- Active worktree is dirty

### Open PRs
- None for `jack.chord-model.20260316_1856`
- Related worktree PRs already merged: `#565 flow: rename fork/branch to and/or, add router support and silence paths` and `#569 chord-model: algedonic signals — repair lineage, error classification, and escalation`

## Wave: clear-the-deck
### Config
- flow: `ship-wave`
- mode: `manual`
- direction: `simplicity`
- area: `pyproject.toml`, `python/`, `rust/loopflow/src/bin/lfd.rs`, `rust/loopflow/src/engine/`, `rust/loopflow/src/lf/`, `rust/loopflow/src/lfd/config.rs`, `rust/loopflow/src/lfd/executor/`, `rust/loopflow/src/ops/`, `swift/Concerto/`, `swift/LoopflowCore/`

### Runtime
- `lfq show clear-the-deck --json` returned `LoopflowError: invalid token`
- No verified live `status`, `iteration`, `open_pr_count`, or `stack_count` from lfd
- `active_run` could not be read because `lfq show` did not return JSON

### Progress
- Main shipped clear-the-deck work on 2026-03-17, including `89cc70fd lfd auth: collapse to local and studio modes (#566)` and `518fbdd3 clear-the-deck: collapse lfd deploy docs and harden pr copy (#571)`
- Active worktree `../loopflow.clear-the-deck` is on branch `jack-heart.clear-the-deck.20260317_1840`
- Branch is diverged from `origin/jack-heart.clear-the-deck.20260317_1840` (`ahead 20, behind 19`)
- Diff vs `main` still carries older roadmap/config/doc changes, including `deploy/*`, `docker/docker-compose.yml`, `docs/lfd.md`, `rust/loopflow/src/lfd/config.rs`, `rust/loopflow/tests/*`, and old roadmap files `wave/clear-the-deck/01-deployment-collapse.md` and `02-sandbox-pause.md`
- No `scratch/` directory in the active worktree

### Items
| # | Title | Status |
|---|-------|--------|
| 03 | Rust Boundary Cleanup | queued |
| 04 | Daemon Surface Cleanup | in-flight |
| 05 | Client Surface Cleanup | queued |

### Blocks
- Live runtime could not be read from lfd because `lfq show` failed with `invalid token`
- No open PR on the active branch
- Active branch is both ahead of and behind origin
- Active diff still references wave items `01` and `02`, while the on-disk roadmap in this worktree starts at `03`

### Open PRs
- None for `jack-heart.clear-the-deck.20260317_1840`
- Related worktree PR already merged: `#566 lfd auth: collapse to local and studio modes`

## Wave: agent-embedding
### Config
- flow: `ship-wave`
- mode: `manual`
- direction: `care`, `clarity`
- area: `swift/Concerto/`, `swift/LoopflowCore/`

### Runtime
- `lfq show agent-embedding --json` returned `LoopflowError: invalid token`
- No verified live `status`, `iteration`, `open_pr_count`, or `stack_count` from lfd
- `active_run` could not be read because `lfq show` did not return JSON

### Progress
- Main shipped `be3b761a attention queue: surface items that need human action (#563)` on 2026-03-17
- Active worktree `../loopflow.agent-embedding` is on branch `jack-heart.agent-embedding.20260317_1347`
- Open PR `#567 agent-embedding: terminal sessions, attention kinds, wave workspace routing` was created at `2026-03-18T00:13:25Z`; all listed CI checks are green, and GitHub reports `mergeStateStatus: DIRTY`
- Active branch is ahead of origin by 1 commit and has local modifications in `rust/loopflow/src/lfd/http/routes/terminal_sessions.rs`, `rust/loopflow/src/lfd/types/terminal_session.rs`, `swift/LoopflowCore/Models/AttentionItem.swift`, and `swift/LoopflowCore/Services/LocalWaveService.swift`
- Scratch artifacts present: `scratch/jack-heart.agent-embedding.20260317_1347.md`, `scratch/jack-heart.agent-embedding.20260317_1347-review.md`, `scratch/agent-embedding-terminal-embedding.md`, PR copy/title/body files

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
- Live runtime could not be read from lfd because `lfq show` failed with `invalid token`
- PR `#567` is green in CI but marked `DIRTY` by GitHub
- Active branch is ahead of the pushed PR by 1 commit and the worktree is dirty

### Open PRs
- `#567` — `agent-embedding: terminal sessions, attention kinds, wave workspace routing`
  - state: `OPEN`
  - draft: `false`
  - created: `2026-03-18T00:13:25Z`
  - updated: `2026-03-18T04:59:43Z`
  - CI: `rust-test`, `python-test`, `e2e-smoke`, `docker-smoke`, `sandbox-smoke`, `swift-test`, `concerto-ui-test` all `SUCCESS`
  - merge state: `DIRTY`
  - URL: <https://github.com/loopflowstudio/loopflow/pull/567>

## Wave: signals
### Config
- flow: `ship-wave`
- mode: `manual`
- direction: `clarity`, `simplicity`
- area: `rust/loopflow/src/lfd/`, `rust/loopflow/src/engine/`, `python/loopflow/`

### Runtime
- `lfq show signals --json` returned `LoopflowError: invalid token`
- No verified live `status`, `iteration`, `open_pr_count`, or `stack_count` from lfd
- `active_run` could not be read because `lfq show` did not return JSON

### Progress
- Active worktree `../loopflow.signals` is on branch `jack.signals.20260316_1856`
- Local branch has one visible design/doc commit beyond `main`: `7868b62e signals: block taxonomy design doc`
- Diff vs `main` is only `scratch/signals-block-taxonomy.md`
- Scratch artifact present: `scratch/signals-block-taxonomy.md`

### Items
| # | Title | Status |
|---|-------|--------|
| 01 | Block Taxonomy | in-flight |
| 02 | Self-Healing Cascade | queued |
| 03 | Stall Detection | queued |
| 04 | Quality Signals | queued |
| 05 | Signal Memory | queued |

### Blocks
- Live runtime could not be read from lfd because `lfq show` failed with `invalid token`
- No open PR on the active branch
- No code diff beyond the scratch design doc on the active branch

### Open PRs
- None for `jack.signals.20260316_1856`

## Cross-Wave
- `lfq show <wave> --json` failed for all four member waves with `LoopflowError: invalid token`; no member wave returned live JSON runtime state
- Port `127.0.0.1:2486`, the default `lfq` target, is currently held by `com.docke`; live lfd state was not available from this worktree during the scan
- Area overlap is heavy:
  - `chord-model` and `signals` both own `rust/loopflow/src/lfd/`, `rust/loopflow/src/engine/`, and `python/loopflow/`
  - `clear-the-deck` overlaps both `chord-model` and `signals` across `python/`, `rust/loopflow/src/engine/`, and `rust/loopflow/src/lfd/*`
  - `clear-the-deck` and `agent-embedding` both touch `swift/Concerto/` and `swift/LoopflowCore/`
- Open PR `#567` from `agent-embedding` touches `rust/loopflow/src/lfd/http/*`, `rust/loopflow/src/lfd/queue.rs`, `rust/loopflow/src/lfd/attention.rs`, and other files that sit inside `chord-model`'s area, as well as Swift files in `agent-embedding`'s own area
- Roadmap files reference each other directly:
  - `wave/chord-model/02-tend-flow-steps.md` calls out `signals/01` as the parallel Phase 1 track
  - `wave/signals/README.md` frames `signals/01` as the parallel track next to `chord-model/02`
  - `wave/chord-model/04-letta-integration.md` says to finish live tend cycles first
  - `wave/agent-embedding/01-attention-queue-completion.md` and `05-calibration-view.md` depend on tend/attention outputs that sit in the chord/signals space

## Raw Signals
- Only `agent-embedding` currently has an open PR; `chord-model` and `clear-the-deck` have related merged PRs from 2026-03-17, while `signals` is still at scratch-doc stage
- `chord-model`, `agent-embedding`, and `signals` all have active scratch artifacts in their sibling worktrees; `clear-the-deck` does not
- `../loopflow.chord-model/scratch/questions.md` says `No open questions.`
- `../loopflow.clear-the-deck` appears to be carrying an older roadmap generation than the `wave/clear-the-deck/` directory in this worktree
- Several member-wave worktrees are dirty or ahead of origin, so local worktree state is ahead of what GitHub currently reflects
