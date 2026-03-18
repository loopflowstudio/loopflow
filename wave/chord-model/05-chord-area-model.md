# 05: Chord-Wave Area Model

**Finish line:** The `area over waves` contract is defined and implemented. A chord-wave's tend flow sees wave configs, run history, PR outcomes, block history, and human decisions — everything it needs to tend effectively — without a separate chord data model.

## Context

Bootstrap already established the core rule: membership lives exclusively in `area` entries that point at `wave/<name>/` directories. The filesystem defines each member wave. lfd stores runtime state. This item makes that contract explicit so `scan-waves` and future UI work can depend on one consistent snapshot.

The trigger work on this branch made part of that contract real:
- sourceless `wave` and `block` triggers derive repo-local membership from `area: [wave/<name>/]`
- merged-PR completions and persistent queue blocks already flow through that membership model
- activation coalescing already batches multiple member completions onto one listener trigger

What's still missing is a shared read model. Today the runtime knows enough to wake the chord, but `scan-waves` still has to reconstruct state from files, git, PRs, and queue behavior piecemeal.

## What to build

1. **Wave state snapshot.** For each member wave, expose:
   - Config (`area`, `direction`, `flow`, `agent`, `triggers`, work items, `mode`)
   - Recent runs and activations (last N: status, duration, what changed, why it ran, PR outcome)
   - Current state (running, idle, blocked, stalled)
   - Branch/PR state (open PRs, CI status, review status)
   - Git activity (recent commits, files touched, diff stats)
   - Membership diagnostics (which `wave/<name>/` entries resolved, which did not)

2. **Cross-wave view.** Across all members, expose:
   - File overlap
   - Trigger/dependency relationships
   - Progress timeline and velocity trends
   - Block history and how each block resolved

3. **Human decision history.** Include calibration decisions, proposal approvals/rejections, and manual wave mutations so tending can reason about prior judgment instead of only raw activity.

4. **Wave-based loader or API.** `scan-waves` should be able to load the full snapshot for a wave whose `area` points at `wave/`. No separate chord CRUD comes back.

## Done when

- The area snapshot returns structured data for all member waves
- `scan-waves` uses the shared loader/API instead of ad-hoc gathering
- Cross-wave file overlap is detectable from the snapshot
- Membership diagnostics catch unresolved `wave/<name>/` entries before they silently skew the chord
- Human decision history is included in the snapshot
