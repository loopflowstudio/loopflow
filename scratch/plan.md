# LOO-842 plan

Built: windows 2 h / 6 h / 6 h / 2 d in `scripts/disk_cleanup.py`; cwd-only builder matches no longer hold a checkout when they descend from an agent. `Cleaner._activity` collects `Activity.agents`: pids named `codex`/`claude` (`comm`) plus `lf session serve` (`ps -o command=`); open files still hold. `scripts/disk-cleanup.md`, `.lf/skills/disk-cleanup.md` and the Disk cleanup row in `docs/dev/reference.md` match.

Remains: nothing in scope. Not yet seen on a real Machine run: whether the next hourly run frees the roots the 2026-11-11 run skipped as "builder in checkout".

Check: `uv run pytest python/tests/test_disk_cleanup.py` — 105 passed (after compress). Gate runs the Python suite again.
