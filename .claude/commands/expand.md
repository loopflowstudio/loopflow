---
requires: diff vs main
produces: .design/<expansion>.md
---
Explore ambitious changes that extend what this branch is already doing.

## Goal

Sketch a possibility for the human to evaluate. Write a design doc they can read, edit, pursue, or discard. Don't over-commit—this is exploration. If the idea is good, it becomes its own branch. If not, delete the doc and move on.

This is for bigger swings—but grounded in the current work. Look at what this branch changes, then ask: what's the natural next step that would multiply the value?

## Workflow

1. Run `git diff main...HEAD` to see what this branch has changed
2. Read the modified files to understand the current direction
3. Identify one expansion that extends this work meaningfully
4. Write a design doc to `.design/<expansion-name>.md`
5. If the expansion is tractable, start building it

## What makes a good expansion

**Extends the branch's intent.** If this branch adds worktree support, expand might add parallel worktree execution. If it improves commit messages, expand might add PR description generation. The expansion should feel like "part 2" of what's already here.

**Multiplicative value.** Changes that make everything else better, not just add one more thing. A new abstraction that simplifies three existing features beats a fourth feature.

**Tractable scope.** Ambitious but achievable in one session. If it needs a design doc longer than 500 words, it's too big for expand.

## Loopflow's architectural beliefs

When expanding, stay aligned with these principles:

- **Worktrees over branches.** Agents run in isolated worktrees so you can work in parallel. Expansions should assume multi-worktree workflows.
- **Prompts are files.** Tasks live in `.claude/commands/` or `.lf/`. Don't add config-driven prompt generation or template engines.
- **Auto mode is default.** Most tasks run headless with `--print`. Interactive is the exception. Design for unattended execution.
- **CLI passthrough.** Loopflow wraps Claude Code and Codex CLIs. Don't reimplement their features—pass args through.
- **Design docs are scaffolding.** `.design/` is for session recovery, not permanent documentation. `lf ops pr land` deletes it.
- **Simple data, simple APIs.** Prefer dataclasses over complex hierarchies. Prefer functions over classes when state isn't needed.

## What to avoid

**Unrelated improvements.** The expansion must connect to what this branch is doing. Generic "wouldn't it be nice" ideas belong in a separate branch.

**Abstraction layers.** If you're tempted to add a plugin system, registry pattern, or config-driven dispatch, that's a sign to stop and reconsider.

**Trend-chasing.** The project should be better at being itself, not more like whatever's popular this month.

