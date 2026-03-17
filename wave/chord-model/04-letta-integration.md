# 04: Letta Integration

**Finish line:** The redesign chord has a Letta agent with persistent memory. Tend cycles load memories before running and write observations after. The chord remembers across runs.

## Context

Letta (formerly MemGPT) provides layered memory: core, recall, archival. Self-hosted via Docker, REST API, Python SDK. Apache-2.0.

Key design decision: Letta is a memory service, not an agent runtime. Thin integration. Waves stay ephemeral with file-based state. The chord is the only thing with persistent qualitative memory. This is the architectural boundary that makes chords more than fancy cron jobs.

## What to build

### Stand up Letta

Self-hosted Letta instance via Docker Compose. Runs alongside lfd. No external dependencies.

### Define memory schema

**Core memories** (always in context, small, high-signal):
- Design principles from the redesign doc
- Key decisions made and their rationale
- Current priorities and focus areas
- Known anti-patterns and past mistakes

**Recall memories** (searchable, medium-term):
- Recent wave activity summaries
- Conflict resolutions and their outcomes
- Human calibration decisions and reasoning
- Tend cycle observations

**Archival memories** (long-term, searchable):
- Full redesign context and history
- Abandoned approaches and why they were abandoned
- Research findings (VSM, Daytona, OpenCode)
- Patterns observed across multiple tend cycles

### Wire into tend flow

```
tend cycle starts
  → load core memories (always)
  → search recall for recent relevant context
  → search archival if assess surfaces something historical
  → run tend flow with memories in prompt context
  → write new memories:
      - scan observations → recall
      - assessment conclusions → recall
      - decisions made → core (if significant) or recall
      - patterns noticed → archival (if cross-cutting)
tend cycle ends
```

### Memory hygiene

Core memories have a size budget. When core fills, older items get demoted to recall. The chord's assess step can explicitly promote/demote memories: "this decision is now foundational" (→ core) or "this concern resolved itself" (→ archival or delete).

## Done when

- Letta running alongside lfd via Docker Compose
- Chord agent created with initial core memories (design doc principles)
- Tend flow loads and writes memories on each cycle
- After 3+ tend cycles, recall contains useful history
- Memory search returns relevant context for new tend cycles
