# Gate review: Concerto bundle executable layout

## What was implemented

Added a `BundleSpec` for `Loopflow Concerto.app` and installs `Concerto`, `lf`, and `lfd` into `Contents/MacOS/`. The installer now verifies the bundle layout before signing: each executable must exist, be executable, be Mach-O, and include the current architecture.

The local installer now fails fast through `StageError` when build artifacts are missing, build stages fail, bundle verification fails, or `codesign --verify` rejects the app. Regression tests cover the missing `lfd` launch failure, wrong-architecture bundles, non-Mach-O placeholders, codesign verification failures, and silent binary install skips.

## Key choices

- Keep app layout in one `BundleSpec` instead of scattering `Contents/MacOS` and resource paths across install code.
- Put `lf` and `lfd` beside `Concerto` because `Bundle.main.url(forAuxiliaryExecutable:)` searches `Contents/MacOS/`.
- Verify before codesigning so broken bundles fail with installer errors instead of launching as a damaged app.
- Keep `--skip` scoped to build stages. Install steps still consume existing artifacts, which preserves the script's local developer workflow.

## How it fits together

The CLI runs parallel builds, then sequential installs. Binary installs use `_atomic_install`; Concerto install copies every executable/resource from `BundleSpec`, validates the resulting bundle, signs it, and runs a signing smoke check.

## Risks and bottlenecks

- Bundle verification depends on macOS tools (`lipo`, `codesign`), matching the installer's macOS app target.
- Codesigning still happens after replacing the existing app, so a signing failure can leave a newly copied but unusable bundle in `/Applications`.
- The full local install path was not run because it would write to user-global install locations and `/Applications`; dry-run plus unit tests covered the changed logic.

## What's not included

- No notarization or distribution packaging changes.
- No changes to Concerto runtime lookup code.
- No migration for older broken app bundles beyond replacing them on the next local install.

## Validation

- `uv run ruff check scripts/install.py python/tests/test_install_script.py`
- `uv run ruff format --check scripts/install.py python/tests/test_install_script.py`
- `uv run pytest python/tests/test_install_script.py -q` → 6 passed
- `uv run pytest python/tests/ -q` → 129 passed
- `uv run python scripts/install.py -n --skip wheel --skip cargo --skip swift` → dry-run shows `Contents/MacOS/: Concerto, lf, lfd`
