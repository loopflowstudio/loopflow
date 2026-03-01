# Built-in Flows

Flows shipped with loopflow. Organized by purpose.

## Code flows (`code/`)

Flows that produce code changes.

| Flow | Steps | Use case |
|------|-------|----------|
| `build` | implement → compress → lint → gate → update-wave | Headless build and wave reconciliation |
| `ship` | design → build → review → land | Interactive design, headless build, interactive review, then land |
| `pair` | design → build | Interactive design then build |
| `grind` | research → iterate → build → gate | Research-driven iteration |
| `incident` | debug → 5whys → build | Fix bug, analyze root cause, build fixes |
| `start` | ingest → kickoff | Pick wave item, elaborate design |
| `ship-wave` | start → build | Pick wave item, elaborate, then build |

## Plan flows (`plan/`)

Flows that produce wave items and analysis.

| Flow | Steps | Use case |
|------|-------|----------|
| `wave-reduce` | fork(reduce×3) → update-wave | Find simplification opportunities |
| `wave-polish` | fork(polish×3) → update-wave | Find polish priorities |
| `wave-expand` | fork(expand×3) → update-wave | Find expansion opportunities |

## Fork pattern

Plan flows use forks to get multiple perspectives:

```yaml
- fork:
    step: reduce
    drafts:
      - direction: infra
      - direction: ux
      - direction: ceo
- update-wave
```

The fork runs `reduce` three times with different directions, then reconciles results via `update-wave`.

## Adding a flow

1. Create `{category}/{name}.yaml` with step list
2. Update this README
