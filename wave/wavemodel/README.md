# Wave Content Model

Standard content model for waves and a design-first onboarding experience.

## Vision

Every wave has a README with strategic content — Vision, Goals, Risks, Metrics — and a roadmap expressed as numbered `##-*.md` files alongside it. Waves start with `lf design`, not configuration. The content model is what makes waves more than cron jobs with a git branch.

The full experience: open Concerto, describe what you want to build, and the design conversation creates a wave with rich strategic content. Waves scale organically via `split-wave` when roadmaps grow. Steps maintain the README as they work — review adds risks, gate validates against goals — and update roadmap items as they ship.

### Not here

- Database schema changes for wave content (stays in markdown)
- Enforced validation of "at least one section" in tooling (convention first)
- Per-tab step routing in Concerto (all chat tabs default to `step: design` for now)
- Runtime convergence between wave executor and session orchestration (executor stays auto/headless; sessions are interactive — separate concerns until convergence work lands)
- Real-time wave content refresh via filesystem watcher (on-demand loading works, live updates don't)

## Goals

- Every wave README follows the standard four-section shape (Vision, Goals, Risks, Metrics)
- The roadmap lives as `##-*.md` files alongside the README, not inside it
- Steps reference README sections by name, roadmap by file convention
- `lf design` is the primary wave creation entry point
- Concerto orients new users toward design, not configuration
- Wave content (Vision, Goals) is visible in the Concerto UI

## Risks

- **README migration could lose nuance.** Mechanical restructuring might flatten content that was intentionally organized differently. Mitigate: preserve all content, only rename/regroup sections. *Phase 01 evidence: migrations preserved all content. Reorganization was clean — "North Star" → "Vision", design decisions and architecture moved into roadmap items. Risk is lower than expected for future migrations.*
- ~~**Concerto NUX depends on agentapi.**~~ *Resolved. The unified agent harness (Phase 04) shipped on the same branch, unlocking inline design sessions earlier than expected. `StartWaveView` creates a `ChatState` with `step: design` and sends the user's prompt as the first turn — no external terminal. Remaining gap: all chat tabs default to `step: design`; per-tab step routing is tracked as future work.*
- ~~**Swift client has dead WaveSchema code.**~~ *Resolved in Phase 03. `WaveSchema.swift` deleted, references removed from `LocalWaveService`, `WaveSidebar`, `RepoState`, and `WaveServiceProtocol`. Clean removal, no regressions.*
- **Step prompts may over-reference sections.** If every step checks every section, prompts get bloated and agents waste tokens on irrelevant context. Mitigate: each step references only the sections relevant to its job. *Phase 01 evidence: gate checks Goals/Risks/Metrics, review checks Vision/Goals/Risks, ingest reads all four for selection. No step references all four sections unconditionally.*
- **Section placement varies across waves.** Scope boundaries appear as "Not here" under Vision (agentapi, remote), "Security boundary (non-goals)" at the end (security), or inline in Vision (wavemodel). *Phase 03 evidence: `WaveContentParser` matches `## Vision`, `## Goals`, `## Risks`, `## Metrics` by exact heading name. Everything else is treated as supplementary. Convention-based, intentionally lenient — non-standard headings are silently ignored.*
- **Wave content loading has no filesystem watcher.** Content is loaded on-demand when a wave is selected and cached in `WaveViewModel`. Changes to wave READMEs on disk won't appear until the user re-selects the wave or status/activation changes trigger a refresh. Acceptable for now, but the agent harness (04) may need to push content updates when design conversations modify the README in real time.

## Metrics

- All wave READMEs have exactly four sections: Vision, Goals, Risks, Metrics *(Phase 01: done)*
- Roadmap lives as `##-*.md` files, not in the README *(Phase 01: done)*
- 6 built-in steps reference README sections and roadmap files by convention *(Phase 01: done)*
- `lf design` conversation produces wave directories directly (no intermediate `add-to-wave` step) *(Phase 02: done)*
- New users in Concerto see "What do you want to build?" not "Configure a wave" *(Phase 03: done)*
- Wave Vision, Goals, Risks, and Roadmap visible in Concerto detail panel *(Phase 03: done)*
- Prompt assembly shared between executor and sessions via `prepare_step_prompt()` *(Phase 04: done)*
- Sessions receive step-level intent, not raw system prompts *(Phase 04: done)*
- Inline design sessions work in Concerto without external terminal *(Phase 04: done)*

