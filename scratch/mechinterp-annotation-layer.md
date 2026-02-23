# Annotation Layer

Tag every LLM call loopflow makes with structured workflow metadata. The annotation is inert until a research partner can join it to activation data — but it's ready when one appears.

## Design

### Attachment point: the agent

The agent invocation is the primary annotation surface. When loopflow spawns an agent (`lf implement`, `lf gate`, etc.), it wraps the invocation in a metadata envelope. Every API call the agent makes inherits that metadata.

```
loopflow spawns agent
  → metadata envelope attached (step, direction, area, flow, context)
    → agent makes N requests, all tagged
    → agent completes, outcome signal attached retroactively
```

### Metadata envelope

At agent spawn (known before the agent runs):

| Field | Example | Why a researcher cares |
|-------|---------|----------------------|
| `step.type` | `gate` | Different task types may activate different circuits |
| `step.direction` | `["designer"]` | Perspectives may create detectable activation shifts |
| `step.area` | `src/api/` | Scope of the codebase the agent is working in |
| `flow.name` | `ship` | Which process is being followed |
| `flow.position` | `3/4` | Where in the chain this step falls |
| `context.docs` | `["CLAUDE.md", "area/api.md"]` | Which documents were assembled into context |
| `context.artifacts` | `["scratch/design.md"]` | Artifacts from previous steps in the flow |
| `context.diff` | `true` | Whether the current branch diff was included |
| `wave.name` | `engbot` | Which recurring workflow this belongs to |
| `wave.iteration` | `5` | How many times this wave has run |

At agent completion (known after):

| Field | Example | Why a researcher cares |
|-------|---------|----------------------|
| `outcome.verdict` | `SHIP` | Ground truth for studying gate calibration |
| `outcome.tests` | `pass` | Objective signal for code quality |
| `outcome.artifacts_produced` | `["src/api/handler.rs"]` | What the agent actually changed |
| `outcome.turns` | `12` | How many agent iterations it took |
| `outcome.duration_ms` | `45000` | Time to completion |

### Propagation

Open design question: how does the envelope travel from loopflow → coding agent → API calls?

| Mechanism | Portability | Fidelity | Notes |
|-----------|-------------|----------|-------|
| Environment variables | High | Low | `LF_STEP_TYPE=gate` — agent must opt in to read and forward |
| API metadata passthrough | Medium | High | Depends on agent supporting a metadata field |
| Sidecar file | High | Medium | `.lf/context.json` — most portable, agent reads on startup |
| Claude Code hooks | Low | High | Hooks inject metadata into API calls — Claude Code only |

The right answer depends on what Anthropic can consume. First collaboration question: "if we tag our traffic with structured workflow metadata, what mechanism lets your team join it to activation data?"

### What this enables

With the envelope attached, a researcher can query:

- "Show me all gate verdicts where the model said SHIP but tests later failed" → overconfidence circuits
- "Compare activations across the same step with designer vs. infra-engineer direction" → perspective features
- "Track activation patterns across steps 1-4 of a ship flow" → behavioral drift across chains
- "Find cases where context included conflicting style guide and direction instructions" → conflict resolution circuits

## Next Steps

1. Define the envelope schema (JSON schema or protobuf)
2. Implement sidecar file write at agent spawn (`.lf/context.json`)
3. Implement outcome signal write at agent completion
4. Add Claude Code hook that reads sidecar and attaches to API metadata header
5. Validate round-trip: spawn agent → metadata written → API calls tagged → outcome recorded
