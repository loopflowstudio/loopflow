# Gate review: architecture Linear roadmap access

## What was implemented

Fixed the deployment path that left `lf op pm show --wave architecture` running
an older app-bundle `lf`. The branch makes `scripts/install.py local --use`
promote the rebuilt `Loopflow.app` into the applications directory that contains
the currently active `lf`, and removes the obsolete global `uv tool install`
wheel step that could fail before promotion.

The PM provider selection code was left unchanged. HEAD already honors
`pm.provider: linear`; the observed `asana_project` error came from `lf 0.9.12`
shadowing the fresh `~/.local/bin/lf` on PATH.

## Key choices

- Follow the active app bundle when bare `lf` resolves to
  `Loopflow.app/Contents/MacOS/lf`, instead of always copying to `/Applications`.
- Keep wheel installation local to the current uv environment; the Python wheel
  has no console entrypoint to install as a global uv tool.
- Avoid a staleness guard in `lfd` for this branch. That would be new behavior
  and belongs behind a design review, not inside the roadmap unblock.

## How it fits together

`local --use` now resolves two destinations before building: the local binary
directory and the applications directory. `_resolve_applications_dir()` checks
the active `lf` path; `_promote()` receives that directory explicitly and copies
the freshly built `Loopflow.app` there. The active bundle and PATH symlinks move
together, so the wave shell no longer keeps executing an old bundled binary.

## Risks and bottlenecks

- The active-app detection intentionally only matches the standard macOS bundle
  shape. Non-bundle `lf` installs still fall back to `/Applications`.
- `lf op pm update` writes may still require Linear team configuration. Reads
  are verified and were the reported blocker.
- The branch does not restart the live `lfd` service unless `local --use
  --service` is run.

## What's not included

- No changes to `ops::pm::resolve_provider` or Linear provider routing.
- No lfd hard-cut route-contract work.
- No stale-binary warning or refusal inside `lfd`; tracked only as a follow-up
  sketch in `scratch/pm-linear-roadmap-access.md`.

## Validation

```
uv run pytest python/tests/test_install_script.py -q
# 12 passed

uv run python scripts/test.py
# PASS python; other suites skipped by changed-aware mapping

lf --version
# lf 0.10.0

lf op pm show --wave architecture
# fetching linear project 8c4ba3f9-cf23-4136-87ed-37847aa7dc82 for wave/architecture
# open Collapse lfd/lfq into lf; shrink lfd to a guarded subscription server
# open Unify the operating prompt
# open Retire "chord" / "member" - one wave-tree vocabulary
```
