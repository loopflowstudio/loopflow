## Try it!

```bash
cargo test -p loopflow pm_pull_accepts -- --nocapture
cargo test -p loopflow builtin_build_or_silent_has_xor_branch -- --nocapture
cargo test -p loopflow wave_pm_is_enabled_requires_provider_project -- --nocapture
```

Run a flow-backed branch with no PM wave or no `wave/<name>/` edits. `deploy` / `sync` now log a clean skip instead of failing or forcing provider auth.

## Intent

Make PM flow ops reusable across more branches. `deploy` and `sync` should work on PM-backed branches, CI-only branches, and plain code branches without extra conditionals. When a branch never touched roadmap files, `pm push-diff` should stay local and return a zero-result no-op.

## Assumptions

- A branch without a resolvable PM-enabled wave should be treated as "nothing to do," not operator error.
- Local wave config still exists when `pm push-diff` needs to report provider/project metadata for a zero-result skip.
- Reviewers are comfortable with documentation describing behavior that is intentionally best-effort/no-op in some branch contexts.

## Key decisions

- Resolve PM waves in flow execution and return an empty list for non-PM contexts.
- Short-circuit `pm_push_diff` before provider construction when `wave/<name>/` has no branch-local diff.
- Update README, built-in flow docs, and release notes together so the new behavior is visible from the CLI docs and the release summary.

## Not included

- New test coverage for the exact no-op branches.
- Any change to `pm export` behavior.
- Any change to `push-diff --all` beyond the per-wave empty-diff fast path.
