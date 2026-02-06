# Built-in Flows

Flows shipped with loopflow. Organized by purpose.

## Code flows (`code/`)

Flows that produce code changes.

| Flow | Steps | Use case |
|------|-------|----------|
| `ship` | implement → compress → gate | Build from design, ship clean |
| `pair` | design → ship | Interactive design then build |
| `grind` | review → iterate → ship → gate | Review-driven iteration |
| `incident` | debug → 5whys → ship | Fix bug, analyze root cause, ship fixes |
| `start` | ingest → kickoff | Pick roadmap item, elaborate design |
| `ship-roadmap` | start → ship | Pick roadmap item, elaborate, build |

## Plan flows (`plan/`)

Flows that produce roadmap items and analysis.

| Flow | Steps | Use case |
|------|-------|----------|
| `roadmap-reduce` | review → fork(reduce×3) → publish | Find simplification opportunities |
| `roadmap-polish` | review → fork(polish×3) → publish | Find polish priorities |
| `roadmap-expand` | review → fork(expand×3) → publish | Find expansion opportunities |
| `research` | explore → review → publish | Investigate then propose |
| `publish` | consolidate → add-to-roadmap | Promote scratch/ to roadmap/ |

## Fork pattern

Plan flows use forks to get multiple perspectives:

```yaml
- review
- fork:
    step: reduce
    drafts:
      - direction: infra-engineer
      - direction: designer
      - direction: product-engineer
- publish
```

The fork runs `reduce` three times with different directions, synthesizes results using the wave's direction, then publishes to roadmap.

## Adding a flow

1. Create `{category}/{name}.yaml` with step list
2. Update this README
