# Tend Assessment — 2026-03-18

## Summary

Both waves shipped substantial work this week but are now simultaneously blocked — chord-model by merge conflicts, agent-embedding by a failing oversized PR that has drifted deep into chord-model's area. The dominant tension is area violation: agent-embedding built terminal session infrastructure across lfd's store, HTTP, and executor layers, which are chord-model's territory. Until the cross-wave boundary is resolved, neither wave can land work and rebasing will only get harder.

## Wave: chord-model

**Health**: blocked

**Evidence**: Eight commits landed on main this week — the highest output of either wave. Work spans engine fundamentals (and/or flow control, algedonic signals, repair backoff), tooling (scratch-clear enforcement, review step split), and organizational consolidation (signals wave folded in). But the active worktree has unresolved merge conflicts in `ops/mod.rs` and `worktree_tests.rs`, and three worktrees exist for what should be one stream of work. No open PR. The in-flight item (Tend Flow Steps, #02) is partially shipped — tend flow and wave configs landed, signals folded — but the item isn't crossed yet.

**Pressure**: The merge conflict is mechanical and solvable in minutes, but the real pressure is sequencing against agent-embedding's lfd changes. If PR #567 lands first, chord-model inherits a large lfd surface change to integrate. If chord-model resolves conflicts and ships first, #567 needs a rebase. Neither is moving, so the dependency is frozen.

## Wave: agent-embedding

**Health**: drifting

**Evidence**: Four commits shipped to main. PR #567 is open but failing CI (scratch-clear and rust-test). The branch is massive: 80 files changed, +4358/-793 lines — well beyond the ~1000 LOC target. Twenty more files sit uncommitted. Item 02 was rescoped mid-flight from "Terminal Embedding" (embedded Ghostty) to "Daemon-Owned PTY Transport" (daemon-tracked sessions) — a legitimate pivot, but the scope expanded rather than narrowed. Most critically, the branch makes extensive changes to `rust/loopflow/src/lfd/` — store migrations, HTTP routes, executor, types, triggers — which is chord-model's declared area, not agent-embedding's (`swift/Concerto/`, `swift/LoopflowCore/`). An uncommitted `wave/lfd/` directory suggests awareness of the boundary problem but no resolution.

**Pressure**: The PR is too large to review and too entangled to land cleanly. The lfd changes need to either move to chord-model or be carved into a separate PR that ships first. The scratch-clear CI failure is trivial (remove scratch artifacts before merging), but the rust-test failure suggests the lfd changes may have compilation issues — which compounds the integration risk.

## Chord-Level

**Balance**: Severely imbalanced. chord-model shipped 8 PRs and is blocked on a minor conflict. agent-embedding shipped 4 PRs but has accumulated a 4300-line branch with deep cross-area violations. chord-model is doing focused, incremental work. agent-embedding is doing broad, entangled work.

**Gaps**: Terminal session infrastructure (store, API, types) genuinely needs to exist in lfd, but no wave owns that work through proper channels. agent-embedding built it out of necessity for the UI layer, but it belongs in chord-model's area. The emerging `wave/lfd/` directory hints at a third wave being needed — or at minimum, the lfd work needs to route through chord-model.

**Phase**: The current phase (tend flow proving out + agent embedding) still makes sense. The problem isn't the phase — it's that the two waves collided at the lfd boundary, which was predictable given that any meaningful Concerto feature needs daemon support.

## Pressure Points

1. **Area boundary violation.** agent-embedding's PR #567 has ~20 files of lfd changes that belong in chord-model's area. This is the root cause of the coordination deadlock. The lfd work needs to be extracted, landed through chord-model (or a shared PR), and then agent-embedding rebases the Swift layer on top.

2. **PR #567 is too large.** At 4358 insertions across 80 files with failing CI, this PR cannot land as-is. It needs to be split: lfd infrastructure first, then Concerto UI on top. The uncommitted 20 files make this worse — the actual scope is even larger than the PR shows.

3. **Stale worktree accumulation.** Three chord-model worktrees and two agent-embedding run worktrees add confusion. The older worktrees should be pruned to reduce cognitive overhead and prevent accidentally working in the wrong one.
