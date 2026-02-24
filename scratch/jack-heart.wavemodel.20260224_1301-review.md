# Review: wavemodel agent harness + design-first Concerto

## What was implemented

- Unified launch plumbing around a canonical `LaunchConfig` (prompt/model/cwd/turn budget) plus `ProcessConfig` and `AgentCapabilities` for runtime flags.
- Moved session prompt assembly fully into `lfd` (`prepare_step_prompt`), so session callers send step context instead of raw prompts.
- Updated Claude/Codex session harness startup to consume prepared launch config and keep provider resume/session-id behavior.
- Strengthened session creation validation: `repo_root` must exist and contain `.lf/`; `cwd` must resolve to a directory inside `repo_root`.
- Shifted Concerto onboarding to design-first inline chat, exposed wave content (Vision/Goals/Risks/Roadmap) in detail UI, and removed schema-first UX paths.
- Added/updated Rust and Swift tests for launch/session behavior and wave content parsing.

## Key choices

- **Prompt assembly in daemon, not UI/CLI**: keeps one source of truth for context gathering and step intent.
- **Split launch vs process concerns**: avoids overloading one config struct with unrelated concerns and makes APIs easier to reason about.
- **Design-first wave creation**: starts users with intent and context, not config forms.
- **Security/consistency check on `cwd`**: prevents sessions from executing outside the declared repo boundary.

## How it fits together

`lfd::prompt::prepare_step_prompt()` now builds the same launch payload used by session harness startup, while CLI/executor paths compose process/capability options around that payload. On the UI side, Concerto uses session APIs for inline design chat and reads parsed wave markdown content to render strategy + roadmap in wave detail.

## Risks and bottlenecks

- Chat tabs still default to `step: design`; per-tab/per-intent step routing is not implemented yet.
- Wave content refresh is still event/on-demand driven (no dedicated filesystem watcher), so edits may not appear instantly in all states.
- Large markdown docs may still cost UI parsing time on main actor until parsing is moved further off hot paths.

## What's not included

- No executor-to-session orchestration convergence yet (wave executor still has its own auto/headless flow).
- No new provider support beyond Claude/Codex session harnesses.
- No schema/database storage migration for wave strategic content (markdown remains source of truth).
