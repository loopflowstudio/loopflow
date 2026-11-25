---
extends: builtin
---
## In this repository

Run local reproductions from the checkout root with inherited `LF_*` unset and
`< /dev/null` — inline in zsh:
`env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') <cmd> < /dev/null`.

| Check | Local reproduction or repair |
|---|---|
| `required-integration` | Synthetic `lf` check (no GitHub log). Run `lf sync`, verify clean/tests, then `lf arm`. |
| `scratch-clear` | Merge-queue/main rejects files under `scratch/` other than `.gitkeep` (a `lf sync` conflict resolution can restore notes). Run `lf scratch clear`, commit if needed, then `lf arm`. |
| `architecture-check` | `git fetch --no-tags origin main:refs/remotes/origin/main && uv run --managed-python python scripts/check_architecture.py` (needs managed Python's SQLite ≥ 3.47). |
| `rust-lint` | `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings`. CI runs rustup `stable`; if CI fails after a local pass, prepend `$HOME/.cargo/bin` to `PATH` (`rust/loopflow/tests/README.md`). Linux-only `cfg(target_os = "linux")` lint runs only in CI — read the log. |
| `migration-check` | `git fetch --no-tags origin main:refs/remotes/origin/main`, then `LOOPFLOW_REQUIRE_ORIGIN_MAIN=1 UV_PYTHON_PREFERENCE=only-managed uv run python scripts/check_migrations.py` and `UV_PYTHON_PREFERENCE=only-managed uv run python scripts/schema_page.py --check`. |
| `macos / swift-test` | `scripts/test_desktop.sh --filter '<failed suite>'` (builds `lf` unless `--skip-build`), `uv run pytest python/tests/test_desktop_performance.py::test_checkout_observation_preserves_read_boundary_and_detects_changes`, `uv run python scripts/check_desktop_launch.py`, `uv run python scripts/check_swift_multiplatform_boundaries.py`. |
| `macos / loopflow-ui-test` | Desktop Xcode compile: `(cd swift && xcodegen generate) && uv run python scripts/build_xcode_ci.py`. iOS Simulator tests: `uv run python scripts/test_ios.py` (pass `-only-testing:LoopflowiOSTests/<Suite>` to focus). |
| `macos / macos-process-test` | `uv run python scripts/test_network.py cargo test --offline -p loopflow --lib <app_process_tests \| journal::os_process:: \| server::mobile::tailscale::macos::>`. |
| `task-installation` | `uv run python scripts/test_task_installation.py` (CI passes `--container` and `--repository-upgrade`; see `scripts/README.md` for local usage). |
| `e2e-smoke` | Two steps: `planning_reconnect_tests` (Linux only — read CI log on macOS), then `cargo build -p loopflow --bins` and `uv run python scripts/test_network.py tests/e2e/test_smoke.sh`. |

When reproducing a failing Rust or macOS test locally hits `syspolicyd`,
`TMPDIR`, `fseventsd`, `has no resident login`, `No agent selected`, or needs
nextest `--rust-filter` / path-to-test traps, read `scripts/testing.md`.
