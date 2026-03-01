# Ops Flows

## Vision

Invert the ops layer. Today `lf ops <verb>` is the interface and steps are add-ons. Flip it: `lf <verb>` is the interface (composable in flows), `lf ops <verb>` is the API (mechanical, for agents). Not new ops commands from scratch — surfacing existing ops as composable steps, using existing flow/wave infrastructure.

## Strategy

Steps declare `fast-path` in frontmatter — a shell command that runs before the agent. Exit 0 = done, no agent. Non-zero = agent starts with failure output as context. `lf land` and `lf rebase` are the first consumers.

Release is decomposed into focused ops commands (`release-check`, `release-notes`, `release-bump`, `release-tag`, `release-status`). The `lf release` step orchestrates them — agent judgment on notes, mechanical execution on everything else.

## Goals

- `lf rebase` rebases at ops speed on the happy path, agent only on conflicts
- `lf release` researches changes, writes narrative notes, executes the release — always agent-powered
- Release notes tell a story: thematic sections with prose, researched from actual diffs
- Release runs on autopilot via cron — patch daily, minor monthly, skip when empty
- Concerto surfaces release config and a "release now" button per repo

## Risks

- **Release notes quality is subjective.** Plan to iterate on the prompt after seeing real output.
- **No golden test for rebase.** The rebase step embeds correctly (existing golden tests pass), but prompt regressions for rebase specifically wouldn't be caught. Low priority — note if adding golden tests for other steps.

## Metrics

- Fast-path rebase latency: seconds for `lf rebase` with no conflicts (target: within 1s of `lf ops rebase`)
- % of rebases resolved by fast-path without agent (target: >80%)
- Release notes quality: user satisfaction score on a 1–5 scale per release (target: 4+)
- Release automation rate: % of releases triggered by cron vs manual (target: >90% after stabilization)
