# Tend Scan — 2026-03-18

## Wave: chord-model

### Config
- **Flow:** ship-wave
- **Mode:** manual
- **Direction:** clarity, care
- **Area:** rust/loopflow/src/lfd/, rust/loopflow/src/lfd/http/, rust/loopflow/src/engine/, python/loopflow/, rust/loopflow/src/engine/builtins/steps/, rust/loopflow/src/engine/builtins/flows/
- **PM:** Asana project 1213717740994598

### Runtime
- **lfd:** not accessible (invalid token — lfd may not be running)
- **Worktree:** `/Users/jack/src/loopflow.chord-model` on branch `jack-heart.chord-model.20260318_0020`
- **Additional worktrees:** `loopflow.chord-model.20260316_1856` (stale, on `jack.chord-model.20260316_1856`), `loopflow.chord-model.20260318_0010` (on `jack-heart.chord-model.20260318_0010`)

### Progress

**Shipped to main this week:**
- `aa4dfeb2` — lfd: ship algedonic signals with repair backoff (#578)
- `bacc2f75` — redesign: fold signals into chord-model and clear scratch on land (#580)
- `f4619676` — ci: enforce clear scratch before landing (#579)
- `a4ff980b` — review: split into demo and code-review, reframe review-design (#575)
- `2573c2ac` — flow: rename fork/branch to and/or, add router support and silence paths (#565)
- `ddb70e5e` — chord-model: algedonic signals — repair lineage, error classification, and escalation (#569)
- `89cc70fd` — lfd auth: collapse to local and studio modes (#566)
- `cfd74283` — redesign: chords are waves, tend flow, wave configs (#560)

**In flight:**
- Active worktree has 2 files changed (43 insertions) vs main: `rust/loopflow/src/lf/commands/ops/mod.rs`, `rust/loopflow/tests/worktree_tests.rs`

### Items

| # | Title | Status |
|---|-------|--------|
| 02 | Tend Flow Steps | in-flight (tend flow + wave configs shipped via cfd74283, signals folded via bacc2f75) |
| 02 | VSM Flow | queued (depends on tend proving out first) |
| 03 | Wave Discovery and Root Chord | queued |
| 04 | Letta Integration | queued |
| 05 | Chord-Wave Area Model | queued |
| 06 | Wave Mutation | queued |
| 07 | DAG and Nested Chords | queued |
| 08 | API Expansion | queued |

### Blocks
- **Merge conflict in worktree.** `loopflow.chord-model` has unresolved merge conflicts in 2 files (`UU` status): `rust/loopflow/src/lf/commands/ops/mod.rs` and `rust/loopflow/tests/worktree_tests.rs`. The branch cannot proceed until these are resolved.
- **Stale worktrees.** Two older chord-model worktrees (`20260316_1856`, `20260318_0010`) exist alongside the current one. Not blocking but adds clutter.

### Open PRs
None from chord-model branches.

---

## Wave: agent-embedding

### Config
- **Flow:** ship-wave
- **Mode:** manual
- **Direction:** care, clarity
- **Area:** swift/Concerto/, swift/LoopflowCore/
- **PM:** Asana project 1213718081058007

### Runtime
- **lfd:** not accessible (same token issue)
- **Worktree:** `/Users/jack/src/loopflow.agent-embedding` on branch `jack-heart.agent-embedding.20260317_1347`
- **Additional worktrees:** two run worktrees (`run-82e6075a`, `run-e96079a0`)

### Progress

**Shipped to main this week:**
- `d8bd7ac2` — concerto: connections panel redesign + Doppler secrets provider (#568)
- `89cc70fd` — lfd auth: collapse to local and studio modes (#566)
- `be3b761a` — attention queue: surface items that need human action (#563)
- `1c8db1e1` — concerto: detect local provider auth, eager daemon startup (#559)

**In flight:**
- Massive diff vs main: 80 files changed, +4358/-793 lines
- PR #567 open: "concerto: add workspace-first wave detail and tracked terminal runs"
- Branch includes: terminal session model, workspace views, wave detail redesign, daemon-owned PTY transport design, attention store updates, portfolio view updates
- Scratch artifacts present: `agent-embedding-terminal-embedding.md`, `pr-body.md`, `pr-title.txt`
- Uncommitted changes in worktree: 20 modified files spanning Rust lfd code, Swift UI, wave items, and a new `wave/lfd/` directory

### Items

| # | Title | Status |
|---|-------|--------|
| 01 | Attention Queue Completion | shipped (be3b761a) + expanded in PR #567 |
| 02 | Terminal Embedding | in-flight (PR #567 — workspace-first terminal + tracked runs) |
| 03 | Portfolio View | queued (item updated in PR #567 branch) |
| 04 | Wave Lifecycle UI | queued (item updated in PR #567 branch) |
| 05 | Calibration View | queued |
| 06 | Window Composition | queued (item updated in PR #567 branch) |
| 07 | Beat Synthesizer | queued |

Note: item 02 was renamed from "Terminal Embedding" to "Daemon-Owned PTY Transport" on the branch, reflecting a scope shift from embedded Ghostty to daemon-tracked terminal sessions.

### Blocks
- **CI failing on PR #567.** Two failures: `scratch-clear` (scratch artifacts in PR) and `rust-test` (Rust compilation or test failure). `tests-result` also fails as a downstream gate. Python, e2e, Swift, Docker, and Concerto UI tests all pass.
- **Large uncommitted working tree.** 20 modified files not yet committed in the agent-embedding worktree, including Rust lfd changes that overlap with chord-model's area.
- **Scope creep.** PR #567 touches `rust/loopflow/src/lfd/` extensively (terminal sessions, store migrations, HTTP routes, triggers, executor) — this is chord-model's area, not agent-embedding's. Also introduces `wave/lfd/` directory, which suggests a new wave being bootstrapped outside the redesign chord's area.

### Open PRs

| # | Title | CI | Age |
|---|-------|----|-----|
| 567 | concerto: add workspace-first wave detail and tracked terminal runs | FAILING (scratch-clear, rust-test) | 15 hours |

---

## Cross-Wave

### Area overlap
PR #567 (agent-embedding) makes substantial changes to `rust/loopflow/src/lfd/` — chord-model's primary area. Files touched include:
- Store layer: migrations, sqlite, postgres, mod.rs
- HTTP routes: waves, terminal_sessions (new), attention
- Executor: wave/mod.rs, helpers
- Types: terminal_session (new), event, attention, mod.rs
- Triggers: activation, loop_ticker
- Binary: lfd.rs

This is the biggest cross-wave signal. The agent-embedding wave is building terminal session infrastructure that lives in lfd — chord-model territory. The two waves will need to coordinate on lfd store/API changes.

### New wave bootstrapping
The agent-embedding worktree has an uncommitted `wave/lfd/` directory. This suggests a new wave being carved out for lfd-specific work, which could be a natural response to the area overlap — but it's happening outside the chord-wave's area and isn't yet registered.

### Dependency ordering
- chord-model's merge conflict blocks its worktree from making progress
- agent-embedding's CI failures block PR #567 from merging
- If #567 merges first, chord-model will need to rebase and integrate the lfd changes
- If chord-model lands lfd-touching changes first, agent-embedding's branch will need a rebase

Neither wave is currently able to land work.

---

## Raw Signals

- **lfd unreachable.** All `lfq show` calls fail with "invalid token." Runtime wave state cannot be observed. This scan is based entirely on git and GitHub state.
- **Three chord-model worktrees.** Suggests iteration/restarts on the current work item without cleanup.
- **Redesign scratch is clean.** The `scratch/` directory on main has only `.gitkeep` — `lf ops land` has been clearing scratch properly since #579 enforced it.
- **Wave item renumbering on branch.** agent-embedding/02 was replaced with a new `02-daemon-owned-pty-transport.md` on the branch, changing the item's identity. The original `02-terminal-embedding.md` was deleted.
- **PM integration active.** Both waves have Asana project IDs. The pm wave shipped Linear support this week (a9390e41).
- **Signals wave folded.** `bacc2f75` explicitly folded the signals wave into chord-model per the redesign README's phasing plan. The `loopflow.signals` worktree still exists but is on the same commit as main.
