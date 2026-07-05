---
layout: default
title: Troubleshooting
---

# Troubleshooting

Common issues and solutions.

## lfd daemon not running

**Symptom:** Concerto shows no daemon, or `curl http://127.0.0.1:2486/health` fails.

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

## Task hangs in batch mode

**Symptom:** Task starts but never completes.

The task may be waiting for input. Use interactive mode:

```bash
lf gate -i    # interactive mode
```

Check if the coding agent is stuck on a permission prompt or clarifying question.

## Rate limits

**Symptom:** Tasks fail with rate limit errors.

Claude, Codex, Gemini, and OpenCode have usage limits. Options:

- Wait and retry
- Reduce parallel waves
- Switch to a different model: `lf gate -m codex`

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
lf op wt prune
```

If the default branch looks dirty after a sync or PR operation you ran from a sibling worktree, resync it explicitly:

```bash
lf op sync
```

Loopflow updates the checked-out default-branch worktree, not just the ref, and restores any dirty local edits afterward.
If restoring those edits conflicts, the stash is left in place for manual recovery.

## Loop stuck in WAITING

**Symptom:** Concerto shows a wave in WAITING state.

The loop hit its PR limit. Options:

1. Review and merge outstanding PRs
2. Adjust the wave runtime settings in Concerto
3. Land accumulated work: see [Waves](waves.md) for loop management

## Context too large

**Symptom:** Task fails with context/token limit errors.

The default context is already minimal: agent doc (CLAUDE.md/AGENTS.md), `LOOPFLOW.md`, `scratch/`, and `wave/`. Reduce further:

```bash
lf qa --no-loopflow         # skip LOOPFLOW.md
lf qa --docs src/small/     # limit --docs to a narrower path or glob
```

`--docs` only adds what you pass—drop paths or narrow globs to shrink it further.

For persistent docs, set `docs:` in `.lf/config.yaml`.

See [Configuration](config.md) for context options.

## Claude Code not found

**Symptom:** `lf` fails with "claude not found" or similar.

Run the setup wizard:

```bash
lf init
```

Check installation:

```bash
lf op doctor
```

## See Also

[Configuration](config.md) · [Waves](waves.md) · [`lfd` reference](lfd.md)
