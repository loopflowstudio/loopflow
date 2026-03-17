# Open questions

- The design doc's suggested verification command (`grep -r 'solo\|team\b\|ci\b' docs/ deploy/ docker/`) overmatches unrelated CI references in general docs and even binary screenshot assets. I validated the deployment-facing files directly (`docs/lfd.md`, `docs/getting-started.md`, `deploy/README.md`, `docker/docker-compose.yml`, `deploy/docker-compose.prod.yml`) instead.
- `cargo test -p loopflow` still fails after the compress pass, but in unrelated `rust/loopflow/tests/land_tests.rs` cases (`land_missing_pr_error_includes_branch_name`, `land_generates_copy_when_cached_pr_copy_is_stale`). The config integration tests are now HOME-isolated, so local `~/.lf/config.yaml` no longer contaminates them.
