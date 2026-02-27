# Ops Flows

## Vision

Invert the ops layer. Today `lf ops <verb>` is the interface and steps are add-ons. Flip it: `lf <verb>` is the interface (composable in flows), `lf ops <verb>` is the API (mechanical, for agents).

Steps that don't need an agent on the happy path declare `fast-path` — a command that runs first. If it succeeds, done. If it fails, the agent session starts with failure context. No regression to agent speed for operations that usually just work.

### Not here

- New ops commands from scratch — this is about surfacing existing ops as composable steps
- Changing how flows/waves work — this uses existing infrastructure
- Team-level features

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
