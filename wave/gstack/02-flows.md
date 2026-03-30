# Stage 2: Flows

Create loopflow flow YAML files that chain gstack steps into the sprint sequence. Wire namespaced flows into the existing `.lf/flows/` discovery system.

## What to build

**gstack flow definitions** (`.lf/flows/gstack/`):

```yaml
# sprint.yaml — the main flow
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
- flow: gstack-review
- gstack:qa
- gstack:ship
- gstack:document-release
- gstack:land-and-deploy
- gstack:canary
- gstack:retro
```

```yaml
# plan-manual.yaml
- gstack:ceo-review
- gstack:design-review
- gstack:eng-review
```

```yaml
# review.yaml — deep review
- and:
    branches:
      - step: gstack:review
      - step: gstack:cso
      - step: gstack:codex
    synthesize: gstack:review-synthesize
```

**Flow discovery** (`flow.rs` or `discovery.rs`):
- Load namespaced flows from `.lf/flows/<name>/`
- Render namespaced flows as hyphenated names in `lf --list`: `gstack-sprint`, `gstack-review`
- Show branch constructs in summaries so custom flows are legible in listings

**Direction use**:
- gstack style is applied by including the `gstack` direction where it helps
- OpenClaw style is available separately as the `openclaw` direction
- No hidden workstyle-specific voice switching at runtime

## Known state from stage 1

- Some imported steps still reference gstack helper binaries (`gstack-review-read`, `gstack-review-log`, `gstack-config`). These are not loopflow-native. Flows that chain review steps may hit these — either stub them, strip the references, or wire loopflow equivalents.
- `design-review` is the planning skill (from `plan-design-review`). The original audit/fix skill was renamed to `design-audit` to avoid collision after dropping the `plan-` prefix.
- Converted steps live in `.lf/steps/gstack/*.md` and resolve as `gstack:<name>` via namespaced-step discovery.

## Done when

1. `lf gstack-sprint` runs the full sprint sequence
2. `lf gstack-plan-manual` runs the three-review planning flow
3. `lf gstack-review` runs deep review (code + security + cross-model)
4. gstack flows can opt into `direction: [gstack]` explicitly
5. Mixed flows (gstack steps + loopflow `implement`) work correctly
6. `lf --list` shows gstack flows
