# Agents & Voices

Voice picker in repo window + dedicated Agents window for background agent management.

## What shipped

**Voice picker in PromptLauncher** — Select voices from `.lf/voices/` via chip UI. Multiple voices combine. Voices pass to CLI via `--voice name1,name2`.

**Agents window** — Cmd+Shift+A opens global agent management. Sidebar lists agents from `~/.lf/agents/*.md`. Detail panel shows/edits goal, pipeline, trigger, merge strategy. Start/stop agents directly.

**Agent CRUD** — Create, edit, delete agents via UI. Changes write back to markdown files with YAML frontmatter.

**Runtime state from DB** — Agent status, iteration count, worktree path, and last run time come from `~/.lf/maestro.db`. UI refreshes on demand.

## Design notes

**Voices are worktree-scoped.** They live in `.lf/voices/{name}.md` and get passed via `--voice name1,name2`.

**Agents are global.** They live in `~/.lf/agents/*.md` with YAML frontmatter. Runtime state comes from `~/.lf/maestro.db`.

**Trigger types:**
- `manual` — only runs when explicitly started
- `main-changed` — runs when origin/main has new commits
- `loop` — runs again immediately after completion (backend stub in `triggers.py`)
- `cron` — scheduled runs (UI ready, backend not implemented)

**Merge strategies:**
- `pr` — open PR for human review
- `auto` — auto-merge when pipeline succeeds

## Deferred work

1. Loop trigger execution logic
2. Cron trigger backend
3. Auto-close worktrees after PR merge
4. Voice creation UI (currently: create `.md` files directly)
5. Context paths add button (currently: edit markdown file directly)
