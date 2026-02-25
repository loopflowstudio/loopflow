# Direction Taxonomy Restructuring

## Status

Implementation pass needed. Review complete — taxonomy finalized, code changes in progress on branch.

## Goal

Replace role-style directions with composable quality-focused direction groups, and make `-d <group>` work consistently across prompt context gathering and fork execution.

## Final taxonomy

```
rust/loopflow/src/engine/builtins/directions/
  infra/
    security.md
    performance.md
    reliability.md
    observability.md
  ux/
    visibility.md
    feedback.md
    consistency.md
    affordance.md
    error-prevention.md
    accessibility.md
    dynamics.md
    aesthetics.md
  craft/
    care.md
    clarity.md
    scale.md
    simplicity.md
  creativity/
    alive.md
    musical.md
  ceo/
    focus.md
    immediacy.md
    truth.md
```

## What changed from the original branch

The original branch shipped `values/` as a flat group and `ceo.md` as a standalone file. Review restructured both:

1. **`ceo.md` → `ceo/` group** — decomposed into three orthogonal voices:
   - `immediacy.md` — speed, bias to action, decide don't deliberate
   - `focus.md` — kill things, stop what isn't working, errors of omission
   - `truth.md` — contrarian truth, 10x thinking, raise the ceiling

2. **`values/` → `craft/` + `creativity/`** — the old values group mixed two concerns:
   - `craft/` — building things right: care (renamed from craft.md), clarity, simplicity, scale
   - `creativity/` — momentum and feel: alive (fleshed out from thin flow.md), musical (new)

## What needs to happen

The code changes are already applied on the branch. The implementation pass should:

1. Verify all tests pass: `cargo fmt`, `cargo clippy`, `cargo test -p loopflow --test flow_tests --test context_tests --test discovery_tests --test golden_prompt`, `cargo test -p loopflow -- engine::flow::tests`, `uv run pytest python/tests/ -q`
2. Regenerate golden prompt if needed (the `with_direction_group` golden now uses `craft` instead of `values`)
3. Commit the taxonomy changes as a single atomic commit on top of the existing branch

## Key decisions

- All groups expand before loading concrete direction files — downstream logic unchanged.
- User groups (`.lf/directions/<group>/`) take precedence over builtin groups.
- No compatibility aliases for removed names (`infra-engineer`, `designer`, `product-engineer`, `values`).
- Short names for CLI users; Concerto can use full paths for disambiguation.
- Fork flows (`wave-reduce` etc.) are demos — the `infra`/`ux`/`ceo` fork split has no deep justification.

## Files changed (relative to original branch)

### Created
- `directions/ceo/immediacy.md`
- `directions/ceo/focus.md`
- `directions/ceo/truth.md`
- `directions/craft/care.md`
- `directions/craft/clarity.md`
- `directions/craft/simplicity.md`
- `directions/craft/scale.md`
- `directions/creativity/alive.md`
- `directions/creativity/musical.md`

### Deleted
- `directions/ceo.md` (decomposed into group)
- `directions/values/` (entire directory — replaced by craft/ and creativity/)

### Modified
- `rust/loopflow/src/engine/flow.rs` — updated tests for new groups
- `tests/goldens/with_direction_group.yaml` — `values` → `craft`
- `tests/goldens/with_direction_group.md` — regenerated
- `wave/infra/00-architecture-report.md` — updated group names
- `wave/infra/README.md` — updated group names
