## Try it!

```bash
cargo test --all
uv run pytest python/tests/
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

What to look for:

- `lfd` now exposes persisted terminal sessions and richer wave/attention state.
- Concerto wave detail views behave like workspaces: runs, terminals, and attention live together.
- Attention items round-trip as the collapsed `interactive` / `algedonic` model instead of the older wider taxonomy.
- Gate polish also fixes the generated macOS app runpath so a locally built Concerto app can resolve bundled frameworks from `Contents/Frameworks`.

Validation from this gate:

- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅ (115 passed)
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅ (16 passed)
- `swift test --package-path swift` ✅ (271 tests passed)
- `xcodebuild test ...` ⚠️ unit/package coverage inside the run passed, but local `ConcertoUITests-Runner` still exits during bootstrap before establishing the UI-test connection

## Intent

Ship the first cohesive pass of agent embedding: `lfd` owns terminal-session state, the HTTP contract exposes it, and Concerto presents a wave as a durable workspace instead of a list plus an external terminal handoff. The branch also finishes the attention-queue contract so daemon types, client models, and UI all agree on what needs operator attention.

## Assumptions

- Concerto continues to talk to a local or bundled `lfd` that is the source of truth for wave and terminal state.
- Terminal sessions are wave-scoped artifacts worth persisting and restoring, not disposable UI-only state.
- Existing callers either already use the new attention kinds or can tolerate the legacy-to-collapsed mapping in the updated client/UI code.
- The remaining local macOS UI-test bootstrap kill is an environment/runner issue separate from the now-fixed app runpath bug.

## Key decisions

- Persist terminal sessions in the daemon and store layer instead of synthesizing them in the UI.
- Keep embedded terminals additive inside the workspace view rather than making them a separate takeover mode.
- Collapse attention kinds to the two product concepts that actually ship.
- Fix the macOS framework lookup problem in `swift/project.yml` so regenerated Xcode projects inherit the correct runpaths.

## Not included

- Remote terminal transport or a broader terminal protocol beyond the daemon-backed local session model.
- A deeper portfolio/calibration redesign beyond the workspace and attention surfaces already present here.
- A root-cause fix for the remaining local `ConcertoUITests-Runner` bootstrap failure.
