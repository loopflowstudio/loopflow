# CI repair — 2026-10-11

Failed head: `662bba5e98284ec653f8e42fdd333df9a1f67afd`, PR #1526,
CI run 38115565667. Sync produced `b68e4c01b2cc2b195e648b42dc5ec1fd8dfd4663`.

All 2,447 Rust tests passed. Three trailing Python CLI proofs still expected
`wt prune` to fetch and merge main. Reproduced those exact three failures against
the synced CLI. Keep the local-only collector; replace obsolete assertions with
six apply/preview cases preserving refs, unpublished commits, index, working edits
and notes with advanced, conflicting or unavailable remotes. Create/list refresh
coverage remains. TESTING.md now includes pruning in this CLI check boundary.

The installation job failed before tests at the ECR image pull with
`toomanyrequests: Rate exceeded`. The same image pulls successfully locally;
no test retries or speculative registry changes added. Fresh-head CI must confirm
provider recovery; local pull success does not prove the GitHub runner is unthrottled.

Review: no production behavior changes or weakened deletion authority. Assertions
cover observable state, not Git mock calls. Six local-only cases replace three
obsolete refresh expectations; remaining refresh tests are retained.

Checks: `cargo build -p loopflow --bin lf --jobs 4`; `LOOPFLOW_TEST_LF="$PWD/target/debug/lf" uv run --no-sync python scripts/test_network.py uv run --no-sync pytest python/tests/test_checkout_refresh.py` — PASS (22); focused synced journal test — PASS (1); `uv run --no-sync ruff check` and `ruff format --check python/tests/test_checkout_refresh.py`, `git diff --check` — PASS; ECR image pull — PASS; fresh-head CI pending.
