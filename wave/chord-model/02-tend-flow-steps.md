# 02: Tend Flow Steps

**Finish line:** Four new loopflow steps — `scan-waves`, `assess`, `propose`, `apply` — form the `tend` flow and run against a chord-wave's member waves through the existing wave model. `lf tend` can observe the redesign chord-wave, propose changes, and either apply or queue them for human calibration.

## Context

Bootstrap is already live: `redesign` registers as a regular wave, its membership lives in `wave/redesign/redesign.yaml`, and the redesign waves start in `manual` mode so the structure exists before automation does. These steps have to operate on that waves-only baseline.

That means:

- No separate chord DTOs or CRUD routes
- Member discovery comes from `area` entries that point at `wave/<name>/` directories
- The tend input has to combine filesystem definition with lfd runtime state
- Default chord-wave behavior belongs here: if a wave's area points at `wave/`, the flow should treat it as tending waves rather than files

## What to build

### scan-waves

Read member wave state. The prompt receives:
- Wave configs (area, direction, flow, agent, triggers, work items, mode)
- Recent run history (last N runs per wave — status, duration, what shipped)
- Branch/PR state (open PRs, merge status, CI results)
- Open blocks and recent self-healing attempts
- Recent git activity (commits per wave, files touched)

Output: a structured scan report in `scratch/` that `assess` can consume without ad-hoc parsing.

### assess

Compare the scan output against the chord-wave's directions and design intent. Ask:
- Is each member wave still earning its place?
- Is the overall shape coherent, or are waves colliding or leaving gaps?
- Are we making measurable progress, or staying busy on shallow work?
- Do agents have the tools to validate the quality they are shipping?
- Is the human still connected to what the system is producing?
- Are any waves stalled, conflicted, or drifting from the redesign vision?

Output: an assessment in `scratch/` with observations, concerns, and a health read per wave.

### propose

Turn assessment into concrete mutations. The levers stay the same: direction, area, flow, work items, agent, step agents, triggers, and lifecycle. Each proposal needs rationale and a confidence level so `apply` can distinguish mechanical changes from human-review changes.

Output: a proposal in `scratch/` with each mutation marked `auto-apply` or `needs-human`.

### apply

Execute the proposal on the waves-only model. Two paths:
- **Auto-apply:** mechanical changes the chord-wave is confident about. Update wave config or work items and log the mutation.
- **Needs-human:** create a calibration/block-queue entry with the proposal, rationale, and expected tradeoff.

Output: applied changes plus human-review items for anything that should not mutate silently.

## The tend flow definition

```yaml
# flows/tend.yaml
steps:
  - scan-waves
  - assess
  - propose
  - apply
```

## Done when

- All four steps exist in `steps/` and are runnable individually
- `lf tend` runs the full flow against a chord-wave
- Scan output is structured and consumable by `assess`
- `propose` produces concrete, actionable mutations with rationale
- `apply` can mutate wave configs/work items and create calibration entries without reintroducing chord-specific APIs
- The redesign chord-wave's first tend cycle runs successfully
