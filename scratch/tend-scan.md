# Tend Scan — 2026-03-17

## Wave: chord-model
### Config
- flow: ship-wave
- mode: manual
- direction: clarity, care
- area: rust/loopflow/src/lfd/, rust/loopflow/src/engine/, python/loopflow/, rust/loopflow/src/engine/builtins/steps/, rust/loopflow/src/engine/builtins/flows/

### Progress
**Shipped recently (on main via PR #560):**
- Chord CRUD removed — chords are waves, no separate tables/DTOs/routes
- Tend flow YAML defined (tend.yaml, tend-chord.yaml)
- All five tend step prompts written (scan-waves, assess, draft-chord, review-chord, apply-chord)
- ship-roadmap flow with or construct and ops items
- reorg flow (single update-wave step)
- Wave YAML configs for all four member waves + redesign chord-wave
- Bootstrap script for wave registration
- Migration 028: drop chords tables

**In flight (branch jack-heart.chords.20260317_1049):**
- Design doc for item 02 (tend flow steps) completed in scratch/
- Item 02-tend-flow-steps.md being retired (moved to scratch design doc)
- Minor land.rs change in progress

### Items
| # | Title | Status |
|---|-------|--------|
| 01 | Chord Triggers | queued |
| 02 | Tend Flow Steps | in-flight — design complete, implementation next |
| 03 | Letta Integration | queued |
| 05 | Chord-Wave Area Model | queued |
| 06 | Wave Mutation | queued |
| 07 | DAG and Default Chord | queued |

### Runtime
No lfd wave registered yet. Wave defined on disk but not running — manual mode, bootstrap phase.

### Blocks
- **Tend flow not yet exercised end-to-end.** Steps and YAML exist but no cycle has run. Item 02 addresses this directly — design doc is complete, implementation is next.
- **scan-waves doesn't read lfd state.** The prompt describes reading git/gh but not `lfq show --json`. Design doc for 02 specifies the fix.

### Open PRs
- PR #559 (jack-heart.ux.20260305_2101): "concerto: detect local provider auth, eager daemon startup" — all CI green, 10 days old. Not chord-model area but touches lfd.

---

## Wave: clear-the-deck
### Config
- flow: ship-wave
- mode: manual
- direction: simplicity
- area: rust/loopflow/src/lfd/auth.rs, rust/loopflow/src/lfd/provider_auth.rs, rust/loopflow/src/lfd/executor/sandbox.rs, rust/loopflow/src/lfd/http/, python/loopflow/

### Progress
**Shipped recently:**
- No commits on main in wave area in last week specific to clear-the-deck items.
- PR #560 removed chord CRUD from lfd HTTP routes — a clear-the-deck-adjacent deletion (434 lines removed from routes/chords.rs, store code simplified).

**In flight:**
- No active branch or PR.

### Items
| # | Title | Status |
|---|-------|--------|
| 01 | Auth Consolidation | queued |
| 02 | Deployment Collapse | queued |
| 03 | Sandbox Pause / Daytona Eval | queued |
| 04 | Growth Infrastructure Cleanup | queued |

### Runtime
No lfd wave registered. Manual mode, not started.

### Blocks
- **Not started.** No items in flight. All four items are independent and could begin anytime.
- **Auth consolidation (01) depends on understanding current auth surface.** PR #559 touches provider auth — may want to land that first.

### Open PRs
None specific to this wave.

---

## Wave: agent-embedding
### Config
- flow: ship-wave
- mode: manual
- direction: care, clarity
- area: swift/Concerto/, swift/LoopflowCore/

### Progress
**Shipped recently:**
- No commits on main in wave area in last week specific to agent-embedding items.
- PR #556 (concerto: fix font loading and extract release script) landed recently — infrastructure work.

**In flight:**
- PR #559 (concerto: detect local provider auth, eager daemon startup) — all CI green, 10 days old. Touches Concerto provider auth detection and daemon startup, which is foundation work for the conductor pivot.

### Items
| # | Title | Status |
|---|-------|--------|
| 01 | Block Queue View | queued |
| 02 | Terminal Embedding | queued |
| 03 | Portfolio View | queued |
| 04 | Wave Lifecycle UI | queued |
| 05 | Calibration View | queued |
| 06 | Window Composition | queued |
| 07 | Beat Synthesizer | queued |

### Runtime
No lfd wave registered. Manual mode, not started.

### Blocks
- **Block queue (01) depends on signals/01 (block taxonomy).** The block queue view needs blocks to display — chicken-and-egg with signals wave.
- **Terminal embedding (02) has research risk.** Ghostty embedding in SwiftUI is unproven.
- **Stale PR.** PR #559 is 10 days old with all CI green — should be landed or closed.

### Open PRs
- PR #559: "concerto: detect local provider auth, eager daemon startup" — all CI green, created 2026-03-07.

---

## Wave: signals
### Config
- flow: ship-wave
- mode: manual
- direction: clarity, simplicity
- area: rust/loopflow/src/lfd/, rust/loopflow/src/engine/, python/loopflow/

### Progress
**Shipped recently:**
- No commits on main specific to signals items.

**In flight:**
- No active branch or PR.

### Items
| # | Title | Status |
|---|-------|--------|
| 01 | Block Taxonomy | queued |
| 02 | Self-Healing Cascade | queued |
| 03 | Stall Detection | queued |
| 04 | Quality Signals | queued |
| 05 | Signal Memory | queued |

### Runtime
No lfd wave registered. Manual mode, not started.

### Blocks
- **Not started.** All items queued.
- **Block taxonomy (01) is a dependency for agent-embedding/01 (block queue view).** The queue needs something to display.

### Open PRs
None.

---

## Cross-Wave

### Area overlap
- **chord-model and signals share area paths:** Both list `rust/loopflow/src/lfd/`, `rust/loopflow/src/engine/`, `python/loopflow/`. File conflicts are likely when both waves are active — same Rust modules, same Python package.
- **clear-the-deck overlaps with chord-model and signals:** `rust/loopflow/src/lfd/http/` and `python/loopflow/` appear in clear-the-deck's area and are subsets of chord-model/signals areas.

### Dependencies
- **chord-model/02 → first tend cycle.** The tend flow must work before the chord-wave can do its job. This is the critical path for the entire redesign.
- **signals/01 → agent-embedding/01.** Block taxonomy must exist before the block queue view has anything to display.
- **chord-model/03 (Letta) → signals/05 (signal memory).** Signal memory stores patterns in Letta — Letta must be running first.
- **chord-model/05 (area model) → chord-model/06 (mutation).** The mutation API operates on the data the area model exposes.

### Trigger relationships
None configured yet. All waves are manual. Triggers (chord-model/01) are a later item.

### PR state
One open PR (#559) across all waves. It touches Concerto (agent-embedding area) and lfd provider auth. CI is green. 10 days old.

---

## Raw Signals

- **Single commit on main in the last week** (cfd74283, PR #560). This was the bootstrap commit — large, foundational, shipped all wave configs and tend step prompts. Activity is concentrated, not distributed.
- **The current branch has 12 commits** building on the bootstrap. Design doc for chord-model/02 is complete. The branch also retires the shipped item file (02-tend-flow-steps.md deleted from wave/).
- **No waves are registered in lfd.** All four member waves exist on disk only. The bootstrap script exists but hasn't been run against a live lfd instance.
- **Phase 1 ordering is clear:** chord-model/02 (tend flow steps) → signals/01 (block taxonomy) → chord-model/03 (Letta). The design doc for 02 is done. Implementation is next.
- **clear-the-deck is fully independent.** Four items, no dependencies on other waves. Could start anytime to generate early momentum and reduce surface area.
- **The chord CRUD removal in PR #560 was significant** — 434 lines of routes, 200+ lines of store code, migration to drop tables. This was effectively a clear-the-deck action done as part of bootstrap.
