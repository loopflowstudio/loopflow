# Garden Assessment — 2026-03-21

## Summary

Five waves, 17 PRs landed in a week, but the system can't observe itself — lfd is down and no wave is registered under its new name. The highest-velocity waves (pm, macos) are approaching the end of their unblocked queues, while the wave that would unlock the next phase (lfd) is stuck on a CI failure. The chord is producing impressive output but losing the ability to coordinate it.

## Wave: model

**Health**: steady

**Evidence**: Three PRs landed this week (#593, #584, #569) delivering real engine capabilities — xor/loop execution, worker capacity, governance flows. 14 commits across engine and builtins. Clean CI, no open PRs, no blocks. Fresh branch with no commits, indicating a between-items pause.

**Pressure**: All 11 items are queued. The wave shipped foundation all week and now faces a sequencing decision: wave-modes (priority 1) is pure engine work that doesn't unblock anything downstream, while tend-flow-steps (priority 3) is the gate to the chord running its own garden cycle — but it needs lfd alive to validate. The wave is productive but potentially building in the wrong order if the goal is chord autonomy.

## Wave: macos

**Health**: thriving

**Evidence**: Four PRs landed in three days (#587→#594), each delivering a complete feature: terminal sessions, multiplexer, wave workspaces, interactive checkpoints. Item 04 (window composition) is in-flight with a design doc and gate pass already on the branch. The wave is crossing finish lines, not just producing commits.

**Pressure**: Item pipeline is thin. After 04 ships, only wave-lifecycle-ui (queued) is unblocked. Portfolio view and calibration view are speculative (priority 4) and depend on data that model hasn't formalized. The chord review said these can read raw state — true, but that makes them harder to build well. The wave will naturally go quiet after its current sprint, which is appropriate given the remaining items' speculative status.

## Wave: lfd

**Health**: blocked

**Evidence**: Two PRs landed (#591, #578), but the current PR (#596 — shared flow execution engine) has been failing rust-test CI for ~12 hours. The active worktree has moved ahead of the PR branch, suggesting a fix may exist locally but hasn't been pushed. lfd itself is not running — every `lfq show` call fails. Two items total, both gated on the executor work converging.

**Pressure**: This is the critical path for the entire chord. lfd being down blocks: model's tend-flow-steps validation, ios's remote items, and the chord's ability to observe its own waves. PR #596 aging with CI failures increases rebase cost against the high-velocity main branch (17 PRs/week). The gap between the active worktree and the PR branch suggests context fragmentation — the fix and the PR are diverging.

## Wave: pm

**Health**: steady, approaching natural silence

**Evidence**: Seven PRs landed this week — highest volume of any wave. The entire PM foundation is now shipped: Asana, Linear, Notion providers with OAuth, bootstrap CLI, pull/export/sync. The worktree has only post-merge cleanup commits. All remaining items are integration and polish (ingest-auto-import, rich text, provider auth consolidation).

**Pressure**: The remaining items are lower urgency and don't block other waves. Ingest-auto-import connects to model's concurrent-ingest, but both halves are queued with no pressure to ship immediately. The wave completed its foundation sprint and can legitimately go quiet. The main risk is that the seven landed PRs represent a large surface area that hasn't been exercised in production — lfd being down means none of the PM sync flows have been validated end-to-end.

## Wave: ios

**Health**: silent

**Evidence**: No PRs landed. No active branches. Stale worktree under old name (dogfood). No worktree under new name. Five items queued, all requiring infrastructure that doesn't exist yet (TestFlight CI, lfd remote, mac mini server).

**Pressure**: None immediate. The wave was just restructured from dogfood and deliberately hasn't started. Every item except concerto-local depends on lfd being operational remotely. This is healthy silence — the prerequisites genuinely aren't met. The wave signals "start here when lfd is stable and you want to deploy to a phone."

## Chord-Level

**Balance**: Severely uneven. pm and macos sprinted through their foundations while lfd — the infrastructure they both need for validation — is stuck. model has engine depth but nothing wired end-to-end. ios hasn't started. The waves that produce user-visible output (macos, pm) are ahead of the waves that make the system self-sustaining (lfd, model).

**Gaps**: Wave rename is incomplete. Worktrees, lfd registrations, and branch naming all still use old wave names. This isn't cosmetic — it means `lfq` commands fail, worktree management is confused, and the restructuring from the chord review hasn't been operationalized. Nobody owns finishing the rename.

Branch cleanup is unowned. 28 stale remote branches, 35 worktrees on disk. No recent pruning. This is accumulating drag on every git operation and making the scan noisier than it needs to be.

**Phase**: The chord is in a transition between "build foundations" (largely complete for pm, macos, model engine) and "wire end-to-end" (not started). The phase shift requires lfd running, which requires PR #596 landing, which requires fixing a rust-test failure. One CI fix is gating the entire chord's phase transition.

## Pressure Points

1. **PR #596 (lfd) CI failure is the single highest-leverage fix.** It blocks lfd from landing, which blocks lfd from running, which blocks model's tend-flow validation, ios's remote items, pm's sync flow validation, and the chord's ability to observe itself. The worktree has moved past the PR branch — reconciling them and getting CI green would unblock four waves.

2. **Wave rename completion is accumulating operational debt.** Every `lfq` call fails. Worktrees are under old names. The chord review decided the restructuring, but nobody has finished implementing it in lfd registrations and worktree naming. Until this is done, the garden cycle can't function — the tooling can't find the waves it's supposed to tend.

3. **Validation gap across all shipped work.** 17 PRs landed this week. lfd is down. None of the new PM providers, flow engine extensions, or Concerto features have been validated through live wave execution. The longer this gap persists, the more likely that landed code has integration bugs that won't surface until everything is wired together — and by then, the blast radius of fixes is much larger.
