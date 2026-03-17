# 02: Tend Flow Steps

**Finish line:** A registered `redesign` wave completes a real `lf tend` cycle against live member-wave state. The run writes scan and routing artifacts from lfd-backed data, chooses a real path (`chord`, `reorg`, or `silence`), and leaves a reviewer-visible recipe for reproducing the cycle.

## Context

The structural wiring shipped on this branch:
- `tend` is now `scan-waves -> or(router: tend/assess)` with `chord`, `reorg`, and `silence` paths
- `tend-chord` expands to `draft-chord -> review-chord -> apply-chord`
- `scan-waves` reads live lfd state through `lfq show <wave> --json`
- Rust flow tests cover tend structure and ops inside `or` sub-flows

What's still missing is the live proof. The redesign/member wave directories exist on disk, but this worktree has not yet started lfd, registered those waves, and exercised the first real tend cycle. Until that happens, the flow is structurally executable but not operationally trusted.

## What to build

1. **Boot the redesign chord in lfd.** Start lfd, run `scripts/bootstrap-redesign.py`, and confirm `lfq show redesign --json` plus each member wave returns live state instead of "not found".

2. **Run the first real tend cycle.** Execute `lf tend` for `redesign` in the repo-local worktree. Keep the artifacts: `scratch/tend-scan.md`, the router output, and the resulting path execution.

3. **Exercise a real routed path.** Expect the first run to choose `reorg` or `silence` while the chord is quiet. If that happens, create one small, reversible pressure point and rerun so the `chord` path also gets a live exercise.

4. **Capture the operating recipe.** Leave one reviewer-friendly script or command sequence showing how to start lfd, bootstrap the waves, run tend, and inspect the resulting artifacts.

5. **Keep the rename separate.** `lf ops` → `lf op` is still a worthwhile cleanup, but it is not part of proving tend live. Do it after the first real cycle exists.

## Done when

- `scripts/bootstrap-redesign.py` registers `redesign` and its member waves in a running lfd
- `lfq show <wave> --json` returns live runtime state for the redesign chord and each member wave
- A real `lf tend` run against `redesign` completes and writes lfd-backed scan output plus router artifacts
- At least one routed path completes in a live run, and the branch documents how to reproduce it
- If the first live run routes to `reorg` or `silence`, a follow-up live run exercises the `chord` path or explicitly documents why that path still cannot run
