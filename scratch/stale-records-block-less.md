# Stale PR bases

Gate repaired the Linux review-opener fixture after syncing main `092ab7ba5b`.
The 5 Whys, inventory and CI failure analysis are preserved in
`docs/reviews/stale-pr-bases.md`. Remaining: hosted Linux verification and delivery.

Checks: `uv run python scripts/test_network.py cargo test -p loopflow --test task_pr_range_tests --test task_pr_authority_tests --test sync_tests --test land_tests` (inherited LF_* cleared, TMPDIR=/private/tmp, closed stdin): 86 passed; `uv run python scripts/demo_pr_base.py`: five scenarios passed; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `uv run ruff check scripts/demo_pr_base.py`, `uv run ruff format --check scripts/demo_pr_base.py`, `uv run python scripts/check_architecture.py`: passed; repaired Linux fixture/full materialized matrix deferred to CI.
