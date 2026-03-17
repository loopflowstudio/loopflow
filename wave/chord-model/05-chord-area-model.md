# 05: Chord Area Model

**Finish line:** The "area over waves" contract is defined and implemented. A chord-wave's tend flow sees wave configs, run history, PR outcomes, block history, and human decisions — everything it needs to tend effectively.

## Context

A wave's area is files. A chord-wave's area is waves. But what does that mean concretely? When the tend flow runs, what data does it receive? This item formalizes the contract based on what the tend flow actually needed during early cycles.

## What to build

1. **Wave state snapshot.** For each member wave, the chord-wave sees:
   - Config (area, direction, flow, agent, triggers, work items)
   - Recent runs (last N: status, duration, what changed, PR outcome)
   - Current state (running, idle, blocked, stalled)
   - Branch/PR state (open PRs, CI status, review status)
   - Git activity (recent commits, files touched, diff stats)

2. **Cross-wave view.** Aggregated across all members:
   - File overlap (which waves are touching the same files)
   - Dependency graph (which waves trigger which)
   - Progress timeline (what shipped when, velocity trends)
   - Block history (what blocked, how it was resolved, by whom)

3. **Human decision history.** From the block queue:
   - Calibration decisions and their rationale
   - Proposal approvals/rejections
   - Manual wave mutations (human changed something directly)

4. **Wave-based API endpoint.** `GET /v0/waves/{id}/area` (or equivalent tend input loader) returns the full area snapshot for a wave whose area points at `wave/`. No separate chord CRUD.

## Done when

- Wave area API returns structured data for all member waves
- Scan step uses the API instead of ad-hoc data gathering
- Cross-wave file overlap is detectable from the snapshot
- Human decision history is included
