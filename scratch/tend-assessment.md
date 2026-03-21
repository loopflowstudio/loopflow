# Tend Assessment — 2026-03-20

## Summary

Both waves are shipping at high velocity — 7 PRs landed across chord-model and agent-embedding this week — but the work is approaching a dependency cliff. Agent-embedding is running out of unblocked items (02 and 03 depend on chord-model pieces that haven't been built), and the tend/garden cycle can't be validated live because lfd is unreachable. The chord is producing motion; the question is whether it's building toward the right finish lines in the right order.

## Wave: chord-model

**Health**: steady

**Evidence**: Three PRs landed (#593, #584, #569) touching 14 commits across engine, lfd, and builtins. Two items nominally in-flight (tend-flow-steps, vsm-flow), but both are validation/wiring work — the structures exist as builtins, they just haven't been proven end-to-end against a live lfd. Ten items queued. Fresh branch, clean working tree, no blocks, no CI failures.

**Pressure**: The in-flight items are the gate to the chord's autonomy. Tend-flow-steps must run live before Letta integration makes sense, and Letta integration is the path to the chord retaining memory across garden cycles. But lfd is unreachable (auth/process issue), so live validation is stuck. Meanwhile, chord-wave-area-model (queued) is what agent-embedding needs to build portfolio and calibration views. The wave is working depth-first on engine internals when a breadth-first pass on area-model would unblock its sibling.

## Wave: agent-embedding

**Health**: steady, approaching drift

**Evidence**: Four PRs landed (#594, #592, #588, #587) — terminal multiplexer, attention lifecycle, interactive checkpoints, wave workspaces. Item 04 (window composition) just kicked off with a design doc. Three items queued: 01 (wave lifecycle UI), 02 (portfolio view), 03 (calibration view). Items 02 and 03 are blocked on chord-model delivering the area model and governance wiring.

**Pressure**: The item pipeline is thin — 4 items total, and 2 of the 3 queued items are blocked on another wave. After item 04 ships, item 01 is the only unblocked work remaining. If chord-model doesn't deliver chord-wave-area-model soon, agent-embedding either stalls or generates make-work. The wave's README promises "multi-repo, multi-wave status at a glance" and "dedicated UX for garden flow's human checkpoint" — neither can be built until the data model exists.

## Chord-Level

**Balance**: Both waves are shipping at comparable velocity, but they're drifting out of phase. Chord-model is building engine depth (xor/loop engine, worker capacity, governance flows) while agent-embedding is building UI surface (terminal sessions, workspaces, checkpoints). The surfaces will need the depth to become real — and that handoff hasn't been sequenced.

**Gaps**: lfd health monitoring. PR #596 (runtime journals) has CI failures and touches both waves' area. Nobody is actively fixing it — it's sitting with failing checks. The lfd auth/process issue (lfq returning "invalid token") is undiagnosed.

**Phase**: Phase 1 (bootstrap tend cycle) is structurally complete — the steps and flows exist as builtins. But it hasn't been proven live, which means the chord can't graduate to phase 2 (build/garden counterpoint) with confidence. The scan shows high build velocity in both waves but zero garden cycles completed. The chord is still in phase 1.

## Pressure Points

1. **lfd reachability gates everything downstream.** The tend/garden cycle can't run live. Letta integration is blocked. The chord can't graduate to autonomous gardening. Fixing the auth/process issue or validating the tend cycle some other way is the single highest-leverage action.

2. **chord-wave-area-model unblocks agent-embedding.** Portfolio view and calibration view — the two items that make Concerto a conductor instead of a chat client — are waiting on this queued chord-model item. Pulling it forward in the sequence would keep agent-embedding productive after item 04 ships.

3. **Open PRs with CI failures are accumulating drag.** #596 (lfd runtime journals) and #589 (pm sync) both fail scratch-clear. They touch shared infrastructure. Letting them age increases rebase cost and blocks downstream work in both waves.
