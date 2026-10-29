# Demo: LOO-669 slice 2, streams as published pointers (2026-10-29)

Headless demo, with the agent standing in for Jack Heart. There was no live
release: none has run since #1755, and running one decides the open v0.16.0 timing question.

## Observed

- **Controller, real `release_run` path.** `cargo test -p loopflow --test release_tests daily_run_moves_stable`: 1 passed.
  Fixture: Latest = v0.9.1 (20d), v0.9.2 (8d), v0.9.3 (1d), with no new commits. Result: `NoChanges`, and
  the effects log reads `publisher promote --tag v0.9.2` followed by `gh release edit v0.9.2 --latest`.
  Stable moved to the newest soaked release and skipped the 1-day-old one. The publisher ran before the marker.
- **Publisher.** `pytest python/tests/test_release_publisher.py python/tests/test_release_automation.py`: 55 passed,
  including promote success, promote refusing a DMG that differs from `SHA256SUMS`, preview proof
  leaving the stable DMG alone, and targets disagreeing on `reader-catalog.txt` being refused.
- **Live releases (read-only, `gh release list`).** Latest is v0.15.0 (published 2026-10-28). The newest
  release at least 7 days old is v0.14.14. `stable_advance` only moves forward, so stable stays on v0.15.0 and
  doesn't roll back. This matches the "First stable move" assumption in `scratch/questions.md`.

## Decision (stand-in)

Advance. Slice 2 does what `scratch/release-streams.md` Build order item 2 describes, and
the demo raised no product question. Still open, carried by the PR description (#1772): whether v0.16.0
releases before or after slice 2 merges. PR CI must still run nextest/clippy, which the local disk reserve refused (5.5 GiB free).
