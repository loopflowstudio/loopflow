# LOO-691 questions

- Assumed (2026-10-30): the release host need not follow `dev`. The scheduled
  `release-run` uses the installed gate `lf`; a main build is only needed to
  release with an unshipped release-operation fix, via `lf install --dev`.
  Recorded in `release/README.md`. Reverse it there if Jack Heart wants the
  host pinned to `dev`.
