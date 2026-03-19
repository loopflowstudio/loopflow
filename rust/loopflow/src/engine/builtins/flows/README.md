# Built-in Flows

Flows shipped with loopflow. Organized by purpose.

## Code flows (`code/`)

Flows that produce code changes.

| Flow | Steps | Use case |
|------|-------|----------|
| `build` | implement → compress → lint → gate → update-wave | Headless build and wave reconciliation |
| `ship` | design → build → or(demo, code-review) → land | Interactive design, headless build, then the right review before landing |
| `pair` | design → build | Interactive design then build |
| `grind` | research → iterate → build → gate | Research-driven iteration |
| `incident` | debug → 5whys → build | Fix bug, analyze root cause, build fixes |
| `start` | ingest → kickoff | Pick wave item, elaborate design |
| `ship-wave` | start → build | Pick wave item, elaborate, then build |

## Plan flows (`plan/`)

Flows that produce wave items and analysis.

| Flow | Steps | Use case |
|------|-------|----------|
| `wave-reduce` | and(reduce×3) → update-wave | Find simplification opportunities |
| `wave-polish` | and(polish×3) → update-wave | Find polish priorities |
| `wave-expand` | and(expand×3) → update-wave | Find expansion opportunities |

## Tend flows (`tend/`)

Flows that let a chord scan, assess, and tune its member waves.

| Flow | Steps | Use case |
|------|-------|----------|
| `tend` | scan-waves → assess → or(tune, silence) | Assess chord health and decide whether to tune |

## VSM flows (`vsm/`)

Flows that walk the viable system model from governance down to execution.

| Flow | Steps | Use case |
|------|-------|----------|
| `govern-identity` | s5-scan → s5-assess → play-chord | Assess chord identity and apply structural mutations |
| `govern-intelligence` | s4-scan → s4-assess → play-chord | Assess environmental changes and apply relevant mutations |
| `govern-control` | s3-scan → s3-assess → play-chord | Assess control health and apply resource or mechanical fixes |
| `govern-coordination` | s2-scan → s2-assess → play-chord | Assess coordination risk and apply interference fixes |

## Scan flows (`scan/`)

Flows that scan the environment and turn the findings into roadmap changes.

| Flow | Steps | Use case |
|------|-------|----------|
| `scan` | scan-report → scan-plan → build | Report external changes, plan the response, then ship it |

## And pattern

Plan flows use `and` to get multiple perspectives:

```yaml
- and:
    branches:
      - step:
          name: reduce
          direction: [infra]
      - step:
          name: reduce
          direction: [ux]
      - step:
          name: reduce
          direction: [ceo]
- update-wave
```

The `and` runs `reduce` three times with different directions, then reconciles results via `update-wave`.

## Adding a flow

1. Create `{category}/{name}.yaml` with step list
2. Update this README
