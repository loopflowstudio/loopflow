---
asana_id: '1213718081081138'
linear_id: 70cde070-1b10-4e97-87b0-e72d35e50d7d
---
# Worker Pools + VSM as Chord

## Two lenses, same territory

Tend and VSM cover the same ground — scanning wave state, judging health, composing mutations, applying changes. Tend is a wide-angle lens: one scan, one assessment, human review. VSM is four specialist lenses running at their own rhythms, autonomously.

## This PR scope

Refactor tend steps + replace old sequential VSM with governance flows. Worker pools and wave mode changes are future PRs.

### Delete

- `rust/loopflow/src/engine/builtins/steps/vsm/s1.md` — old sequential step
- `rust/loopflow/src/engine/builtins/steps/vsm/s2.md` — old sequential step
- `rust/loopflow/src/engine/builtins/steps/vsm/s3.md` — old sequential step
- `rust/loopflow/src/engine/builtins/steps/vsm/s4.md` — old sequential step
- `rust/loopflow/src/engine/builtins/steps/vsm/s5.md` — old sequential step
- `rust/loopflow/src/engine/builtins/flows/vsm/vsm.yaml` — old sequential flow

### Create: tend step refactor

**`tend/play-chord.md`** — Merges current `draft-chord` + `apply-chord` into one step. Reads the assessment (`scratch/tend-assessment.md`), composes mutations from pressure points, and applies them immediately. Drafts and executes in one pass. Writes `scratch/tend-chord.md` (what was played — mutations with rationale) and modifies wave configs/items on disk. Commits with `tend: play chord`.

**Update `tend/review-chord.md`** — Changes posture from "approve before applying" to "review what already happened." Human sees what `play-chord` did and can amend or revert. No longer gates execution — it follows execution.

**Delete `tend/draft-chord.md`** — Merged into `play-chord`.

**Delete `tend/apply-chord.md`** — Merged into `play-chord`.

**Update `tend.yaml`** flow:
```yaml
- tend/scan-waves
- or:
    router: tend/assess
    paths:
      tune:
        steps:
          - tend/play-chord
          - tend/review-chord
      silence:
        description: "Everything is in tune"
```

### Create: VSM scan steps

All under `rust/loopflow/src/engine/builtins/steps/vsm/`:

**`s5-scan.md`** — Scan chord identity state.
- Reads: member wave configs (YAML), README vision/strategy/goals, roster (which waves exist), direction configs, algedonic history (which waves needed repair, escalation patterns)
- Reads: recent chord mutations — what changed since last s5 cycle
- Writes: `scratch/vsm-s5-scan.md`
- Observation only. No judgment, no mutations.

**`s4-scan.md`** — Scan environment.
- Reads: dependency state (`cargo outdated`, `uv pip list --outdated`), security advisories (GitHub dependabot alerts, `cargo audit`), upstream API changelogs, recent main branch changes that cross wave boundaries
- Reads: per-chord feed subscriptions (if configured) — what external signals this chord cares about
- Writes: `scratch/vsm-s4-scan.md`
- Looks outward, not inward. The only scan that reads beyond wave state.

**`s3-scan.md`** — Scan control/health state.
- Reads: `lfq show <wave> --json` for each member — run status, iteration count, active runs
- Reads: recent run history — velocity (items shipped per cycle), error rates, completion times
- Reads: token usage / cost data (if available from `lfq usage`)
- Reads: algedonic signals — repair chains, retry counts, escalation history
- Reads: CI status on open PRs, blocked/stalled PRs
- Writes: `scratch/vsm-s3-scan.md`

**`s2-scan.md`** — Scan coordination state.
- Reads: all member wave backlogs (item files, priority order)
- Reads: open PRs across all member waves — which files they touch
- Reads: area definitions per wave — detect overlapping areas
- Reads: recent merge conflicts or rebase failures between waves
- Writes: `scratch/vsm-s2-scan.md`

### Create: VSM assess steps

**`s5-assess.md`** — Assess identity and policy.
- Requires: `scratch/vsm-s5-scan.md`
- Produces: `scratch/vsm-s5-assessment.md`
- Questions:
  - Is this chord still responsible for the right things?
  - Does the member roster match the chord's purpose? Should any wave be created, archived, merged, or split?
  - Are autonomy levels appropriate given algedonic history?
  - Has direction drifted from stated intent?
  - Are the chord's goals still the right goals?
- Output: assessment with pressure points specific to identity/boundary concerns.

**`s4-assess.md`** — Assess intelligence / environmental changes.
- Requires: `scratch/vsm-s4-scan.md`
- Produces: `scratch/vsm-s4-assessment.md`
- Questions:
  - What environmental changes affect member waves?
  - Which findings are urgent (security advisories, breaking deps) vs informational?
  - Should any wave's backlog be updated with new items based on environmental changes?
  - What's coming that members should prepare for?
- Output: assessment with proposals — environmental findings filtered for relevance, prioritized by urgency.

**`s3-assess.md`** — Assess control / health.
- Requires: `scratch/vsm-s3-scan.md`
- Produces: `scratch/vsm-s3-assessment.md`
- Questions:
  - Are members performing? Velocity, error rates, completion trends.
  - Which waves needed repair? How often? Is the pattern improving or degrading?
  - Where should attention be allocated? What's blocked, stalled, or idle?
  - What should the s1 worker pool size be? (Resource allocation — how many concurrent workers can this chord sustain?)
  - Are any waves blocked by something mechanical (failing CI, stalled PR, config error) that s3 can fix directly?
- Output: assessment with health ratings per wave, pool size recommendation, mechanical blocks to fix.

**`s2-assess.md`** — Assess coordination.
- Requires: `scratch/vsm-s2-scan.md`
- Produces: `scratch/vsm-s2-assessment.md`
- Questions:
  - Are any members working on overlapping areas?
  - Do any open PRs conflict (touching same files)?
  - Is work oscillating (one wave undoing another's changes)?
  - What's the safe ordering for s1's backlog? Which items can run concurrently without stepping on each other?
  - Should triggers or dependencies between members change?
- Output: assessment with conflict map, recommended backlog ordering for s1, interference fixes.

### Create: VSM governance flow YAMLs

All under `rust/loopflow/src/engine/builtins/flows/vsm/`:

**`govern-identity.yaml`**:
```yaml
- vsm/s5-scan
- vsm/s5-assess
- tend/play-chord
```

**`govern-intelligence.yaml`**:
```yaml
- vsm/s4-scan
- vsm/s4-assess
- tend/play-chord
```

**`govern-control.yaml`**:
```yaml
- vsm/s3-scan
- vsm/s3-assess
- tend/play-chord
```

**`govern-coordination.yaml`**:
```yaml
- vsm/s2-scan
- vsm/s2-assess
- tend/play-chord
```

### Update: builtins.rs

Register the new steps and flows. Remove old vsm/s1-s5 registrations.

### Update: flow_tests.rs

Replace `builtin_vsm_flow_structure` test with tests for the four governance flows.

### Update: wave item

Update `wave/chord-model/02-vsm-flow.md` to reflect the new design.

### Update: README.md

Update VSM steps table and flow table.

## Future PRs (not this branch)

### Wave modes
`flow` replacing `manual`. Separate infrastructure change.

### Worker pools
`workers: u32` replacing `serialized: bool`. Dispatch changes in `activation.rs`. Default `workers: 1`. No unlimited mode.

`workers` composes with any mode:
- `flow` + `workers: 3` — triggered batch of up to 3
- `loop` + `workers: 3` — 3 persistent loopers
- `cron` + `workers: 3` — on schedule, launch up to 3

### VSM chord configs
Five member waves with independent rhythms:
```
wave/redesign/
  redesign.yaml
  wave/s5-policy/          # cron: weekly, flow: govern-identity
  wave/s4-intelligence/    # cron: daily, flow: govern-intelligence
  wave/s3-control/         # cron: every 4h, flow: govern-control
  wave/s2-coordination/    # cron: every 4h, flow: govern-coordination
  wave/s1-operations/      # mode: loop, workers: N, flow: ship-roadmap
```

### Concurrent ingest
Make `ingest` safe for multiple workers. Explore PM provider (Linear/Asana) as atomic arbiter for item claiming.

## Validation

- `cargo test --test flow_tests` — governance flow structure tests
- `uv run pytest python/tests/test_bootstrap_redesign_script.py -q`
- Full suite: `cargo fmt --check && cargo clippy -- -D warnings && cargo test --all && uv run pytest python/tests/`
