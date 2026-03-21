# Tend Scan — 2026-03-20

## Wave: chord-model

### Config
- **Flow**: build-or-silent
- **Mode**: manual
- **Direction**: clarity, care
- **Area**: rust/loopflow/src/lfd/, rust/loopflow/src/engine/, python/loopflow/, rust/loopflow/src/engine/builtins/steps/, rust/loopflow/src/engine/builtins/flows/

### Runtime
- **lfd**: not queryable (lfq returns "invalid token" — lfd may not be running or auth is stale)

### Progress

**Shipped this week (landed to main):**
- #593: worker capacity, xor/loop engine, ops→op rename (merged 2026-03-21)
- #584: rename tend→garden, add VSM governance flows, xor/or/loop engine (merged 2026-03-18)
- #569: algedonic signals — repair lineage, error classification, and escalation (merged 2026-03-18)

14 commits touched chord-model area paths on main in the past week. Heavy activity across the engine, lfd, and builtins.

**In flight:**
- Active worktree at `/Users/jack/src/loopflow.chord-model` on branch `jack-heart.chord-model.20260320_2226` — clean working tree, no commits ahead of main (fresh branch).

### Items

| # | Title | Status |
|---|-------|--------|
| 02b | Wave Modes (`flow` replacing `manual`) | queued — phase 0, not yet started |
| 02c | Planning Flow and Chord Governance | queued — VSM governance flows shipped, planning tree traversal remains |
| 02d | Concurrent Ingest | queued — PM providers ready, atomic arbitration not built |
| 02e | Wave Crons | queued — `crons` config field not yet implemented |
| 4-tend-flow-steps | Tend Flow Steps | in-flight — tend/scan and tend/assess steps exist as builtins, live `lf tend` cycle needs validation |
| 4-vsm-flow | VSM Flow | in-flight — four governance flows shipped, single-pass `lf vsm` command needs wiring |
| 4-chord-wave-area-model | Chord-Wave Area Model | queued |
| 4-wave-discovery-and-root-chord | Wave Discovery and Root Chord | queued — phase 3 |
| 4-wave-mutation | Wave Mutation | queued |
| 4-dag-and-nested-chords | DAG and Nested Chords | queued |
| 4-letta-integration | Letta Integration | queued — blocked on live garden output |
| 4-api-expansion | API Expansion | queued |

### Blocks
- No CI failures on chord-model branches (most recent PR #593 merged clean).
- No open questions in scratch/.
- No merge conflicts detected.

### Open PRs
None from chord-model.

### Unlanded Branches
| Branch | Commits ahead | Diff stats | PR |
|--------|---------------|------------|----|
| origin/jack-heart.chord-model.20260320_2137 | 5 | 277 files, +15399/-4737 | none (stale — appears to be the pre-merge state of #593) |
| origin/jack-heart.chord-model.20260319_1840 | 5 | 293 files, +19475/-5243 | none (stale) |
| origin/jack-heart.chord-model.20260318_1226 | 5 | 194 files, +9806/-2835 | #584 merged |
| origin/jack-heart.chord-model.20260318_0010 | 2 | stale land artifacts | none |
| origin/jack-heart.chord-model.20260317_2324 | 5 | 75 files, +1181/-3541 | none (stale) |

All older branches appear to be remnants of landed PRs or abandoned iterations. No significant unlanded work detected.

---

## Wave: agent-embedding

### Config
- **Flow**: build-or-silent
- **Mode**: manual
- **Direction**: care, clarity
- **Area**: swift/Concerto/, swift/LoopflowCore/

### Runtime
- **lfd**: not queryable (same auth issue)

### Progress

**Shipped this week (landed to main):**
- #594: interactive checkpoints, wave workspaces, pm bootstrap, and garden rename (merged 2026-03-21)
- #592: wave workspaces with terminal-native keyboard routing (merged 2026-03-20)
- #588: terminal multiplexer, attention lifecycle, flow engine extensions (merged 2026-03-20)
- #587: terminal sessions, attention lifecycle, wave workspace UI (merged 2026-03-19)

4 commits touched agent-embedding area paths (swift/) on main in the past week.

**In flight:**
- Active worktree at `/Users/jack/src/loopflow.agent-embedding` on branch `jack-heart.agent-embedding.20260320_2148`.
- 1 new commit since last merged PR: `kickoff: 04-window-composition design doc`
- Working tree has a deleted file: `wave/agent-embedding/04-window-composition.md` (the item was consumed into a scratch design doc)

### Items

| # | Title | Status |
|---|-------|--------|
| 01 | Wave Lifecycle UI — Remaining | queued — worktree management and step-level run history gaps |
| 02 | Portfolio View | queued — multi-repo, multi-wave conductor dashboard |
| 03 | Calibration View | queued — garden flow's human checkpoint UX |
| 04 | Window Composition — Polish | in-flight — kickoff design doc written on current branch |

### Blocks
- No CI failures on agent-embedding branches.
- No open questions in scratch/.
- No merge conflicts.

### Open PRs
None from agent-embedding.

### Unlanded Branches
| Branch | Commits ahead | Diff stats | PR |
|--------|---------------|------------|----|
| origin/jack-heart.agent-embedding.20260320_2148 | 5+ | 245 files, +18930/-4753 | none (active worktree, includes stale pre-merge state + 1 new commit) |
| origin/jack-heart.agent-embedding.20260320_1445 | 5 | 245 files, +18930/-4753 | #594 merged |
| origin/jack-heart.agent-embedding.20260318_1627 | 6 | 220 files, +13463/-4220 | #588 merged |
| origin/jack-heart.agent-embedding.20260318_1241 | 5 | 86 files, +5471/-1098 | #587 merged (via earlier branch) |
| origin/jack-heart.agent-embedding.20260317_1347 | 5 | 81 files, +4572/-794 | stale |

Active work is on `20260320_2148` — one kickoff commit for window composition. All other branches are remnants of landed PRs.

---

## Cross-Wave

### Shared activity
- Both waves shipped work the same day (2026-03-21) — #593 (chord-model) and #594 (agent-embedding) merged within hours.
- The agent-embedding PRs (#587, #588, #592, #594) included engine/flow changes that also live in chord-model's area (builtins/steps/, builtins/flows/). This is expected — Concerto features often require new steps or flow extensions.
- The `lfd` wave (PR #596, open) touches `rust/loopflow/src/lfd/` which overlaps with chord-model's area. This PR has CI failures (scratch-clear and concerto-ui-test).

### Dependencies
- **agent-embedding → chord-model**: Portfolio view (02) and calibration view (03) depend on chord-wave area model and governance flows being wired. These chord-model items are queued.
- **chord-model → agent-embedding**: Letta integration needs live garden output, which needs the tend/garden cycle running end-to-end. The tend steps exist but haven't been validated live.
- **lfd PR #596 blocks both**: The runtime journal work in lfd changes how flows execute, which affects both waves' area. It needs CI fixed before landing.

### PM wave adjacency
- PR #589 (pm: full pull/export sync) is open with CI failures (scratch-clear). PM sync work feeds into both waves' item management.

---

## Raw Signals

- **lfd is unreachable**: All three `lfq show` calls failed with "invalid token." Either lfd is not running or the auth token expired. This means no runtime state is available for any wave — no iteration counts, no active run info, no queue state.
- **Branch proliferation**: 5 remote chord-model branches and 5 remote agent-embedding branches exist, all remnants of landed PRs or stale iterations. 30+ worktrees on disk total. No cleanup has run recently.
- **Velocity is high**: 17 PRs landed to main in the past week across all waves. Both member waves are shipping multiple PRs per day.
- **Both open PRs fail scratch-clear**: PRs #596 and #589 both fail the `scratch-clear` CI check. This suggests scratch artifacts are being committed to PR branches.
- **Current branch (redesign)**: `jack-heart.redesign.20260320_2227` has 10 commits ahead of main — these are the recent merged PRs (the branch was created from a recent main state that includes them). This is the chord's own branch for tend/garden work.
- **04-window-composition kickoff**: The agent-embedding worktree has begun item 04 with a design doc. The wave item file was deleted from the working tree (consumed into scratch), suggesting the kickoff step ran and produced `scratch/04-window-composition.md`.
