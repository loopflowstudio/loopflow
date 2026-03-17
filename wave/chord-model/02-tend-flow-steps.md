# 02: Tend Flow Steps

**Finish line:** Four new loopflow steps — `scan-waves`, `assess`, `propose`, `apply` — that together form the tend flow. Runnable via `lf scan-waves`, `lf assess`, etc. against a chord's member waves. The counterpoint to build.

## Context

Build creates. Tend maintains. A wave's natural flow is build (code, tests, PRs). A chord's natural flow is tend (scan, assess, propose, apply). Same flow engine, same infrastructure — the difference is area: files vs waves.

The tend flow is the chord's primary loop. It needs to work with the existing flow engine — these are steps like any other, assembled into a flow.

## What to build

### scan-waves

Read member wave state. The prompt receives:
- Wave configs (area, direction, flow, work items)
- Recent run history (last N runs per wave — status, duration, what shipped)
- Branch/PR state (open PRs, merge status, CI results)
- Open blocks (anything that's stuck)
- Recent git activity (commits per wave, files touched)

Output: structured scan report in scratch/. Machine-readable enough for assess to consume.

### assess

Compare scan output against the chord's directions and the design doc. The prompt asks:
- Is each wave still earning its place?
- Is the overall shape coherent — are the waves collectively building what we intend?
- Are we making real, measurable progress? Or staying busy on easy stuff?
- Are we lost in details that don't matter, or skipping details that do?
- Do agents have the tools to evaluate they're creating polished, reliable experiences?
- Is the human still connected to what's being produced, or is there drift?
- Are any waves in conflict (same files, competing approaches)?
- Is any wave stalled (activity without progress) or producing shallow work?

Output: assessment in scratch/. Observations, concerns, and a health rating per wave.

### propose

Based on assessment, suggest concrete wave mutations. The levers:
- **Direction**: add/remove directions to shift what a wave optimizes for
- **Area**: tighten or widen scope
- **Flow**: change the process (add research step, remove unnecessary gates)
- **Work items**: re-prioritize, rewrite, delete stale items
- **Agent**: shift model (opus for research, haiku for cleanup)
- **Step agents**: different models for different steps in the flow
- **Triggers**: change what triggers this wave and how often
- **Wave lifecycle**: pause, resume, split, combine, or prune a wave

Output: proposal in scratch/. Each mutation with rationale. Flagged as auto-apply (mechanical) or needs-human (judgment call).

### apply

Execute the proposal. Two paths:
- **Auto-apply**: mechanical changes the chord is confident about. Update wave config, reorder work items, adjust triggers. Log what changed.
- **Needs-human**: route to the block queue. The human sees the proposal in a calibration moment — trajectory review, not just status. They approve, modify, or reject each mutation.

Output: applied changes + block queue entries for human review.

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
- `lf tend` runs the full flow against a chord
- Scan output is structured and consumable by assess
- Propose produces concrete, actionable mutations with rationale
- Apply can modify wave configs and create block queue entries
- The redesign chord's first tend cycle runs successfully
