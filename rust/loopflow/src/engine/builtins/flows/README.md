# Built-in Flows

Flows shipped with loopflow. Organized by purpose.

## Code flows (`code/`)

Flows that produce code changes.

| Flow | Steps | Use case |
|------|-------|----------|
| `ship` | implement → compress → gate | Build from design, ship clean |
| `design-ship-review` | design → ship → review | Interactive design, headless ship, interactive review |
| `pair` | design → ship | Interactive design then build |
| `grind` | research → iterate → ship → gate | Research-driven iteration |
| `incident` | debug → 5whys → ship | Fix bug, analyze root cause, ship fixes |
| `start` | ingest → kickoff | Pick wave item, elaborate design |
| `ship-wave` | start → ship → update-wave | Pick wave item, elaborate, build, then update wave plan |

## Plan flows (`plan/`)

Flows that produce wave items and analysis.

| Flow | Steps | Use case |
|------|-------|----------|
| `wave-reduce` | fork(reduce×3) → publish | Find simplification opportunities |
| `wave-polish` | fork(polish×3) → publish | Find polish priorities |
| `wave-expand` | fork(expand×3) → publish | Find expansion opportunities |
| `publish` | consolidate → add-to-wave | Promote scratch/ to wave/ |

## Fork pattern

Plan flows use forks to get multiple perspectives:

```yaml
- fork:
    step: reduce
    drafts:
      - direction: infra
      - direction: ux
      - direction: ceo
- publish
```

The fork runs `reduce` three times with different directions, then automatically synthesizes the results before publishing to wave/.

## Adding a flow

1. Create `{category}/{name}.yaml` with step list
2. Update this README
