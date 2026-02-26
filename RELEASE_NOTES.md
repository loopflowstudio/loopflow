# v0.9.3

Release pipeline hardening: irreversible publishes (crates.io, PyPI) now gate on the GitHub Release succeeding first, so a build failure can't leave registries in an inconsistent state. Fixes the DMG upload step for macOS runners with externally-managed Python (PEP 668).

## Fixes

- **Release pipeline ordering** — `publish-crates` and `publish-pypi` now gate on the `release` job (which gates on `build-native` and `build-dmg`), so irreversible registry publishes only happen after all reversible artifacts succeed
- **DMG upload on macOS CI** — replaced `pip3 install boto3` with `uv pip install --system boto3` to work with PEP 668 externally-managed Python on newer macOS runner images