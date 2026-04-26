---
asana_id: '1213879276306962'
linear_id: 98d8f734-1f32-470b-b08d-d0d5c9d4c646
notion_id: 32af8f99-3d81-8132-a56d-d07cdbcb3384
---
# Letta integration

**Finish line:** The root wave has Letta-backed persistent memory. Garden cycles load memories before running and write observations after. Root remembers across runs, including which interventions actually helped.

## Context

Letta provides layered memory: core, recall, archival. The architectural boundary stays the same after bootstrap: Letta is a memory service, not an agent runtime. Ordinary waves stay ephemeral with file-based state. Durable qualitative memory belongs at the garden layer.

The governance flows (`govern-identity`, `govern-intelligence`, `govern-control`, `govern-coordination`) and their scan/assess steps now exist as builtins. The garden flow and its `mutate` / `review` follow-up are also shipped. Letta integration can build on real scan observations and governance decisions from those flows.

## What to build

### Stand up Letta

Run a self-hosted Letta instance alongside lfd. Keep the setup reproducible and local to the repo.

### Define memory schema

**Core memories** (always in context, small, high-signal):
- Root-wave principles and durable decisions
- Current priorities and focus areas
- Known anti-patterns and recurring mistakes

**Recall memories** (searchable, medium-term):
- Recent wave activity summaries
- Conflict resolutions and their outcomes
- Repair attempts and whether they worked
- Human calibration decisions and reasoning
- Garden-cycle observations and proposals

**Archival memories** (long-term, searchable):
- Earlier wave layouts and why they changed
- Abandoned approaches and why they were abandoned
- Research findings that should stay available but not hot
- Patterns observed across multiple garden cycles

### Wire memory into garden

```text
garden cycle starts
  -> load core memories
  -> search recall for recent relevant context
  -> search archival when assessment surfaces something historical
  -> run garden/govern flow with memories in prompt context
  -> write new memories:
      - scan observations -> recall
      - assessment conclusions -> recall
      - durable decisions -> core or recall
      - cross-cutting patterns -> archival
garden cycle ends
```

### Memory hygiene

Core memories need a size budget. When core fills, demote lower-value entries to recall. Root should be able to promote, demote, or retire memories as part of ordinary gardening instead of letting the store bloat.

### Pattern use

Memory should feed back into real decisions:
- when a wave stalls in a familiar way, surface the last intervention that helped
- when an incident repeats, reuse prior repair or escalation context
- when calibration keeps flagging the same drift, propose the narrower mutation first

## Done when

- Letta runs alongside lfd via repo-local tooling
- Root has initial core memories seeded from current wave docs and recent decisions
- Garden cycles load and write memories on each run
- After 3+ cycles, recall contains useful history
- Memory search returns relevant context for later cycles
- At least one repeated repair or calibration pattern is recalled and used in a later decision
