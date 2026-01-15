# Agents & Voices

Voice picker in repo window + dedicated Agents window for background agent management.

## Review

**Verdict:** Ready to ship

No blocking issues. Implementation follows existing patterns. Tests cover cron trigger logic.

## Design notes

**Voices are worktree-scoped.** Live in `.lf/voices/{name}.md`, passed via `--voice name1,name2`.

**Agents are global.** Live in `~/.lf/agents/*.md` with YAML frontmatter. Runtime state from `~/.lf/maestro.db`.

**Trigger types:**
- `manual` — only runs when explicitly started
- `main-changed` — runs when origin/main has new commits
- `loop` — runs again immediately after completion
- `cron` — scheduled runs (5-field: minute hour day month weekday)

**Merge strategies:**
- `pr` — open PR for human review
- `auto` — auto-merge when pipeline succeeds

## Deferred

1. Auto-close worktrees after PR merge
2. Voice creation UI
