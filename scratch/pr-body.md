## Try it!

```bash
uv run pytest python/tests/test_install_script.py -q
uv run python scripts/install.py -n --skip wheel --skip cargo --skip swift
```

The dry run prints the app bundle executable layout:

```text
Contents/MacOS/: Concerto, lf, lfd
```

Full validation run:

```bash
uv run ruff check scripts/install.py python/tests/test_install_script.py
uv run ruff format --check scripts/install.py python/tests/test_install_script.py
uv run pytest python/tests/ -q
```

## Intent

Fix local Concerto installs so the app bundle contains every executable the app launches. `lfd` and `lf` now live in `Contents/MacOS/` beside `Concerto`, matching `Bundle.main.url(forAuxiliaryExecutable:)`, and the installer rejects broken bundles before claiming success.

## Assumptions

- The local Concerto installer targets macOS and can use `lipo` and `codesign`.
- `swift build -c release` and `cargo build -p loopflow --release` produce the artifacts installed into the bundle.
- `--skip` skips build stages only; install steps still install existing artifacts.

## Key decisions

- Added `BundleSpec` as the single source of truth for the app layout.
- Made missing artifacts and invalid bundles fatal `StageError`s instead of silent skips.
- Added a `codesign --verify` smoke check after ad-hoc signing.

## Not included

- Notarization or release packaging changes.
- Runtime lookup changes in the Swift app.
