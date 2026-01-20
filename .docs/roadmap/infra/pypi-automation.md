# CI: Automate PyPI Publishing

**Status:** proposed

## Problem

Releases require running `scripts/publish.py` locally twice—once to create the PR, once to finalize after merge. The finalize step (tag, publish to PyPI) could fail if the local environment is misconfigured.

## Proposal

Add a tag-triggered GitHub Actions workflow for PyPI publishing:

```yaml
name: Publish to PyPI

on:
  push:
    tags:
      - 'v*'

jobs:
  publish:
    runs-on: ubuntu-latest
    permissions:
      id-token: write  # trusted publishing
    steps:
      - uses: actions/checkout@v4
      - uses: astral-sh/setup-uv@v4
      - run: uv sync
      - run: uv run python -m build
      - uses: pypa/gh-action-pypi-publish@release/v1
```

## Why This Matters

- Removes dependency on local environment for publishing
- Tag creates release—no second manual step
- Uses trusted publishing (no API tokens to manage)
- Idempotent: re-running a tag push is safe

## What This Doesn't Change

- `scripts/publish.py` still creates the release PR
- DMG builds still require local macOS (can't automate in GitHub Actions without macOS secrets)
- Version bumping and release notes remain manual

## Open Questions

- Confirm PyPI trusted publisher is configured for `loopflowstudio/loopflow`
- Whether to create GitHub Release from the workflow (adds changelog visibility)
