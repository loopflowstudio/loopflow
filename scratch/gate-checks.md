# LOO-858 gate checks (2026-11-13)

- `scripts/test.py --reuse-passing --rust-filter '…'`: not run. Resource preflight FAIL, 9.1 GiB free against the 32 GiB reserve; `resource_envelope.py --recover` freed nothing (7.3 GiB after). Rust suites, fmt check and clippy go to CI.
- `uv run python scripts/check_migrations.py`: pass after making `desktop_shells_terminal` partial (`WHERE terminal_id IS NOT NULL`). CI's migration-check had failed on the unique index.
- CI on 08a7e945 → merge of main: rust-lint, python-test, website-test, architecture-check passed before the migration fix; full matrix reruns on the pushed head.
- Merge of origin/main (#1920): main's `grace_expiry_refuses_bytes_before_repainting_the_terminal_screen` unit test in `transport/hosted.rs` was dropped; it exercised the resume/grace protocol this branch deletes.
