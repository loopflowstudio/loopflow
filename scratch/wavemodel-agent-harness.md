# Wavemodel agent runtime + design-first Concerto

## Objective

Keep one canonical runtime path for interactive agent work (`lfd` sessions) and one onboarding path in Concerto (design-first inline chat).

## Current state (implemented on this branch)

### Runtime and prompt assembly

- Session creation is now step-context based; callers provide session metadata and `lfd` assembles prompts server-side.
- `repo_root` validation is stricter: sessions require a local repo containing `.lf/`.
- `cwd` validation is stricter: it must resolve to a directory inside `repo_root`.
- Session harness startup now consumes prepared launch payloads (`system_prompt`, `task_prompt`, `model`, `cwd`) rather than raw session config.

### Launch config model

Launch path responsibilities are split into clear types:

- **`LaunchConfig`**: canonical output of prompt assembly (`system_prompt`, `task_prompt`, `model`, `max_turns`, `cwd`, `skip_permissions`).
- **`ProcessConfig`**: process execution behavior (`auto`, `stream`, `stream_format`, `context_file`).
- **`AgentCapabilities`**: provider feature flags (for now, `chrome`).

This removed duplicated config shaping between `PreparedPrompt`, `PromptBuild`, and legacy `LaunchConfig` usage.

### Concerto UX

- Start-wave onboarding now launches inline design chat directly (no terminal detour).
- Wave detail/sidebar surfaces design content from markdown (Vision/Goals/Risks/Roadmap).
- Schema-first wave setup paths were removed from the main onboarding experience.

### Tests added/updated

- Rust coverage for launch/session behavior and session validation changes.
- Swift coverage for chat state and wave content parsing/UI behavior.

## Decisions to preserve

1. Sessions are the orchestration boundary for interactive runs.
2. Prompt assembly belongs in `lfd`, not UI/CLI callers.
3. Session startup does not accept raw `system_prompt` mode.
4. Unsupported providers fail clearly rather than degrading silently.
5. Session prompt mode remains interactive; wave executor stays auto/headless until convergence work lands.

## Remaining work

### Runtime convergence

- Route wave executor step runs through the same session orchestration path.
- Add `workspace_changed` signaling so UI can refresh on file updates.

### Concerto session UX

- Stop defaulting every chat tab to `step: design`; choose step context per tab/wave intent.
- Improve provider capability messaging for non-Claude/Codex providers.

### Wave content freshness and performance

- Add refresh triggers/watch behavior for wave README/roadmap edits.
- Move heavier markdown parsing off main-actor hot paths.

## Risks

- Runtime drift can return if executor and session paths evolve independently.
- Wave design context can become stale without reliable refresh signals.
- Large markdown parsing can still cause UI hitching until parsing work is moved off hot paths.

## Validation baseline used for this branch

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'`
- `tests/e2e/test_smoke.sh`
