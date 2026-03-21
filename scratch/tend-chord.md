# Chord — 2026-03-20

## Context

Both waves are shipping fast but drifting out of phase. Chord-model is building engine depth while agent-embedding is about to exhaust its unblocked items. The single highest-leverage move is resequencing chord-model to deliver the area model before agent-embedding stalls.

## Mutations

### 1. Resequence chord-model: area model before engine depth

**Wave**: chord-model
**Lever**: Items
**Before**: 4-chord-wave-area-model is queued behind 4-tend-flow-steps and 4-vsm-flow (both in-flight validation work). The wave is working depth-first on engine internals.
**After**: 4-chord-wave-area-model moves to top priority alongside 4-tend-flow-steps. Next ingest picks area-model if tend-flow-steps is blocked on lfd reachability.
**Rationale**: Area model is the gate for agent-embedding items 02 (portfolio view) and 03 (calibration view) — the two items that make Concerto a conductor. Without it, agent-embedding stalls after item 04 ships. Tend-flow-steps remains important but is blocked on lfd being reachable, so the wave shouldn't wait idle while that's resolved.
**Risk**: Area model depends on runtime state conventions that tend-flow-steps would validate. Building the model before proving the tend cycle could mean building against assumptions that don't hold live. Mitigation: the model's core contract (member waves derived from `area` entries pointing at `wave/<name>/`) is already established and doesn't depend on lfd — the richer runtime fields can follow.

### 2. Silence agent-embedding after items 04 and 01

**Wave**: agent-embedding
**Lever**: Silence
**Before**: Four items total. Item 04 (window composition) in-flight, items 01/02/03 queued. Items 02 and 03 are blocked on chord-model delivering chord-wave-area-model.
**After**: After items 04 and 01 ship, agent-embedding goes silent — no items, wave watches its area. Wakes when chord-model delivers the area model and items 02/03 become buildable.
**Rationale**: Keeping the wave active with no unblocked work generates make-work or drift. Silence shrinks the blocking queue — one fewer wave competing for review means faster throughput on chord-model, which is where the leverage is right now. Agent-embedding has shipped four PRs this week; it's earned a pause.
**Risk**: Momentum loss. The wave has been shipping daily; going silent could make it harder to restart. Mitigation: the wake condition is concrete (area-model lands), and items 02/03 are well-defined — restart is a cold-start on a clear design, not a rediscovery.

## Coherence

These two mutations reinforce each other. Mutation 1 (resequence chord-model) produces the artifact that mutation 2's wake condition depends on. Silencing agent-embedding reduces review pressure, giving chord-model more attention bandwidth to deliver area-model faster. No conflicts, no ordering dependency between them — both can be applied immediately.

The combined effect: chord-model shifts from depth-first to breadth-first for one item, agent-embedding pauses gracefully, and the chord reconverges when the area model ships.

## Deferred

**lfd reachability.** The assessment's #1 pressure point. This is an ops issue (process not running or auth token expired), not a wave mutation. Diagnosing it requires hands-on debugging — starting lfd, checking token state, reading logs. The tend-flow-steps item already owns this gap. No wave config change helps here; it needs a human with a terminal.

**Open PRs with CI failures (#596, #589).** These are accumulating rebase cost but don't warrant wave mutations. They're mechanical — fix scratch-clear, push, re-run CI. Flagging for the next interactive session rather than mutating wave configs around them.

**vsm-flow wiring.** In-flight in chord-model but lower priority than area-model and tend-flow-steps. The four governance flows shipped; the single-pass `lf vsm` command is nice-to-have, not a gate for anything else. Let it stay queued behind the two priority items.
