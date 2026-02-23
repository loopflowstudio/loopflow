# Wave Content Model

Standard content model for waves and a design-first onboarding experience.

## Vision

Every wave has a README with strategic content — Vision, Goals, Risks, Metrics — and a roadmap expressed as numbered `##-*.md` files alongside it. Waves start with `lf design`, not configuration. The content model is what makes waves more than cron jobs with a git branch.

The full experience: open Concerto, describe what you want to build, and the design conversation creates a wave with rich strategic content. Waves scale organically via `split-wave` when roadmaps grow. Steps maintain the README as they work — review adds risks, gate validates against goals — and update roadmap items as they ship.

### Not here

- Database schema changes for wave content (stays in markdown)
- Enforced validation of "at least one section" in tooling (convention first)
- Full interactive `lf design` inside Concerto (depends on agentapi)

## Goals

- Every wave README follows the standard four-section shape (Vision, Goals, Risks, Metrics)
- The roadmap lives as `##-*.md` files alongside the README, not inside it
- Steps reference README sections by name, roadmap by file convention
- `lf design` is the primary wave creation entry point
- Concerto orients new users toward design, not configuration
- Wave content (Vision, Goals) is visible in the Concerto UI

## Risks

- **README migration could lose nuance.** Mechanical restructuring might flatten content that was intentionally organized differently. Mitigate: preserve all content, only rename/regroup sections. *Phase 01 evidence: migrations preserved all content. Reorganization was clean — "North Star" → "Vision", design decisions and architecture moved into roadmap items. Risk is lower than expected for future migrations.*
- **Concerto NUX depends on agentapi.** The full interactive design experience can't ship until interactive agent sessions work in Concerto. Mitigate: Phase 03 does what it can now (UI framing, content display), full experience completes later.
- **Step prompts may over-reference sections.** If every step checks every section, prompts get bloated and agents waste tokens on irrelevant context. Mitigate: each step references only the sections relevant to its job. *Phase 01 evidence: gate checks Goals/Risks/Metrics, review checks Vision/Goals/Risks, ingest reads all four for selection. No step references all four sections unconditionally.*
- **Section placement varies across waves.** Scope boundaries appear as "Not here" under Vision (agentapi, remote), "Security boundary (non-goals)" at the end (security), or inline in Vision (wavemodel). Phase 03's README parser should match `## Vision`, `## Goals`, `## Risks`, `## Metrics` as the four sections and treat everything else as supplementary.

## Metrics

- All wave READMEs have exactly four sections: Vision, Goals, Risks, Metrics *(Phase 01: done)*
- Roadmap lives as `##-*.md` files, not in the README *(Phase 01: done)*
- 6 built-in steps reference README sections and roadmap files by convention *(Phase 01: done)*
- `lf design` conversation produces wave directories directly (no intermediate `add-to-wave` step)
- New users in Concerto see "What do you want to build?" not "Configure a wave"

