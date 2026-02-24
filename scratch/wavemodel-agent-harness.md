# Wavemodel runtime + Concerto onboarding (current state)

## Goal

Keep one canonical agent runtime (`lfd` sessions) and one onboarding path (design-first Concerto).

## Current baseline

- Session startup is step-based only: callers send step context fields, and lfd assembles prompts server-side.
- Session creation validates `repo_root` and requires a local repo with `.lf/`.
- Harness startup takes a prepared prompt (`system_prompt`, `task_prompt`, `model`, `cwd`) instead of raw session config.
- Concerto start-design launches inline sessions (no terminal detour) and sends the user prompt as the first turn.
- Wave detail/sidebar UI is design-first: vision/goals/risks/roadmap are visible; schema UI paths are removed.

## Decisions to preserve

1. Sessions are the orchestration boundary for interactive agent runs.
2. Prompt assembly happens in lfd, not in UI/CLI callers.
3. No raw `system_prompt` session mode.
4. Unsupported providers fail clearly instead of silently degrading.
5. Session prompt mode is interactive; wave executor remains auto/headless.

## Remaining work

### Runtime convergence

- Route wave executor step runs through the same session orchestration path.
- Add `workspace_changed` signaling so UI content can refresh after file updates.

### Concerto session UX

- Stop defaulting all chat tabs to `step: design`; pick step context per tab/wave intent.
- Add clearer provider capability messaging for non-Claude/Codex providers.

### Wave content freshness/perf

- Add refresh triggers/watch behavior for README + roadmap changes.
- Move heavy markdown parsing off main-actor UI paths.

## Risks

- Divergence risk returns if executor and sessions continue evolving separately.
- Design intent in UI can drift stale without content refresh events.
- Large wave docs can cause UI hitching until parsing is moved off hot UI paths.

## Validation baseline

Recorded green run for this branch changes:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'`
- `tests/e2e/test_smoke.sh`
