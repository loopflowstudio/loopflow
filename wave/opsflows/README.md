# Ops Flows

## Vision

Invert the ops layer. Today `lf ops <verb>` is the interface and steps are add-ons. Flip it: `lf <verb>` is the interface (composable in flows), `lf ops <verb>` is the API (mechanical, for agents). Not new ops commands from scratch — surfacing existing ops as composable steps, using existing flow/wave infrastructure.

## Strategy

`fast-path` is built — steps declare a shell command in frontmatter that runs before the agent. Exit 0 = done, no agent spun up. Non-zero = agent starts with failure output as context. `lf land` is the first consumer; sprint 04 (`lf rebase`) is the second.

Release is decomposed into focused ops commands (`release-check`, `release-notes`, `release-bump`, `release-tag`, `release-status`). The `lf release` step orchestrates them with agent judgment on release notes, mechanical execution on everything else. Cron waves run daily patch and monthly minor releases. The monolith (`lf ops release patch`) still works for manual use.

Remaining sprints: 04 wraps `lf ops rebase` in a step with fast-path for no-conflict rebases. 05 brings release config and "Release Now" to Concerto.

## Goals

- `lf rebase` rebases at ops speed on the happy path, agent only on conflicts
- `lf release` researches changes, writes narrative notes, executes the release — always agent-powered
- Release notes tell a story: thematic sections with prose, researched from actual diffs
- Release runs on autopilot via cron — patch daily, minor monthly, skip when empty
- Concerto surfaces release config and a "release now" button per repo

## Risks

- **Release notes quality is subjective.** Plan to iterate on the prompt after seeing real output.
- **Ops decomposition scope.** Splitting `lf ops release` into sub-commands is the biggest Rust change. Keep the existing monolithic command working during transition.

## Metrics

- Fast-path rebase latency: seconds for `lf rebase` with no conflicts (target: within 1s of `lf ops rebase`)
- % of rebases resolved by fast-path without agent (target: >80%)
- Release notes quality: user satisfaction score on a 1–5 scale per release (target: 4+)
- Release automation rate: % of releases triggered by cron vs manual (target: >90% after stabilization)
