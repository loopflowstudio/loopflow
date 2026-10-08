# Prune superseded install artifacts (LOO-390, PR 2)

Jack Heart authorized autonomous delivery. Design and measurements:
`docs/reviews/storage-footprint.md#install-artifact-retention--2026-10-07`.

Remaining: observe reclamation after a release containing this change installs
(expected 161 binaries and 62 bundles here; measure `df` and `du` before/after).
Linux `/proc` process scan is compiled only in CI.

Check: `cargo test -p loopflow --lib artifact_tests` 5 passed; `cargo clippy -p loopflow --all-targets -- -D warnings` clean.
