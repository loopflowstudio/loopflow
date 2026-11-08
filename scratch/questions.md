# PR #1813 integration repair

2026-11-07: GitHub reported all published checks green at
`be8280bd1ec4b3f5e35417ba908a85dbe2f01f66`, but the PR was `CONFLICTING` /
`DIRTY`. Loopflow's `required-integration` names that condition. Continued the
existing sync onto `90a82635fc94993ea95f42702905929e7d1acf4a` locally.

The account contract follows main's next-launch reports, Machine holds/release
and pause/unpause. Token/isolation operations stay removed; direct served
execution and caller directories stay intact.

Remaining: commit and publish/arm this same PR.

Checks:
- Rust account/provider/server/served-read/CLI tests: initial merge failures repaired; schema, wire, help and account-state rerun passed (10/10); the broader runs passed their remaining cases.
- Native-limit account fixture: passed from an empty cwd with a temporary HOME; the checkout-scoped run selected a personal route without its fixture login.
- Swift headless protocol/registry/DTO suites: 88/88 passed; protocol rerun after final schema generation: 15/15 passed.
- `cargo fmt --check`: passed; final `cargo clippy --all-targets -- -D warnings`: passed.
- `uv run python scripts/check_architecture.py`: passed.
- `uv run python scripts/check_migrations.py`: passed.
