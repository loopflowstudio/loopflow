# Ops Flows

## Vision

Invert the ops layer. Today `lf ops <verb>` is the interface and steps are add-ons. Flip it: `lf <verb>` is the interface (composable in flows), `lf ops <verb>` is the API (mechanical, for agents). Not new ops commands from scratch — surfacing existing ops as composable steps, using existing flow/wave infrastructure.

## Strategy

`fast-path` is the architectural linchpin. Steps declare a shell command in frontmatter that runs before the agent. Exit 0 = done, no agent spun up. Non-zero = agent starts with failure output as context. This keeps ops speed on happy paths while preserving agent resilience for failures.

Sprints 01 and 04 depend on `fast-path` — sprint 01 builds it as a step runner feature, sprint 04 is the second consumer. Sprints 02 and 03 build on existing `lf ops release` infrastructure, decomposing it into finer-grained ops commands and wrapping them in a step that always uses an agent (release notes require LLM judgment).

## Goals

- `lf land` lands the PR, rotates the shortname worktree, advances to next wave item — fast-path, no agent
- `lf rebase` rebases at ops speed on the happy path, agent only on conflicts
- `lf release` researches changes, writes narrative notes, executes the release — always agent-powered
- Release notes tell a story: thematic sections with prose, researched from actual diffs
- Release runs on autopilot via cron — patch daily, minor monthly, skip when empty
- Concerto surfaces release config and a "release now" button per repo
- `fast-path` as a general step feature — any step can declare a fast command that skips the agent on success

## Risks

- **Worktree rename while cwd is inside it.** Need to handle gracefully.
- **Release notes quality is subjective.** Plan to iterate on the prompt after seeing real output.
- **Ops decomposition scope.** Splitting `lf ops release` into sub-commands is the biggest Rust change. Keep the existing monolithic command working during transition.

## Metrics

- `lf rebase` with no conflicts completes at the same speed as `lf ops rebase` (no agent overhead)
- `lf land` in a shortname worktree rotates and advances without manual steps
- Release notes for a 10+ PR release read as narrative, not bullet list
- Cron-triggered release fires and completes unattended
