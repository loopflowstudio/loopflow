---
layout: default
title: Troubleshooting
---

# Troubleshooting

Common issues and solutions.

## lfd daemon not running

**Symptom:** `lfd status` shows nothing or errors.

Check if installed:

```bash
launchctl list | grep lfd
```

Reinstall:

```bash
lfd install
```

Run in foreground to debug:

```bash
lfd serve
```

## Task hangs in auto mode

**Symptom:** Task starts but never completes.

The task may be waiting for input. Use interactive mode:

```bash
lf review -i    # interactive mode
```

Check if the agent is stuck on a permission prompt or clarifying question.

## Rate limits

**Symptom:** Tasks fail with rate limit errors.

Claude, Codex, and Gemini have usage limits. Options:

- Wait and retry
- Reduce parallel agents
- Switch to a different model: `lf review -m codex`

## Worktree issues

**Symptom:** Git worktree commands fail or show stale data.

List all worktrees:

```bash
git worktree list
```

Clean up stale entries:

```bash
git worktree prune
```

Remove merged worktrees:

```bash
lfops wt prune
```

## Loop stuck in WAITING

**Symptom:** `lfd status` shows a loop in WAITING state.

The loop hit its PR limit. Options:

1. Review and merge outstanding PRs
2. Increase the limit: `lfd loop <flow> <area> --limit 10`
3. Land accumulated work: see [Background Agents](agents.md) for loop management

## Context too large

**Symptom:** Task fails with context/token limit errors.

Reduce context:

```bash
lf review --no-lfdocs           # skip repo docs
lf review --no-diff-files       # skip branch files
lf review -p specific-file.py   # include only what you need
```

Use summaries for large codebases:

```bash
lfops summarize src/
```

See [Configuration](config.md) for context options.

## Claude Code not found

**Symptom:** `lf` fails with "claude not found" or similar.

Run the setup wizard:

```bash
lf init
```

Check installation:

```bash
lfops doctor
```

## See Also

[Configuration](config.md) · [Background Agents](agents.md) · [`lfd` reference](lfd.md)
