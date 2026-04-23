# Review: flow PM no-ops and push-diff fast path

## What was implemented

- `op: pm pull` and `op: pm push-diff` now no-op inside flows when the current branch does not resolve to a PM-enabled wave.
- `lf op pm push-diff <wave>` now checks for `wave/<name>/` changes before building the remote PM provider. If the branch never touched roadmap files, it returns a zero-result skip instead of making auth or network calls.
- README and built-in flow docs now describe the safe no-op behavior, and release notes call out the change.

## Key choices

- Treat missing PM context as "nothing to do," not as an error. Flow chains are reused by CI-only and non-PM branches; skipping keeps those branches on the happy path.
- Check `wave/<name>/` for branch-local changes before provider setup. That avoids unnecessary PM auth prompts and remote requests on branches that only changed code.
- Document the behavior where users discover it: flow docs, PM command examples, and release notes.

## How it fits together

`resolve_pm_waves_for_flow` is the flow-level gate. It now returns an empty list when the branch cannot resolve to a PM-enabled wave, and the flow executor logs a skip instead of failing. `pm_push_diff_async` adds a second gate deeper in the PM path: it diffs `wave/<name>/` against the push-diff baseline and exits early with zero counts when nothing under that directory changed.

## Risks and bottlenecks

- The new fast path still assumes `resolve_provider()` can read enough local config to report the provider in the zero-result response.
- `push-diff --all` still visits every PM-enabled wave; this change only avoids remote setup for waves with no local roadmap diff.
- No new automated coverage was added for the exact skip paths in this pass; the existing targeted tests only cover adjacent CLI and flow wiring.

## What's not included

- No new PM sync tests for the skip branches.
- No behavior changes for `pm export` or explicit `pm pull <wave>` / `pm push-diff <wave>` calls beyond the new empty-diff fast path.

## Validation

- `cargo test -p loopflow pm_pull_accepts -- --nocapture`
- `cargo test -p loopflow builtin_build_or_silent_has_xor_branch -- --nocapture`
- `cargo test -p loopflow wave_pm_is_enabled_requires_provider_project -- --nocapture`
