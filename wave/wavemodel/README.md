# Wave Content Model

Standard content model for waves and a design-first onboarding experience.

## Vision

Every wave has strategic content — Vision, Goals, Risks, Metrics, Roadmap — that agents read, maintain, and evolve. Waves start with `lf design`, not configuration. The content model is what makes waves more than cron jobs with a git branch.

The full experience: open Concerto, describe what you want to build, and the design conversation creates a wave with rich strategic content. Waves scale organically via `split-wave` when roadmaps grow. Steps maintain the README as they work — review adds risks, implement checks off roadmap items, gate validates against goals.

### Not here

- Database schema changes for wave content (stays in markdown)
- Enforced validation of "at least one section" in tooling (convention first)
- Full interactive `lf design` inside Concerto (depends on agentapi)

## Goals

- Every wave README follows the standard five-section shape
- Steps reference sections by name, not ad-hoc patterns
- `lf design` is the primary wave creation entry point
- Concerto orients new users toward design, not configuration
- Wave content (Vision, Goals) is visible in the Concerto UI

## Risks

- **README migration could lose nuance.** Mechanical restructuring might flatten content that was intentionally organized differently. Mitigate: preserve all content, only rename/regroup sections.
- **Concerto NUX depends on agentapi.** The full interactive design experience can't ship until interactive agent sessions work in Concerto. Mitigate: commit 3 does what it can now (UI framing, content display), full experience completes later.
- **Step prompts may over-reference sections.** If every step checks every section, prompts get bloated and agents waste tokens on irrelevant context. Mitigate: each step references only the sections relevant to its job.

## Metrics

- All 5 wave READMEs pass `grep` for standard sections
- `lf design` conversation produces wave directories directly (no intermediate `add-to-wave` step)
- New users in Concerto see "What do you want to build?" not "Configure a wave"

## Roadmap

| # | Phase | What it delivers | Status |
|---|-------|-----------------|--------|
| 01 | README shape + migrations + step prompts | Standard content model across all waves and steps | This branch |
| 02 | Design creates waves + split-wave | `lf design` writes to `wave/` directly; `split-wave` step exists | |
| 03 | Concerto NUX | Design-first onboarding, hidden worktrees, wave content in detail panel | |
