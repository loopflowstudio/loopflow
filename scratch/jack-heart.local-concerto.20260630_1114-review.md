# Gate review: local build story + Loopflow rename

## What was implemented

The macOS app is now user-facing **Loopflow** while the internal Swift target and executable remain `Concerto`. Local builds stage `lf`, `lfd`, and `Loopflow.app` into a per-worktree `local-bin/`; `scripts/install.py local --use` promotes that build by symlinking `lf`/`lfd` onto PATH and copying `Loopflow.app` into `/Applications`.

Release packaging now builds `swift/dist/Loopflow.dmg`, uploads `Loopflow-<version>.dmg` and `Loopflow-latest.dmg`, and stamps the app bundle version from `RELEASE_TAG` or the Cargo workspace version. Docs describe the desktop app as a peer release artifact, not a side build.

The branch also fixes Codex skill launches with empty system prompts by skipping empty context-file writes, so Codex does not receive an invalid `model_instructions_file`.

## Key choices

- Keep `com.loopflow.concerto` and the `Concerto` executable. That preserves existing permissions, deep links, Xcode target names, and internal dev scripts while renaming user-visible app surfaces.
- Build first, promote second. `local-bin/` gives each sibling worktree an isolated build; `--use` is the explicit active-build switch.
- Symlink CLI binaries but copy the app. Symlinks make rebuilt `lf`/`lfd` active immediately; macOS app installation still follows the conventional `/Applications/Loopflow.app` copy.
- Share bundle-version stamping through `scripts/bundle_version.py` so local and release packaging read the same version source.

## How it fits together

`install.py local` runs the selected build stages, stages binaries into `local-bin/`, creates `local-bin/Loopflow.app`, stamps `Info.plist`, and verifies/signs the bundle. With `--use`, it installs the Python wheel, symlinks `lf` and `lfd` into the resolved install dir, removes the legacy `/Applications/Loopflow Concerto.app`, and copies `Loopflow.app` into `/Applications`.

`release-concerto.py` still builds the `Concerto` Swift product, packages it under the `Loopflow.app` bundle name, stamps the bundle from `RELEASE_TAG`, creates `Loopflow.dmg`, then the release workflow uploads the new R2 keys.

## Risks and bottlenecks

- The public DMG key changed. The loopflowstudio website download link must move to `https://downloads.loopflow.studio/Loopflow-latest.dmg` in the same release window.
- `scripts/install.py local --use` writes to `/Applications`; this gate validated dry-run and unit behavior, not a real promotion on this machine.
- Release signing/notarization depends on CI secrets and was not exercised locally.
- Docker smoke exits green locally, but two Docker runtime tests self-skipped because the Rust harness expects `/var/run/docker.sock` while this machine exposes Docker through OrbStack.

## What's not included

- No Wave -> Loop code migration. `scratch/vocabulary.md` records the vocabulary decision for a later migration.
- No `concerto-dev.py` reorg. The dev app and target remain under the Concerto nickname.
- No loopflowstudio website change.

## Validation

- `uv run python scripts/install.py local -n --skip cargo --skip swift`: passed; shows `Loopflow.app` staged into `local-bin/` and no promotion without `--use`.
- `uv run pytest python/tests/test_install_script.py -v`: 7 passed.
- `cargo fmt --all -- --check`: passed.
- `uv run pytest python/tests/`: 153 passed.
- `swift test --package-path swift`: 336 Swift tests passed.
- `uv run python scripts/check_swift_multiplatform_boundaries.py`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `tests/e2e/test_smoke.sh`: passed.
- `cargo test --all`: passed.
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`: 16 passed.
- `docker version`: passed.
- `cargo test -p loopflow docker_ -- --nocapture`: passed; see Docker socket note above.
- `cd swift && xcodegen generate`: passed.
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -skip-testing:ConcertoUITests -resultBundlePath ConcertoTests.xcresult CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`: passed; 336 tests.
- `git diff --check`: passed.
- `uv run ruff check scripts/install.py scripts/generate_screenshots.py scripts/bundle_version.py python/tests/test_install_script.py tests/regression/test_orphaned_runs_reset_wave_status.py tests/regression/test_terminal_session_dto_exposes_tmux_name.py python/tests/test_pm_reset.py`: passed.
