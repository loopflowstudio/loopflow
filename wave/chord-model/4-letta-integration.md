---
asana_id: '1213717740994855'
linear_id: 6c656949-5c43-4493-8987-efb8ec3644b0
---
# 04: Letta Integration

**Finish line:** The redesign chord-wave has a Letta agent with persistent memory. Garden cycles load memories before running and write observations after. The chord-wave remembers across runs, including which algedonic and calibration interventions actually helped.

## Context

Letta (formerly MemGPT) provides layered memory: core, recall, archival. The architectural boundary stays the same after bootstrap: Letta is a memory service, not an agent runtime. Waves stay ephemeral with file-based state. The redesign chord-wave is the only place where durable qualitative memory belongs.

Item 02 now covers the first live garden cycle. Finish that first so Letta has a real stream of scan observations, routed decisions, and human calibration events to remember instead of a purely structural test run.

The old `signals/05` idea folds here rather than living in a separate wave. Resolution history, calibration notes, and repeated stall/algedonic patterns belong in chord memory, not in a parallel block system.

## What to build

### Stand up Letta

Run a self-hosted Letta instance alongside lfd. Keep the setup reproducible and local to the repo.

### Define memory schema

**Core memories** (always in context, small, high-signal):
- Design principles from the redesign docs
- Key chord-wave decisions and their rationale
- Current priorities and focus areas
- Known anti-patterns and recurring mistakes

**Recall memories** (searchable, medium-term):
- Recent wave activity summaries
- Conflict resolutions and their outcomes
- Algedonic repair attempts and whether they worked
- Human calibration decisions and reasoning
- Tend-cycle observations and proposals

**Archival memories** (long-term, searchable):
- Full redesign context and history
- Abandoned approaches and why they were abandoned
- Research findings (VSM, Daytona, OpenCode)
- Patterns observed across multiple garden cycles

### Wire memory into garden

```
garden cycle starts
  -> load core memories
  -> search recall for recent relevant context
  -> search archival when assessment surfaces something historical
  -> run garden flow with memories in prompt context
  -> write new memories:
      - scan observations -> recall
      - assessment conclusions -> recall
      - durable decisions -> core or recall
      - cross-cutting patterns -> archival
garden cycle ends
```

### Memory hygiene

Core memories need a size budget. When core fills, demote lower-value entries to recall. The chord-wave should be able to promote, demote, or retire memories as part of ordinary gardening instead of letting the store bloat.

### Pattern use

Memory should feed back into real decisions:
- when a wave stalls in a familiar way, surface the last intervention that helped
- when an algedonic incident repeats, reuse prior repair or escalation context
- when calibration keeps flagging the same drift, propose the narrower mutation first

## Done when

- Letta runs alongside lfd via repo-local tooling
- The redesign chord-wave has initial core memories seeded from the redesign docs
- Garden loads and writes memories on each cycle
- After 3+ garden cycles, recall contains useful history
- Memory search returns relevant context for new garden cycles
- At least one repeated algedonic or calibration pattern is recalled and used in a later garden decision
