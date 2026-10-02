# v0.12.31

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.31 makes it easier to start a conversation, find unfinished work and reach the Session that needs attention. Bare `lf` follows your request in the current checkout, while explicit operators handle repository and Task progression. Doctor can inspect broken Homes without changing their schema, and delivery fixes address stale CI failures and release-publisher compatibility.

## Start with the request, reach for operations when needed

Ordinary conversations no longer begin with a repository inventory or direct all work elsewhere. Repository and Task operations have explicit entry points that preserve existing work and review boundaries.

- Bare `lf` opens a general-purpose conversation in the launching terminal and can work in the current checkout.
- `lf operate` resolves to `repo/operate` for repository cleanup, activity summaries, Wave progression and Task capture. Repository skill overrides remain supported; `wave/operate` stays explicit.
- `lf task/operate "<issue>"` guides a Task through existing work, routine failures and unfinished Flows until completion or a concrete blocker. Headless runs stop when required judgment is missing; review and delivery authority remain explicit.
- Task links can name an existing Session with a `session` query, opening the blocking conversation in Desktop. Missing or unrelated Sessions leave the current workspace intact and offer Retry.

## Keep current Tasks in view

Wave Task lists now emphasize current work, with successful completions available inline. Unresolved execution remains visible, and retained Sessions remain accessible.

- The **Completed** control starts with the last seven days. Edit the range inline, or enter **0** for All Tasks; toggling it off remembers the selected range.
- Settled canceled and duplicate Tasks stay out of completion history. Unknown completion dates appear only in unbounded history, without inferred dates.
- Start availability and explanations follow planning state, preventing canceled, duplicate and completed inventory from starting new work.

## Diagnose failures without changing the evidence

Doctor runs storage inspection before ordinary startup admission. Missing or incompatible databases can therefore produce diagnostics without initialization or migration, while readable execution evidence, scheduler obligations and installation checks remain available.

- Valid machine-level Execs no longer appear corrupt merely because they have no repository scope. Relative repository paths still fail validation.
- Missing scheduled receipts remain visible, with the configured executable, log path and reconciliation commands needed to investigate.
- Binary freshness reports use cached `origin/main`, explicitly identify that limit and show at most three recent commits without fetching.

## Operational notes

- **Scheduled work:** newly reconciled jobs and CI-watch installations use the stable machine CLI entry point, avoiding binaries retired by installation changes. Existing Sessions retain their runtime ownership. Recovering affected live cron jobs requires installing this release, reconciling the jobs and observing a real scheduled receipt.
- **Installation:** download, promotion, rollback and recovery no longer depend on Task PR authority. Installation checks remain in place, and installation guidance points to published releases.
- **Release settlement:** fresh GitHub evidence of a completed merge now lets settlement resume after CI repair instead of stopping on stale failures. The publisher uses `lf release publish` for staging and finalization, fixing the installed-CLI incompatibility that stopped the v0.12.30 publisher.
- **Skill names:** the former `loopflow` and `review-open-work` skills are consolidated into `repo/operate`; the retired names are removed.
- **Acceptance limits:** supplied evidence includes focused Rust, Swift and publisher tests. Published upgrade and interrupted-switch recovery, live cron recovery, the complete interactive Task operator workflow, and native Task-history focus and Session-continuation checks remain unverified end to end.

## Small changes

- Codex Flow decision schemas now require nullable explanation fields, addressing provider schema rejection while preserving historical receipts.