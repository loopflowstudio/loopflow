## Try it!

```bash
# Validate launcher command/URL shapes
uv run python scripts/check_vendor_session_launch.py

# Run a normal interactive handoff in the vendor CLI/TUI
lf design

# Force a handoff for a normally-headless step
lf gate --web

# Open Codex or Claude's standalone app instead of the CLI/TUI
cat >> .lf/config.yaml <<'YAML'
session:
  launch: ide
YAML
lf design -m codex
```

Expected: `cli` opens the configured vendor TUI with the prompt loaded. `ide` opens Codex/Claude via URL scheme and falls back to CLI if no app handles the link. OpenCode always uses CLI.

Validation run:

- `cargo fmt --check` ✅
- `cargo +nightly clippy -- -D warnings` ✅
- `cargo +nightly test --all` ✅
- `uv run python scripts/check_vendor_session_launch.py` ✅
- `uv run pytest python/tests/` ✅
- `swift test --package-path swift` ✅
- `RUSTUP_TOOLCHAIN=nightly tests/e2e/test_smoke.sh` ✅
- `RUSTUP_TOOLCHAIN=nightly uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅

Notes: local stable `cargo clippy` / `cargo test --all` failed before project code on `libsqlite3-sys` using unstable `cfg_select` with Rust 1.93.0, so Rust validation used the installed nightly toolchain. The Concerto xcodebuild UI suite was started and interrupted after several minutes with no test output; Swift package tests passed.

## Intent

Loopflow stops hosting interactive sessions and becomes the layer above them. Headless work still runs through Loopflow, but human handoff now opens the vendor's own session surface with the worktree and prompt ready.

## Assumptions

- Interactive Codex and Claude launches pre-fill prompts but do not auto-submit; the human presses Enter after reviewing.
- `session.launch` applies to interactive step runs, and `--web` now means “force interactive handoff” for otherwise-headless steps.
- Codex and Claude app URL schemes are best-effort local integrations; missing or rejected handlers should fall back to CLI instead of failing the run.

## Key decisions

- Use `session.launch: cli | ide` as the only launch target distinction.
- Replace web-client launch with vendor-session launch rather than adding another flag.
- Remove mobile pairing now, but leave the larger native-chat / `lfd/sessions/harness` teardown for a separate branch.
- Exclude Cursor from `ide` because it has no stable folder+prompt GUI launch.

## Not included

- Concerto “open in app” action.
- Session resume/continue.
- Cursor GUI handoff.
- Full native chat and harness teardown.
