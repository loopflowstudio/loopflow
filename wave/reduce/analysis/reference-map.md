---
head: 615729570782d730d2ea3b196e34779db9f63555
status: bootstrap
---

# Reference Map

## Why this exists

Reduce needs outside reference systems for comparison. The point is not to copy
them. The point is to find where loopflow's concepts are real, where they are
accidental, and where a mature implementation has already answered a question
we are still carrying as code.

## Coding agent sessions

| Loopflow surface | References | What to study |
|---|---|---|
| Local coding sessions | OpenAI Codex CLI, OpenCode | Session lifecycle, approval model, sandboxing, prompt/context protocol, resume/continuation behavior |
| Multi-surface agent control | Codex CLI/app/docs, OpenCode TUI/web/IDE | How one session model spans terminal, desktop, IDE, and web without duplicating concepts |
| Provider selection | OpenCode providers/config, Codex auth/model config | Whether provider differences belong in step config, wave config, or runtime session state |
| Long-running work | Codex cloud/task model, loopflow waves | What state should persist across tasks vs. what should be recomputed from repo state |

Research sources:

- https://github.com/openai/codex - Codex CLI repository; describes Codex CLI
  as a local coding agent and links to Codex docs.
- https://opencode.ai/docs/ - OpenCode docs; terminal, desktop app, and IDE
  extension over an open source coding agent.

## Context assembly

| Loopflow surface | References | What to study |
|---|---|---|
| Step context | Codex AGENTS.md behavior, Claude/Codex skills, aider repo map | What belongs in always-included context vs. explicit docs |
| Repo awareness | aider repository map | Symbol maps and dependency summaries that fit a budget without pretending to be the whole repo |
| Compression | loopflow `token-compress`, Codex context compaction | Which facts must survive compaction: decisions, paths, commands, open questions, invariants |
| Prompt portability | Loopflow steps, Claude skills, Codex skills | How much prompt structure can be shared across agent runtimes |

Research sources:

- https://aider.chat/docs/repomap.html - aider's repository map docs.
- https://github.com/openai/codex - includes AGENTS.md conventions in a large
  real coding-agent codebase.

## Browser and computer use

| Loopflow surface | References | What to study |
|---|---|---|
| Web QA harnesses | Playwright MCP, gstack browser skills | Accessibility-tree actions vs. screenshot-driven control |
| Concerto/browser verification | Playwright test suites, browser-use style agents | How to make UI checks reproducible enough for agents to trust |
| Agent tools | MCP servers, custom tools, structured snapshots | Tool descriptions, safety boundaries, and deterministic artifacts |

Research sources:

- https://github.com/microsoft/playwright-mcp - Playwright MCP server; browser
  automation via structured accessibility snapshots.

## Work orchestration

| Loopflow surface | References | What to study |
|---|---|---|
| Waves and workers | CI job queues, background workers, agent swarms | Queue semantics, retry semantics, cancellation, ownership |
| Triggers | GitHub Actions, webhook consumers | How to prevent duplicate work and make activation explainable |
| PM sync | Asana/Linear/Notion integrations | Which side owns item status at each point in the lifecycle |
| Release | Release Please, Changesets, conventional release systems | Separating shipped behavior, release intent, and generated notes |

## First comparison project

Chart loopflow's agent-session model against Codex and OpenCode:

- Session identity: what names a session?
- Context: what is included, who decides, and how is it refreshed?
- Tools: how are permissions and side effects represented?
- Continuation: how does a session resume after interruption or compaction?
- Surface parity: which concepts appear in CLI, daemon, and UI?
- Output: how does the user review what happened?

The reduce-shaped outcome is a proposal to delete, rename, or promote concepts
inside loopflow. "Codex does X" is not a proposal. "Loopflow carries two names
for one concept because Codex/OpenCode show the stable boundary is Y" is.

Status: first pass recorded in `wave/reduce/analysis/session-model-comparison.md`.
Draft proposal: `wave/reduce/proposals/session-record-spine.md`.
