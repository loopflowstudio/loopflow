## Try it!

```bash
uv run python scripts/install.py refresh --no-pull
uv run python scripts/install.py local --use
```

Both active install paths now run:

```bash
lf op sync-skills --global --yes
```

after installing or promoting `lf`, so generated loopflow skills are refreshed under `~/.claude/skills` and `~/.agents/skills`.

Validation run:

```bash
uv run ruff check scripts/install.py python/tests/test_install_script.py
uv run pytest python/tests/test_install_script.py -v
uv run pytest python/tests/
```

## Intent

Keep local `lf` installs and vendor skill catalogs in lockstep. After a local install, Claude and Codex should see the same freshly installed loopflow steps without a separate manual `lf op sync-skills --global --yes`.

## Assumptions

- Global skill sync is desirable after an active install, but stale skills should not make the binary install fail.
- `refresh` remains the fast CLI-only path; `local --use` remains the full promote path.
- `lf op sync-skills --global --yes` is the source of truth for writing both Claude and Codex skills.

## Key decisions

- Run sync after installing or promoting binaries so the just-installed `lf` performs the generation.
- Warn and continue on sync exit failures and launch errors. The binaries are already in place, and the user can rerun sync manually.
- Cover the helper with installer tests using a fake executable to verify the real argv and the non-fatal failure contract.

## Not included

- No retry behavior for failed skill sync.
- No app install changes in `refresh`.
- No cleanup of stale wave roadmap mirror files.
