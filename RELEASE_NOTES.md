# v0.12.31

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.31 makes it easier to start a conversation, find unfinished work and reach the Session that needs attention. Bare `lf` follows your request in the current checkout, while explicit operators handle repository and Task progression. Doctor can inspect broken Homes without changing their schema, and release recovery now handles more failures between merge, packaging and publication.

## Start with the request, reach for operations when needed

Ordinary conversations no longer begin with a repository inventory or direct all work elsewhere. Repository and Task operations have explicit entry points that preserve existing work and review boundaries.

- Bare `lf` opens a general-purpose conversation in the launching terminal and can work in the current checkout.
- `lf operate` resolves to `repo/operate` for repository cleanup, activity summaries, Wave progression and Task capture. Repository skill overrides remain supported; `wave/operate` stays explicit.
- `lf task/operate "<issue>"` guides a Task through existing work, routine failures and unfinished Flows until completion or a concrete blocker. Headless runs stop when required judgment is missing; review and delivery authority remain explicit.
- Task links can name an existing Session with a `session` query, opening the blocking conversation in Desktop, including Sessions on later result pages. Missing or unrelated Sessions leave the current workspace intact and offer Retry.

## Keep current Tasks in view

Wave Task lists emphasize current work, with successful completions available inline. Unresolved execution remains visible, and retained Sessions remain accessible.

- The **Completed** control starts with the last seven days. Edit the range inline, or enter **0** for All Tasks; toggling it off remembers the selected range.
- Settled canceled and duplicate Tasks stay out of completion history. Unknown completion dates appear only in unbounded history, without inferred dates.
- Start availability and explanations follow planning state, preventing canceled, duplicate and completed inventory from starting new work.

## Diagnose failures without changing the evidence

Doctor inspects storage before ordinary startup admission. Missing or incompatible databases produce diagnostics without initialization or migration, while readable execution evidence, scheduler obligations and installation checks remain available.

- Valid machine-level Execs no longer appear corrupt merely because they have no repository scope. Relative repository paths still fail validation.
- Missing scheduled receipts remain visible, with the configured executable, log path and reconciliation commands needed to investigate.
- Binary freshness reports use cached `origin/main`, explicitly identify that limit and show at most three recent commits without fetching.

## Recover releases from interrupted preparation

Release recovery checks the exact merged source before building or tagging. If concurrent merges leave migrations unprepared, it can prepare a successor patch when publication is confirmed absent; partial or unknown publication stops for reconciliation.

- Existing tags and canonical migrations are preserved.
- Interrupted minor releases can adopt a corrected closing patch from the same version cycle, including one published before its receipt was saved.
- Packaged CLIs must pass installer preflight in a fresh disposable Home during package smoke checks, cached preparation reuse and publication.
- Fresh GitHub evidence of a completed merge lets release settlement resume after CI repair instead of stopping on stale failures.

## Operational notes

- **Scheduled work:** newly reconciled jobs and CI-watch installations use the stable machine CLI entry point, avoiding binaries retired by installation changes. Existing Sessions retain their runtime ownership. Recovering affected live cron jobs requires installing this release, reconciling the jobs and observing a real scheduled receipt.
- **Installation:** download, promotion, rollback and recovery no longer depend on Task PR authority. Installation checks remain in place, and installation guidance points to published releases.
- **Publishers:** staging and finalization use `lf release publish`, fixing the installed-CLI incompatibility that stopped the v0.12.30 publisher. Configured publishers must now support the documented `inspect` command.
- **Journal consumers:** repository journal evidence now uses Execs and traces, with `.lf/journal/traces`, `trace_id` and `node: exec`. Update consumers to the new path and field names. Session captures retain their `~/.lf/runs` layout and remain excluded from automatic cleanup.
- **Skill names:** the former `loopflow` and `review-open-work` skills are consolidated into `repo/operate`; the retired names are removed.
- **Acceptance limits:** release recovery was exercised with simulated providers. Live concurrent-merge recovery, publication, installation and cron recovery remain unverified; hosted package preflight remains CI-owned. The complete interactive Task operator workflow and native Task-history focus and retained-Session continuation also remain unverified end to end.

## Small changes

- Codex Flow decision schemas require nullable explanation fields, addressing provider schema rejection while preserving historical receipts.
- Session capture completion persists its terminal outcome synchronously before returning, so completion no longer depends on the best-effort telemetry queue draining in time. Stream telemetry stays asynchronous, and repeated settlement preserves the original terminal receipt.