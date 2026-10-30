# LOO-691 questions

- Assumed (2026-10-30): the release host need not follow `dev`. The scheduled
  `release-run` uses the installed gate `lf`; a main build is only needed to
  release with an unshipped release-operation fix, via `lf install --dev`.
  Recorded in `release/README.md`. Reverse it there if Jack Heart wants the
  host pinned to `dev`.

## Status (2026-10-30)

Done: release fetches an unseen release merge (`release_version`) and any
missing build revision (`materialize_exact_source_worktree`); fetch failures
now surface. Remains: gate/CI, then installed acceptance on the next real
release after a merge lands on origin only.

Check: `cargo test -p loopflow --test release_tests release_run_fetches_a_release_merge` — pass.
