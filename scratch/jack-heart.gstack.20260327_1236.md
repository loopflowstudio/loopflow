# gstack stage 2: file conventions + flows

Two pieces of work: rewrite gstack step file conventions to use loopflow-standard paths, then build flow YAML files that chain them.

## File convention rewrite

gstack steps currently coordinate through `~/.gstack/projects/$SLUG/` with datetime-stamped filenames and `ls -t | head -1` discovery. Rewrite to use loopflow conventions so gstack steps chain with builtin steps (`implement`, `gate`, etc.) and with each other.

### Path mapping

**`scratch/` — in context, cleared on merge:**

| Was | Becomes | Notes |
|-----|---------|-------|
| `~/.gstack/projects/$SLUG/{user}-{branch}-design-*.md` | `scratch/<branch>.md` | The design doc. One file, progressively elaborated by each step (office-hours writes, ceo-review/eng-review/design-review/autoplan update). |
| `~/.gstack/projects/$SLUG/{user}-{branch}-test-outcome-*.md` | `scratch/test-outcome.md` | QA results. Ship reads. |

**`.gstack/` — persistent, not in context:**

| Was | Becomes | Notes |
|-----|---------|-------|
| `~/.gstack/projects/$SLUG/*-reviews.jsonl` | `.gstack/reviews.jsonl` | Review gate log. Ship checks gate status, retro reads completion data. |
| `.gstack/qa-reports/` | `.gstack/qa-reports/` | Unchanged. Bulky reports + screenshots. |
| `.gstack/security-reports/` | `.gstack/security-reports/` | Unchanged. CSO JSON output. |
| `.context/retros/` | `.gstack/retros/` | Retro snapshots for trend comparison. Cross-session. |
| `~/.gstack/projects/$SLUG/{BRANCH}-autoplan-restore-*.md` | `.gstack/restore/` | Autoplan session recovery. Cross-session. |
| `~/.gstack/projects/$SLUG/land-deploy-confirmed` | `.gstack/land-deploy-confirmed` | First-deploy marker. Cross-session. |
| `~/.gstack/projects/$SLUG/ceo-plans/` | `scratch/<branch>.md` | CEO plan merges into the design doc, not a separate file. |

### What goes away

- `~/.gstack/projects/$SLUG/` directory and the `gstack-slug` helper — no longer needed
- Datetime-stamped filenames — scratch/ is branch-scoped, one file per artifact
- `ls -t | head -1` glob discovery — just read the known path
- `gstack-review-log` / `gstack-review-read` shell helpers — steps append to `.gstack/reviews.jsonl` directly
- `/tmp/` intermediaries — steps that wrote temp files for codex prompts can write to scratch/ instead

### The design doc contract

`scratch/<branch>.md` is *the* design doc. Steps don't create separate artifacts — they update this file with their perspective:

- `office-hours` — writes the initial design
- `ceo-review` — adds CEO plan, priorities, scope decisions
- `design-review` — adds design constraints, UX considerations
- `eng-review` — adds technical constraints, test strategy
- `autoplan` — adds test plan, implementation sequence

The implementing session sees one coherent doc. This is the handoff that makes `gstack:office-hours → implement` and `gstack:office-hours → gstack:ceo-review → implement` both work.

### Implementation approach

All 29 steps need path rewrites. The changes are mechanical — same content and logic, different file paths. For each step:

1. Replace `~/.gstack/projects/$SLUG/` paths with `scratch/` or `.gstack/` per the mapping above
2. Remove `gstack-slug` invocations
3. Replace `ls -t | head -1` discovery with direct path reads
4. Replace `gstack-review-log` / `gstack-review-read` calls with direct JSONL append/read
5. Change "write a new artifact" instructions to "update scratch/<branch>.md" where applicable

The converter (`python/loopflow/workstyle/convert.py`) already rewrites step references. Add a post-conversion pass for file path conventions, or do it as a manual edit pass on the already-imported steps.

## Flows

### gstack-sprint — full lifecycle

```yaml
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

### gstack-plan-manual — interactive planning

```yaml
- gstack:ceo-review
- gstack:design-review
- gstack:eng-review
```

### gstack-review — parallel deep review with custom synthesizer

```yaml
- and:
    branches:
      - step: gstack:review
      - step: gstack:cso
      - step: gstack:codex
    synthesize: gstack:review-synthesize
```

### New engine feature: custom synthesizer on `and`

Add `synthesize: Option<String>` to `FlowItem::And`. Parse from YAML, carry through to `ConcreteAnd`, use named step instead of hardcoded `FORK_SYNTHESIZE_STEP` when present. Fall back to builtin when omitted.

### New step: gstack:review-synthesize

Synthesis prompt combining three review perspectives. Reconcile overlapping findings, calibrate severity, distinguish substantive disagreements from model-flavor differences.

## Validation

```bash
cargo test --all
cargo fmt --check
cargo clippy -- -D warnings
uv run pytest python/tests/
# Spot-check: grep for ~/.gstack/projects in step files (should be zero matches after rewrite)
grep -r 'gstack/projects' .lf/workstyles/gstack/steps/ | wc -l
```

## Done-when

- [ ] No gstack step references `~/.gstack/projects/$SLUG/` — all use `scratch/` or `.gstack/`
- [ ] `gstack:office-hours` writes to `scratch/<branch>.md`; `implement` finds it
- [ ] Review steps update `scratch/<branch>.md` rather than creating separate artifacts
- [ ] `.gstack/reviews.jsonl` replaces the review-log/review-read helper pattern
- [ ] Flow YAML files parse and chain gstack steps with loopflow builtins
- [ ] `and` constructs accept optional `synthesize` field
- [ ] `lf --list` shows gstack flows
