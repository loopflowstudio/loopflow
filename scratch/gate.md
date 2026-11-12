# LOO-855 gate (2026-11-12, HEAD 92ac6f4a3a, current with origin/main)

- `scripts/test.py --reuse-passing --rust-filter 'test(server::subscriptions::)'`: stopped at resource preflight (uv cache inventory passed its 15 s deadline); product suites didn't run, so its printed plan was run by hand.
- `scripts/check_architecture.py`, `check_rust_test_inventory.py`, `cargo fmt --all -- --check`: pass.
- `cargo nextest run -p loopflow -E 'test(server::subscriptions::)'`: 13/13 pass, 5.1 s.
- `cargo clippy --all-targets -- -D warnings`: pass.
- Whether the merge-queue flake is gone: CI owns it (`rust-test (library, 5, 5)`).
