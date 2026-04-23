# Flows Reorg: 6+7 categories → 3

Collapse the current `flows/{algedonic,build,code,garden,ops,vsm}` + `steps/{code,garden,interactive,ops,plan,vsm,wave}` split into a single taxonomy organized by **agency**.

## Target layout

```
rust/loopflow/src/engine/builtins/
  build/         # manual: human-driven work
    flow/        # 9 flows
    step/        # 23 steps
  govern/        # auto: system-driven coordination
    flow/        # 8 flows
    step/        # 14 steps
  ops/           # side-channel utilities
    flow/        # 2 flows
    step/        # 12 steps
```

Full placement captured at the end.

## Why

- **Build vs code** isn't altitude-separate; code is build's inner loop. Merging kills 5 of 7 cross-category step shares.
- **Garden vs vsm** isn't parallel; vsm governance *actuates via* garden (`wave/mutate` at the tail of every `govern-*`). Merging kills the remaining garden/vsm share.
- **Algedonic** has one flow and operates like build work — folds in naturally.
- **Interactive, plan, wave** as standalone step categories don't earn top-level status; each distributes into its parent category.

Result: two meaningful categories (manual/auto) + one utility bucket, vs 13 partially-overlapping labels today.

## Qualified step names

Currently: `garden/scan`, `wave/mutate`, `vsm/s2-assess` — prefix is the step-category directory. With the reorg, those prefixes don't match new homes.

**Decision: drop the prefix.** `garden/scan` → `scan`, `wave/mutate` → `mutate`, etc. Verified no name collisions across the 49 steps. Clean names, one source of truth.

Breaks `.lf/steps/garden/*` style overrides. Acceptable — no external users rely on stable names, and this is the simplification the whole reorg exists for.

## Work

1. **Scan for hardcoded references.** `rust/loopflow/build.rs` generates categories from directory structure; `src/engine/builtins.rs` hardcodes `include_str!` paths; `src/lf/discovery.rs` has `BUILTIN_CATEGORIES` + `builtin_descriptions()` keyed on qualified names.
2. **Move files.** 60+ physical moves per target layout above.
3. **Update engine.** `build.rs` category generation, `builtins.rs` `include_str!` paths, `discovery.rs` category list + descriptions.
4. **Rewrite flow YAMLs.** Any `- garden/scan`, `- wave/mutate`, `- vsm/s2-*` references become bare names.
5. **Update tests.** `flow_tests.rs` asserts on qualified names — rewrite.
6. **README.** Category tables collapse to build/govern/ops. Steps no longer listed under plan/interactive/wave/scan/garden/vsm separately.
7. **Verify.** `cargo test`, `uv run pytest python/tests/`, e2e smoke.

Estimate: ~60 file moves, ~6 Rust files modified, ~15 flow YAMLs updated, ~10 test assertions updated, README rewrite.

## Out of scope

- Concerto Flows view (next PR, consumes the new layout via `lfd /flows`)
- Splitting `wave/mutate` into vsm-focused + garden-focused versions — no longer needed once categories merge
- `ops/` subdivision (git/release/wave) — defer; 12 items flat is readable
- `ingest` cross-reference (build-or-silent uses it, home is govern) — live with the one cross-cut

## Placement (full tree)

**build/flow/** (9):
build, build-or-silent, code, deploy, design-and-ship, incident, pair, queue, ship

**build/step/** (23):
kickoff, research, iterate, refresh-plan, reduce, polish, expand, 5whys *(from plan/)*;
implement, compress, gate, debug, ci-fix, integrate-upstream, qa, triage *(from code/)*;
design, explore, demo, code-review, review-design, refine, review-open-work *(from interactive/)*

**govern/flow/** (8):
garden, garden-act, govern-operations, govern-coordination, govern-control, govern-intelligence, govern-identity, s1-build

**govern/step/** (14):
scan, assess, wave-report *(from garden/)*;
mutate, review *(from wave/)*;
s2-scan, s2-assess, s3-scan, s3-assess, s4-scan, s4-assess, s5-scan, s5-assess *(from vsm/)*;
ingest *(from plan/)*

**ops/flow/** (2):
release, sync

**ops/step/** (12):
commit, init, land, lint, pr, rebase, release, release-notes, update-wave, split-wave, synthesize, validate

### Placement adjustments from use

- **s1-build → govern/flow/.** No `kickoff`/design ceremony; it's the "just build" path that `govern-operations` triggers autonomously. Belongs with the autonomous family.
- **sync → ops/flow/.** `rebase → integrate-upstream` is git-hygiene plumbing, not build work. Side-channel utility.
