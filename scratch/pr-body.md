## Try it!

```bash
uv run python scripts/install.py local -n --skip cargo --skip swift
```

The dry run shows this worktree staging `lf`, `lfd`, and `Loopflow.app` into `local-bin/` without promoting them. Add `--use` after a real build to symlink `lf`/`lfd` onto PATH and install `/Applications/Loopflow.app`.

Validation run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
uv run pytest python/tests/
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift
uv run python scripts/check_swift_multiplatform_boundaries.py
cd swift && xcodegen generate
cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -skip-testing:ConcertoUITests -resultBundlePath ConcertoTests.xcresult CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

All passed locally. `cargo test -p loopflow docker_ -- --nocapture` also passed; two Docker runtime cases self-skipped because the local Docker setup is OrbStack-backed and the Rust harness checks `/var/run/docker.sock`.

## Intent

Make the desktop app a peer of `lf` and `lfd` in local builds and releases. The user-facing product is now `Loopflow.app` / `Loopflow.dmg`, while Concerto remains the internal Swift target and executable name.

## Assumptions

- Keeping `com.loopflow.concerto` is intentional so existing app permissions and deep-link registrations survive the rename.
- The loopflowstudio website download link will be updated with this release to use `Loopflow-latest.dmg`.
- `scripts/pull-local-bin.sh` remains the CLI-only fast path; `scripts/install.py local --use` is the full local build + app promotion path.

## Key decisions

- Build into per-worktree `local-bin/`; promote explicitly with `--use`.
- Symlink `lf`/`lfd` from the active worktree so rebuilds take effect without another copy step.
- Copy `Loopflow.app` into `/Applications` and remove the legacy `Loopflow Concerto.app` during promotion.
- Stamp `CFBundleShortVersionString` and `CFBundleVersion` from `RELEASE_TAG` in CI or the Cargo workspace version locally.
- Skip empty context-file writes for Codex skill steps, because Codex rejects an empty `model_instructions_file`.

## Not included

- No Wave -> Loop code migration; `scratch/vocabulary.md` only records the vocabulary decision.
- No `concerto-dev.py` reorg; the dev nickname stays Concerto.
- No website repository update for the renamed DMG URL.
