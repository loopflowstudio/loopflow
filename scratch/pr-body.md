## Try it!

```bash
LOOPFLOW_CODESIGN_IDENTITY=- uv run scripts/install.py local --use
type -a lf
lf --version
lf op pm show --wave architecture
```

`lf` should resolve to the refreshed `Loopflow.app` bundle or local bin copy,
report `lf 0.10.0`, and read the architecture wave's Linear roadmap. If Linear
auth is missing or expired, the command should fail with a Linear-specific
auth/config error, not `asana_project is missing`.

## Intent

Unblock architecture roadmap access after the PM provider flip to Linear. The
source provider path already selects Linear from `wave/architecture/GOAL.md`;
the failure came from a stale `lf 0.9.12` inside the active macOS app bundle
shadowing the fresh binary on PATH.

## Assumptions

The active macOS bundle has the standard `Loopflow.app/Contents/MacOS/lf`
layout. Non-bundle installs keep the existing `/Applications` promotion
behavior.

## Key decisions

`local --use` now promotes the rebuilt app into the same applications directory
as the currently active bundle. The wheel install step now uses `uv pip install`
only; the old `uv tool install` path was obsolete because the Python wheel has
no console entrypoint.

No PM provider-selection code changed, and no lfd stale-binary guard was added.
That guard is a separate design decision.

## Not included

This does not touch the lfd hard-cut route-contract work. It also does not
restart `lfd` unless the operator explicitly runs `local --use --service`.
