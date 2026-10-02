# Install recovery

Jack Heart reported `lf install` failing after download with “Task PR authority refused” and an installation/store mismatch. The subsequent `lf doctor` ran the old release, reporting missing scheduled receipts and a stale binary.

The candidate reaches `install promote` in the original Git checkout. Its checkout guard resolves Task PR authority through an ordinary store open before installation compatibility checks. That store open can reject the candidate precisely because the installed runtime/schema is different. The same dependency exists in switch recovery.

Repair: remove Task checkout gating from machine installation, including promotion, rollback and recovery. Retain the existing published-artifact, migration, lifecycle, switch ownership and promotion-lock checks. Installation can run from the checkout where it was requested.

Checks: `cargo test -p loopflow --test global_commands installation_` (2 passed), `cargo test -p loopflow --test local_promotion` (2 passed), `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `uv run --project website --extra test pytest website/tests/test_readme_index_sync.py`, and `git diff --check` passed. Live published installation still needs replay with a released fix; a source build cannot be promoted. Missing cron receipts are independent and are not repaired by changing installation authority.

Review finding resolved: removing only the download-path guard would leave promotion and interrupted-switch recovery with the same dependency. Removed all installation callers and the now-unused Task helper; retained candidate and switch authority checks. No schema changes or production-data edits.
