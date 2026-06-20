# Gate review — session handoff

## What was implemented

- Added `session.launch: cli | ide` to repo/user config and documented it in `docs/config.md`, `docs/lf.md`, built-in Loopflow docs, prompt goldens, and wave plans.
- Replaced the old `--web` behavior (copy prompt + open marketing web URL) with `launch_session`: interactive steps and explicit `--web` now open a vendor session with the assembled prompt loaded.
- Implemented CLI launch shapes for Codex, Claude, and OpenCode, plus app URL schemes for Codex and Claude. OpenCode and missing GUI handlers fall back to CLI.
- Removed mobile pairing from the product surface: `lf op pair`, QR/link pairing payloads, long-lived pairing-token helper, camera usage string, and the pairing smoke script are gone.
- Reframed desktop/workflows/mobile wave docs around “Loopflow as the layer above”; mobile is archived, desktop keeps the embedded terminal frame, and workflows now own vendor-session launch.

## Key choices

- **Two launch targets, not four.** `cli` means terminal/TUI; `ide` means the vendor standalone app. Concerto’s embedded pane is just a rendering surface for `cli`, not a separate config value.
- **Pre-fill, don’t auto-run.** Codex and Claude interactive surfaces load the prompt and wait for the human to press Enter. That matches the vendor behavior and keeps the handoff reviewable.
- **Use vendor URL schemes for app launch.** Codex uses `codex://threads/new`; Claude uses `claude://code/new`. The old web URLs and `open -a` paths do not seed the right session.
- **Keep teardown partial.** This branch removes pairing and ships the launcher. The larger native-chat / `lfd/sessions/harness` removal stays out of scope.

## How it fits together

`lf` assembles the same prompt as before, parses `agent: harness:model`, then chooses execution by mode. Headless runs still call the existing agent launcher. Interactive runs and `--web` call `launch_session(session.launch, harness, model, repo_root, prompt)`, which builds both a CLI command and, for `ide`, a scheme URL. The URL is tried first for Codex/Claude app launches; failure falls back to the CLI command.

## Risks and bottlenecks

- `--web` is now a compatibility name for “interactive vendor handoff,” not literal browser launch. Docs were updated, but existing muscle memory may lag.
- App URL scheme availability is inherently local. The code falls back to CLI when `open`/`xdg-open` fails, but it cannot prove the app accepted every deep-link parameter.
- Codex/Claude launches pre-fill rather than auto-send; users still press Enter. That is intentional but different from the earlier spike assumption.
- Local stable Rust 1.93.0 failed in `libsqlite3-sys` build script (`cfg_select` unstable). Nightly completed the Rust suites. This appears toolchain/dependency-level, not introduced by this diff.
- Concerto UI xcodebuild was started and then interrupted after several minutes with no test output. `swift test --package-path swift` passed.

## What's not included

- No removal of native chat rendering or `lfd/sessions/harness`.
- No Concerto “open in app” button yet.
- No session resume/continue support.
- No Cursor GUI launch target.
- No OpenCode app launch; it remains CLI-only.
