# Ops Flows

## Vision

Invert the ops layer. Today `lf ops <verb>` is the interface and steps are add-ons. Flip it: `lf <verb>` is the interface (composable in flows), `lf ops <verb>` is the API (mechanical, for agents). Not new ops commands from scratch — surfacing existing ops as composable steps, using existing flow/wave infrastructure.

## Strategy

`fast-path` is built — steps declare a shell command in frontmatter that runs before the agent. Exit 0 = done, no agent spun up. Non-zero = agent starts with failure output as context. `lf land` is the first consumer; sprint 04 (`lf rebase`) is the second.

Remaining sprints: 02 improves release note quality within the existing `lf ops release` path — richer PR context, narrative prompt, no new commands. Sprint 03 decomposes `lf ops release` into finer-grained ops commands and wraps them in a `lf release` step with cron cadence. Sprint 04 is just a step file that consumes the fast-path infrastructure.

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

- `lf rebase` with no conflicts completes at the same speed as `lf ops rebase` (no agent overhead)
- Release notes for a 10+ PR release read as narrative, not bullet list
