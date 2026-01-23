# Execution Model Redesign

## Target Naming

```
Flow (definition - sequence of steps)
  └── Step (component definition)

Agent (optional)
  └── FlowRun (execution of a flow)
        └── StepRun (execution of a step)

StepRun can exist without FlowRun (interactive `lf step`)
```

## Current → Target Mapping

| Current   | Target    | Notes |
|-----------|-----------|-------|
| Run       | FlowRun   | Execution of a flow by an agent |
| Session   | StepRun   | Unified: can have `flow_run_id` or be standalone |

## StepRun Model

```python
class StepRun:
    id: str
    step: str                    # which step was run
    flow_run_id: str | None      # parent FlowRun (None = standalone/interactive)
    agent_id: str | None         # owning agent (None = interactive)
    status: StepRunStatus
    started_at: datetime
    ended_at: datetime | None
    worktree: str | None
    # ... other fields
```

## Benefits

1. **Unified tracking** - both agent and interactive executions use same model
2. **Explicit naming** - FlowRun vs StepRun makes the hierarchy clear
3. **Composable** - StepRun works with or without parent FlowRun
4. **Matches mental model** - "the agent did 3 step runs" reads naturally
