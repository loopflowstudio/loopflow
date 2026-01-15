# Agents & Voices

Voice picker in repo window + dedicated Agents window for background agent management.

## Review

**Verdict:** Ready to ship

Code is clean, matches design doc intent, no bugs found. A few minor style observations that aren't blockers.

### Minor items

**pyproject.toml adds duplicate dev deps** (`pyproject.toml:66-70`) — `[dependency-groups] dev` adds `fastapi>=0.128.0` and `pytest>=9.0.2`, but `[project.optional-dependencies] dev` already has `pytest>=7.0.0`. The `fastapi` dep appears unused—nothing imports it. Delete the `[dependency-groups]` section unless there's a reason for it.

**Context paths UI has no way to add** (`AgentDetailPanel.swift:172-204`) — You can remove context paths but not add new ones. The design doc shows chips with a way to add. Not blocking since agents can still define context in the markdown file directly.

## Design notes

**Voices are worktree-scoped.** They live in `.lf/voices/{name}.md` and get passed via `--voice name1,name2`.

**Agents are global.** They live in `~/.lf/agents/*.md` with YAML frontmatter. Runtime state comes from `~/.lf/maestro.db`.

**Trigger types:**
- `manual` — only runs when explicitly started
- `main-changed` — runs when origin/main has new commits
- `loop` — runs again immediately after completion (backend stub added in `triggers.py`)
- `cron` — scheduled runs (UI ready, backend not implemented)

**Deferred work** (captured in `.design/questions.md`):
1. Loop trigger execution logic
2. Pipeline storage for agents
3. Auto-close worktrees after PR merge
4. Goal file watching for re-triggers
5. Voice creation UI
6. Pipeline visualization
