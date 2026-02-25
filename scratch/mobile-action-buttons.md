# Mobile action buttons (Stage 02)

## Status

Implemented on branch `jack-heart.mobile.20260225_1122` and validated on February 25, 2026.

This document is the consolidated source for the design intent, implementation shape, and current validation state.

## Goal

Reduce follow-up friction in chat by turning likely next steps into one-tap actions, especially on iPhone where typing is slower.

## What shipped

### 1) Engine-level synthetic tools

- Added `SyntheticTool` + `ClientContext` and injected tools through `AgentConfig`.
- Added `synthetic_tools_for_context` so UI-aware sessions receive `suggest_actions` guidance.
- `ClientContext` is passed through launch/session setup (`client_has_ui`, `client_compact`).

### 2) Step-aware suggestion style

- Added `action_style` parsing from step frontmatter.
- Style is propagated into synthetic guidance:
  - `procedural`: workflow-forward choices
  - `exploratory`: branching/curiosity choices
  - default fallback: concrete generic actions

### 3) Provider-agnostic tool realization

- Harnesses realize synthetic tools via prompt guidance (no MCP registration).
- Model emits tagged payloads:

```xml
<lf:suggest_actions>
[{"label":"Land PR"},{"label":"Run tests"}]
</lf:suggest_actions>
```

### 4) Stream parsing + typed events

- Added shared Rust `LfTagParser` to parse `<lf:suggest_actions>` across streaming deltas.
- Parser strips tags from visible chat text and emits typed event payloads.
- Added `SessionEvent::SuggestedActions` and `SuggestedActionPayload`.

### 5) Swift state + UI integration

- Added shared `SuggestedAction` model and `ChatState.suggestedActions`.
- Added sanitization rules:
  - discard empty labels
  - cap list size
  - latest payload replaces previous actions (including empty-after-sanitize payloads)
- Added `ActionButtonsView` in LoopflowCore and embedded it in `WaveChatView` above composer.
- Tap behavior routes through normal send flow (`sendSuggestedAction`), then clears suggestions immediately.

## Runtime behavior

### Injection rules

- Sessions with `client_has_ui: true` get `suggest_actions` guidance.
- Headless contexts (`lf`, wave executor) do not.
- Compact clients (iPhone) get lower action count guidance than regular-width clients.

### Clear/hide rules

Suggested actions are cleared on:
- user send,
- user manual typing start,
- new agent turn start,
- session end/reset.

## Validation status

### Passing

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all -- --skip lfd::executor::docker::tests::docker_startup_lost_agent_does_not_flip_terminal_run_wave_status --skip lfd::executor::docker::tests::docker_startup_rehydrates_running_agents_and_cleans_orphans`
- `uv run pytest python/tests/`
- `swift test --package-path swift`

### Environment-limited

- `cargo test --all` fails locally only on Docker-socket-dependent tests (no `/var/run/docker.sock`).
- `xcodebuild test` for Concerto currently fails locally at `ConcertoUITests` link step (`open() failed, errno=1`).

## Risks and follow-ups

- Suggestion quality is still prompt-compliance-dependent; no strict provider tool contract.
- Text-tag protocol is robust to stream splits but still a malformed-input surface.
- Future synthetic tools (for example `memory`) should reuse this exact pipeline before adding new transport or protocol complexity.

## Out of scope (still)

- Additional synthetic tools beyond `suggest_actions`.
- Persistence/analytics/ranking of actions.
- Transport redesign or MCP-style registration.
