# LOO-842 plan

Built: windows 2 h / 6 h / 6 h / 2 d in `scripts/disk_cleanup.py`; cwd-only builder matches no longer hold a checkout when they descend from an agent. `Cleaner._activity` collects `Activity.agents`: pids named `codex`/`claude` (`comm`) plus `lf session serve` (`ps -o command=`); open files still hold. `scripts/disk-cleanup.md`, `.lf/skills/disk-cleanup.md` and the Disk cleanup row in `docs/dev/reference.md` match.

Remains: nothing in scope. Not yet seen on a real Machine run: whether the next hourly run frees the roots the 2026-11-11 run skipped as "builder in checkout".

Gate checks (2026-11-14; `scripts/test.py --reuse-passing` stopped at resource preflight: `~/.cache/uv` inventory exceeded its 15 s deadline, so suites ran by hand):
- `uv run pytest python/tests/test_disk_cleanup.py` — 105 passed
- `scripts/check_architecture.py` — pass; `ruff check` / `ruff format --check` on changed Python — clean
- `cargo test -p loopflow --test integration documented_commands::` — 4 passed
- `website/dev.py test` (candidate lf on PATH) — 73 passed
- Rest of `python/tests/` left to CI (the diff reaches only the disk cleanup module)

Concept review (2026-11-11): the model holds. "Builder in checkout" now means a non-agent build tool working there; an agent's own build holds through open files (cargo keeps `.cargo-lock` open). Anything descending from `lf session serve` is treated as agent-side, including a person's shell in a Loopflow terminal; that only matters for a live builder with no files open under the output, which idle retention already covers. Two `ps` reads stay: macOS `comm` is a path that can contain spaces, so it can't share a line with arguments.
