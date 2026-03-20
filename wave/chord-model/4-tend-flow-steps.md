---
asana_id: '1213718081081138'
linear_id: a6cfa97d-f4b0-4c65-8ba9-3c7da42d0cba
---
# 02: Tend Flow Steps

**Finish line:** A registered `redesign` wave completes a real `lf tend` cycle against live member-wave state. The run writes scan and routing artifacts from lfd-backed data, chooses a real path (`tune` or `silence`), and leaves a reviewer-visible recipe for reproducing the cycle.

## Context

The structural wiring is in place now:
- `tend` expands as `scan-waves -> or(router: tend/assess)` with `tune` and `silence` paths
- `tend-tune` expands to `draft-chord -> review-chord -> apply-chord`
- `scan-waves` reads live lfd state through `lfq show <wave> --json`
- `WaveDto` now exposes the runtime fields `scan-waves` needs today: `flow_steps`, `triggers`, `open_pr_count`, `stack_count`, and optional `active_run` PR / queue state
- `ship-roadmap` still allows ops inside an `or` sub-flow, so tend can branch without giving up mechanical follow-through
- `scripts/bootstrap-redesign.py` registers `redesign` plus its member waves through the ordinary waves API
- Python, docs, and lfd HTTP routes no longer expose standalone chord CRUD
- Rust flow tests cover tend structure and ops inside `or` sub-flows

What is still missing is the live proof. The redesign/member wave directories exist on disk, but this worktree has not yet started lfd, registered those waves, and exercised the first real tend cycle. Until that happens, the flow is structurally executable but not operationally trusted. No other redesign item owns that operational gap — this item does.

This item is the active Phase 1 proving slice. Later algedonic polish — stall detection, richer self-healing heuristics, and memory-backed pattern recall — can follow once the live tend cycle exists, but they are not prerequisites for closing this gap.

## Validation baseline

Use the current codebase to re-check the structural slice before attempting the live run:

- `cargo test -p loopflow --test flow_tests`
- `cargo test -p loopflow lf::commands::flow::tests`
- `cargo test --all`
- `uv run pytest python/tests/`
- Inspect `rust/loopflow/src/engine/builtins/flows/tend/tend.yaml`, `rust/loopflow/src/engine/builtins/flows/tend/tend-tune.yaml`, `lfq show <wave> --json`, and the built-in tend docs under `rust/loopflow/src/engine/builtins/steps/tend/`

Expected results:
- `tend` parses as `scan-waves -> or(router: tend/assess)` with `tune` and `silence` paths
- `ship-roadmap` keeps working with ops inside an `or` sub-flow
- Python and docs no longer expose standalone chord CRUD
- `scan-waves.md` reads lfd state via `lfq show --json` and emits a runtime section

That baseline is the setup for the real remaining proof: boot lfd, register the redesign waves, and run `lf tend` against live state.

## What to build

1. **Boot the redesign chord in lfd.** Start lfd, run `scripts/bootstrap-redesign.py`, and confirm `lfq show redesign --json` plus each member wave returns live state instead of "not found".

2. **Run the first real tend cycle.** Execute `lf tend` for `redesign` in the repo-local worktree. Keep the artifacts: `scratch/tend-scan.md`, the router output, and the resulting path execution.

3. **Exercise a real routed path.** Expect the first run to choose `silence` while the chord is quiet. If that happens, create one small, reversible pressure point and rerun so the `tune` path also gets a live exercise.

4. **Close the runtime gaps the demo exposed.**
   - isolate dev lfd auth state from any other local daemon (`LF_HOME` or equivalent)
   - make sure PR state is visible to the run snapshot so CI polling sees the right targets
   - keep the tend demo from depending on hand-edited token files or ad hoc setup

5. **Capture the operating recipe.** Leave one reviewer-friendly script or command sequence showing how to start lfd, bootstrap the waves, run tend, and inspect the resulting artifacts.

6. **Keep the rename separate.** `lf ops` → `lf op` is still a worthwhile cleanup, but it is not part of proving tend live. Do it after the first real cycle exists.

## Done when

- `scripts/bootstrap-redesign.py` registers `redesign` and its member waves in a running lfd
- `lfq show <wave> --json` returns live runtime state for the redesign chord and each member wave
- A real `lf tend` run against `redesign` completes and writes lfd-backed scan output plus router artifacts
- At least one routed path completes in a live run, and the branch documents how to reproduce it
- If the first live run routes to `silence`, a follow-up live run exercises the `tune` path or explicitly documents why that path still cannot run
