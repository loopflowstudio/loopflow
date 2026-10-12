# Stale PR bases — lf-cb9012a

The implemented design, 5 Whys, refusal inventory and preservation evidence
live in [the durable review](../docs/reviews/stale-pr-bases.md).

Jack Heart initially requested publication for review without landing. His later
October 11 comment `7689d642-38dc-4991-9b0c-c0d96c343403` authorizes landing
PR #1533 after resolving main's conflicts in place, with the investigation
preserved before scratch is cleared. This gate reviews the current branch;
delivery and its required integration remain subsequent work.

## Remaining

- Delivery must integrate current main and resolve the reported #1491 conflict
  in this checkout before landing. Current HEAD still includes only main through
  `0e9ac044ef`.
- Hosted CI owns the full matrix. Installed Etude recovery remains unproved;
  that checkout was untouched.
- Source-only refusal paths in the inventory are findings, not reproduced repairs.
- Broader Python verification still has four pruning failures from the installed
  registry's nullable worktree reader and one nested macOS sandbox failure; see
  the durable review. Isolated CI must cover these remaining checks.

## Gate review

Current ancestry resolves publication without dropping branch dependencies.
Stacked publication retains sync's comparison fork; its regression proves that
later sync preserves main's deletion and child edits. Empty ranges, unrelated
history, managed ownership and post-push recording retain their protections.
No additional production-code change was needed in this review.

Checks: with inherited `LF_*` cleared and `TMPDIR=/private/tmp`, `uv run python scripts/test_network.py cargo test -p loopflow --test task_pr_range_tests --test task_pr_authority_tests --test sync_tests --test pr_tests --test land_tests` passed 116 tests; `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `uv run ruff check scripts/demo_pr_base.py`, `uv run ruff format --check scripts/demo_pr_base.py`, `uv run python scripts/check_architecture.py` and `git diff --check` passed; network-isolated `uv run pytest python/tests/ -q` had 409 passes and five failures described above; full materialized matrix remains with CI.
