# Tend Scan — 2026-03-17

## Wave: chord-model

### Config
- **Flow:** ship-wave
- **Mode:** manual
- **Direction:** clarity, care
- **Area:** rust/loopflow/src/lfd/, rust/loopflow/src/engine/, python/loopflow/, rust/loopflow/src/engine/builtins/steps/, rust/loopflow/src/engine/builtins/flows/

### Runtime
**Not registered in lfd.** Defined on disk but `lfq show chord-model` returns "wave not found."

### Progress
One commit on main this week: `cfd74283 redesign: chords are waves, tend flow, wave configs (#560)` — this landed the structural tend wiring (flow YAML, scan-waves step, or-routing, CLI support, tests).

PR #565 (`jack-heart.chords.20260317_1049`) is open with all 7 CI checks passing. Title: "flow: rename fork/branch to and/or, wire or-routing in CLI." This is the current branch — the tend flow steps work from item 02.

### Items
| # | Title | Status |
|---|-------|--------|
| 02 | Tend Flow Steps | in-flight — structural wiring landed (#560), live lfd proof still pending |
| 03 | Letta Integration | queued — depends on 02 completing live proof |
| 04 | Chord-Wave Triggers | queued |
| 05 | Chord-Wave Area Model | queued — partial: scan-waves reads WaveDto already |
| 06 | Wave Mutation | queued |
| 07 | DAG and Default Chord-Wave | queued |

### Blocks
- **Live lfd proof gap.** Item 02's structural slice is done. The remaining work is operational: boot lfd, register redesign waves, run a real tend cycle. No code blocker — it's execution.
- **No lfd registration.** None of the redesign waves are registered. `lfq list` shows only `mobile-e2e-test` (failed) and `fork-test-8e7e1adf` (idle). `scripts/bootstrap-redesign.py` exists but hasn't been run against a live lfd.

### Open PRs
- **#565** — "flow: rename fork/branch to and/or, wire or-routing in CLI" — all CI green, no reviews, created 2026-03-17

---

## Wave: clear-the-deck

### Config
- **Flow:** ship-wave
- **Mode:** manual
- **Direction:** simplicity
- **Area:** rust/loopflow/src/lfd/auth.rs, rust/loopflow/src/lfd/provider_auth.rs, rust/loopflow/src/lfd/executor/sandbox.rs, rust/loopflow/src/lfd/http/, python/loopflow/

### Runtime
**Not registered in lfd.** Defined on disk only.

### Progress
No commits on main touching this wave's area in the past week beyond the shared `cfd74283` commit. No worktree or branch activity visible.

### Items
| # | Title | Status |
|---|-------|--------|
| 01 | Auth Consolidation | queued |
| 02 | Deployment Collapse | queued |
| 03 | Sandbox Pause and Daytona Evaluation | queued |
| 04 | Growth Infrastructure Cleanup | queued |

### Blocks
- **No activity.** This wave has not started. All four items are queued with no branches, PRs, or commits.
- **Area overlap with chord-model.** Both waves touch `rust/loopflow/src/lfd/` and `python/loopflow/`. Auth consolidation and provider-auth changes could conflict with engine work from chord-model.

### Open PRs
None.

---

## Wave: agent-embedding

### Config
- **Flow:** ship-wave
- **Mode:** manual
- **Direction:** care, clarity
- **Area:** swift/Concerto/, swift/LoopflowCore/

### Runtime
**Not registered in lfd.** Defined on disk only.

### Progress
PR #563 (`jack.agent-embedding.20260316_1856`) is open with all 7 CI checks passing. Title: "attention queue: lfd backend, concerto UI, pm wave, auth improvements." This covers block queue / attention queue work — maps to item 01 (block queue view). Created 2026-03-16.

No commits on main touching `swift/Concerto/` or `swift/LoopflowCore/` this past week.

### Items
| # | Title | Status |
|---|-------|--------|
| 01 | Block Queue View | in-flight — PR #563 covers attention queue backend + UI |
| 02 | Terminal Embedding | queued |
| 03 | Portfolio View | queued |
| 04 | Wave Lifecycle UI | queued |
| 05 | Calibration View | queued |
| 06 | Window Composition | queued |
| 07 | Beat Synthesizer | queued |

### Blocks
- **PR #563 scope is broader than item 01.** Title mentions "pm wave, auth improvements" alongside "attention queue" and "concerto UI." May indicate scope expansion or bundling of cross-wave work into one PR.
- **Depends on signals/01.** Block queue view depends on the block taxonomy from signals/01 to produce real block types. Without it, the queue has no typed content.

### Open PRs
- **#563** — "attention queue: lfd backend, concerto UI, pm wave, auth improvements" — all CI green, no reviews, created 2026-03-16

---

## Wave: signals

### Config
- **Flow:** ship-wave
- **Mode:** manual
- **Direction:** clarity, simplicity
- **Area:** rust/loopflow/src/lfd/, rust/loopflow/src/engine/, python/loopflow/

### Runtime
**Not registered in lfd.** Defined on disk only.

### Progress
No commits on main touching this wave's specific area in the past week beyond the shared `cfd74283` commit. No worktree or branch activity visible.

### Items
| # | Title | Status |
|---|-------|--------|
| 01 | Block Taxonomy | queued |
| 02 | Self-Healing Cascade | queued |
| 03 | Stall Detection | queued |
| 04 | Quality Signals | queued |
| 05 | Signal Memory | queued |

### Blocks
- **No activity.** This wave has not started. All items are queued.
- **Area overlap with chord-model.** Identical overlap: both touch `rust/loopflow/src/lfd/`, `rust/loopflow/src/engine/`, `python/loopflow/`.
- **signals/01 is a dependency for agent-embedding/01.** Block queue view needs block types defined here.

### Open PRs
None.

---

## Cross-Wave

### Area Overlaps
Three waves share significant area:
- **chord-model**, **signals**, and **clear-the-deck** all touch `rust/loopflow/src/lfd/` and `python/loopflow/`
- **chord-model** and **signals** additionally share `rust/loopflow/src/engine/`
- **agent-embedding** is isolated in `swift/` with no overlap

When these waves start running in parallel, file conflicts between chord-model, signals, and clear-the-deck are likely in `lfd/` and `python/loopflow/`.

### Dependencies
- **chord-model/02** (tend flow proof) → **redesign chord-wave** (can't tend for real until lfd registration + live cycle)
- **signals/01** (block taxonomy) → **agent-embedding/01** (block queue needs typed blocks)
- **chord-model/02** → **chord-model/03** (Letta needs real tend output to remember)

### Active Work Across Member Waves
Two PRs are in-flight from two different waves:
- PR #565 (chord-model: tend flow steps) — this branch
- PR #563 (agent-embedding: attention queue + block queue)

Both have all CI passing. Neither has been reviewed or merged.

### Non-Member PR Activity
Two additional open PRs (`#564`, `#561`) relate to a `pm` wave that is not a member of the redesign chord-wave. Both touch `python/loopflow/` which overlaps with chord-model, signals, and clear-the-deck areas.

## Raw Signals

- **Bootstrap incomplete.** No redesign waves are registered in lfd. `scripts/bootstrap-redesign.py` exists but hasn't been run. The chord-wave can't tend until its members exist in lfd.
- **Stale lfd state.** Only two old test waves exist in lfd (`mobile-e2e-test`, `fork-test-8e7e1adf`). Neither relates to redesign.
- **Two waves active, two dormant.** chord-model and agent-embedding have in-flight PRs. clear-the-deck and signals have zero activity.
- **Phase 1 sequencing.** The redesign README says Phase 1 is: chord-model/02 + signals/01 in parallel, then chord-model/03. Currently only chord-model/02 is in-flight. signals/01 has not started.
- **PR #563 scope.** The agent-embedding PR title suggests scope beyond its wave's area ("pm wave, auth improvements"). Worth checking whether it stayed within `swift/` or touched shared areas.
