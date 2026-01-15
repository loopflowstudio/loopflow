# scripts/publish.py

Publish loopflow to PyPI from main branch.

## Files

- `scripts/publish.py` — CLI entrypoint for publishing
- `src/loopflow/publish.py` — publishing utilities (version handling, build, test)
- `src/loopflow/llm_http.py` — added `generate_release_notes()` and `ReleaseNotes` model
- `src/loopflow/builtins/release_notes.txt` — prompt for generating release notes
- `Maestro/dev` — moved from repo root (Swift-only now)

## Usage

```bash
./scripts/publish.py                    # patch bump (default)
./scripts/publish.py minor              # minor bump
./scripts/publish.py --dry-run          # preview without executing
./scripts/publish.py --force            # bypass main branch check
./scripts/publish.py --skip-tests       # skip test run
```

## Workflow

1. Preflight: check on main, synced with origin, no uncommitted changes
2. Run tests (unless `--skip-tests`)
3. Generate release notes via LLM from commits since last tag
4. Bump version in `__init__.py`, validate with build
5. Write RELEASE_NOTES.md, commit, push
6. Tag and push tag
7. Publish to PyPI
8. Install locally via `uv tool install`
