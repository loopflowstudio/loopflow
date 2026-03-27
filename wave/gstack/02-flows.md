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
- Workstyle flows can opt into imported style explicitly with `direction: [gstack]`

**Direction use**:
- gstack style is applied by including the `gstack` direction where it helps
- OpenClaw style is available separately as the `openclaw` direction
- No hidden workstyle-specific voice switching at runtime

## Known state from stage 1

- Some imported steps still reference gstack helper binaries (`gstack-review-read`, `gstack-review-log`, `gstack-config`). These are not loopflow-native. Flows that chain review steps may hit these — either stub them, strip the references, or wire loopflow equivalents.
- `design-review` is the planning skill (from `plan-design-review`). The original audit/fix skill was renamed to `design-audit` to avoid collision after dropping the `plan-` prefix.
- Converted steps live in `.lf/workstyles/gstack/steps/*.md` and resolve as `gstack:<name>` via `SkillSourceKind::Workstyle` in discovery.

## Done when

1. `lf gstack-sprint` runs the full sprint sequence
2. `lf gstack-plan-manual` runs the three-review planning flow
3. `lf gstack-review` runs deep review (code + security + cross-model)
4. gstack flows can opt into `direction: [gstack]` explicitly
5. Mixed flows (gstack steps + loopflow `implement`) work correctly
6. `lf --list` shows gstack flows
