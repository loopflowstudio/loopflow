`cargo test -p loopflow --test one_machine_tests flow_steps_keep_the_explicit_home_despite_stale_pins_and_path -- --exact` — passed (1 test); broader verification remains with gate/CI.

`uv run python scripts/check_architecture.py`, `uv run pytest python/tests/test_architecture.py -q` (21), `scripts/test_desktop.sh -Xswiftc -gnone --filter PortfolioRepoStateTests` (4), `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, website `uv run python dev.py test -k "portable_architecture or readme_index_sync"` (2) — passed; full CI remains pending on the repaired head.
