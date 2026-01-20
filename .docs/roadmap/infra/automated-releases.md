---
status: proposed
area: infra
---

# Automate PyPI releases

Publishing is manual—someone runs `uv build` and `uv publish` locally. This is error-prone and doesn't scale. Release commits like `c40f636e release: v0.6.8` should trigger automated publishing.

## Scope

Included:
- GitHub Actions workflow triggered on version tags
- Build wheel and sdist
- Publish to PyPI via trusted publishing (no API tokens)
- Create GitHub Release with changelog

Not included:
- Automated version bumping (manual version bump in __init__.py is fine)
- Automated changelog generation
- Pre-release/beta channels

## Approach

1. Set up PyPI trusted publishing for the loopflow package (one-time manual step in PyPI)
2. Add release workflow triggered on `v*` tags:

```yaml
name: Release

on:
  push:
    tags: ['v*']

jobs:
  publish:
    runs-on: ubuntu-latest
    permissions:
      id-token: write  # For trusted publishing
      contents: write  # For GitHub release
    steps:
      - uses: actions/checkout@v4
      - uses: astral-sh/setup-uv@v4
      - run: uv build
      - uses: pypa/gh-action-pypi-publish@release/v1
      - uses: softprops/action-gh-release@v2
```

3. Document release process: bump version → commit → tag → push tag

The version lives in `src/loopflow/__init__.py` and is read by hatchling. No changes needed to build config.
