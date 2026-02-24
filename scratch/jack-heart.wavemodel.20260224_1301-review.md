# Branch review: jack-heart.wavemodel.20260224_1301

## What was implemented

- Unified lfd session startup around server-side step prompt assembly (`prepare_step_prompt`) and `PreparedPrompt` handoff to harnesses.
- Session create now validates `repo_root` and returns clear request errors for invalid config and invalid repo roots.
- Concerto start-design flow now launches inline chat sessions with step-mode session config instead of schema-driven creation UX.
- Wave detail/sidebar UI now centers design intent content (vision/goals/risks/roadmap) and removes legacy schema UI paths.

Polish added in this gate pass:

- Restored `max_turns` and `yolo_mode` behavior through the new `PreparedPrompt` path so Claude sessions still honor both flags.
- Prompt assembly now supports explicit run mode selection; sessions use `interactive` while wave executor remains `auto`.
- Added local `.lf` preflight check in `StartWaveView` with a clear `lf init` action message.
- Cleared cached chat states on repo/connection target changes to avoid stale session config reuse.
- Added Swift test coverage to verify `ChatState` forwards configured session provider/waveRunId/config into session creation.

## Key choices

- **Step-only sessions, no raw system prompt mode:** keeps one canonical prompt assembly path.
- **Provider rollout guardrails:** unsupported providers fail clearly (`unsupported` vs `not implemented yet`).
- **Interactive session prompt mode:** sessions intentionally build prompt context with interactive run semantics; executor keeps auto/headless semantics.
- **No compatibility shim for old nested session payloads:** API now expects flattened step-context fields.

## How it fits together

`POST /v0/sessions` now accepts step-context fields directly, lfd validates and normalizes them, then builds a `PreparedPrompt` from repository context and step content. That prompt is passed to harness startup, and Concerto chat sends user turns into that session stream. On the UI side, start-design and wave detail views share the same session-backed chat state model while wave content is parsed from `wave/<name>/README.md` + roadmap files for intent display.

## Risks and bottlenecks

- `RepoState.chatState(for:)` still defaults all chats to `step: design`; wave-detail chat step selection is not yet specialized.
- Wave content parsing remains on-demand (no filesystem watch); content can be stale until refresh triggers.
- Content parsing still runs on main-actor paths; very large markdown could cause minor UI hitching.
- Session create request shape is intentionally changed; older clients sending nested `config` will fail fast.

## What's not included

- Routing wave executor step runs through session orchestration.
- Workspace-changed events to auto-refresh wave content live.
- Gemini/OpenCode harness implementations.
- Broader per-tab/per-wave session step selection UX.
