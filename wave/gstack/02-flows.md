# Stage 2: Flows

Create loopflow flow YAML files that chain gstack steps into the sprint sequence. Wire workstyle flows into the flow discovery system.

## What to build

**gstack flow definitions** (`.lf/workstyles/gstack/flows/`):

```yaml
# gstack-sprint.yaml — the main flow
# think → plan → build → review → test → ship → reflect
- gstack:office-hours
- xor:
    router: gstack:office-hours
    paths:
      autoplan:
        step: gstack:autoplan
        description: "Auto-plan with minimal interaction"
      manual:
        flow: gstack-plan-manual
        description: "Interactive planning — CEO, design, eng reviews"
- implement
- gstack:review
- gstack:qa
- gstack:ship
- gstack:retro
```

```yaml
# gstack-plan-manual.yaml
- gstack:ceo-review
- gstack:design-review
- gstack:eng-review
```

```yaml
# gstack-review.yaml — deep review
- gstack:review
- gstack:cso
- gstack:codex
```

**Flow discovery** (`flow.rs` or `discovery.rs`):
- Load flows from `.lf/workstyles/<name>/flows/` in addition to `.lf/flows/`
- Workstyle flows are namespaced: `gstack-sprint`, `gstack-review`
- When a workstyle flow runs, the workstyle's voice.md is active

**Voice resolution**:
- Current: `.lf/voice.md` → `~/.lf/voice.md` → builtin
- New: workstyle `voice.md` → `.lf/voice.md` → `~/.lf/voice.md` → builtin
- Workstyle voice is active when any step from that workstyle is running

## Done when

1. `lf gstack-sprint` runs the full sprint sequence
2. `lf gstack-plan-manual` runs the three-review planning flow
3. `lf gstack-review` runs deep review (code + security + cross-model)
4. gstack voice is active during gstack steps, loopflow voice during loopflow steps
5. Mixed flows (gstack steps + loopflow `implement`) work correctly
6. `lf --list` shows gstack flows
