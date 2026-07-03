# Gate review: install syncs skills

## What was implemented

`scripts/install.py` now syncs loopflow-generated Claude and Codex skills after both active install paths:

- `refresh` rebuilds and installs `lf`/`lfd`, then runs the freshly installed `lf op sync-skills --global --yes`.
- `local --use` promotes the worktree build, then runs the promoted `lf op sync-skills --global --yes`.

The README documents the behavior, and release wave memory records it as shipped.

Gate added one hardening pass: sync launch failures are now non-fatal too, matching the helper's documented contract that binaries stay installed even when skill sync cannot complete.

## Key choices

- Skill sync runs after binary install/promotion so the generated skill catalog comes from the version the user just installed.
- Sync failure warns and continues. A stale skill catalog is annoying; a failed install after binaries are already in place is worse.
- `refresh` stays CLI-only. It does not build or install Loopflow.app.
- Tests use a tiny fake `lf` executable to verify the actual argv, instead of asserting mock wiring.

## How it fits together

`refresh` and `local --use` both converge on `_sync_skills(lf_bin)`. The helper streams `lf op sync-skills --global --yes` output with the `skills` label, warns on nonzero exit or launch errors, and returns without raising.

## Risks and bottlenecks

- Global skill sync writes under `~/.claude/skills` and `~/.agents/skills`; permission or filesystem failures will leave skills stale.
- The sync runs during install, so it adds a small amount of latency to the active install paths.
- If `lf op sync-skills` semantics change, this installer path should be updated with it.

## What's not included

- No retry loop for global skill sync.
- No telemetry or structured install report for skill sync failure.
- No cleanup of stale local roadmap mirror files; release memory records that as broader wave hygiene work.

## Validation

```bash
uv run ruff check scripts/install.py python/tests/test_install_script.py
uv run pytest python/tests/test_install_script.py -v
uv run pytest python/tests/
```

Results:

- Ruff: passed.
- Installer tests: 9 passed.
- Python suite: 146 passed.
