---
layout: default
title: Troubleshooting
---

# Troubleshooting

Common issues and solutions.

## A Wave is not running

The Loopflow app invokes its bundled `lf` and talks directly to each Wave.
Inspect the selected Wave, then start it in the foreground:

```bash
lf status <wave> --json
lf wave <wave>
```

## Task Session stops advancing

**Symptom:** A Task remains waiting, blocked, failed, or submitted without an
obvious next action.

Read its durable state before restarting anything:

```bash
lf task status INF-123 --json
lf task attach INF-123
```

`attach` opens the audited Task control prompt; it does not write bytes into a
provider terminal. Answer a pending decision, send a follow-up, or resume a
stopped process through the same Task Session:

```bash
lf task decide INF-123 cd_123 approve
lf task follow-up INF-123 "address the latest review"
lf task resume INF-123 "continue from the failure"
lf task resume INF-123 --model codex --reason "Claude quota exhausted"
```

Plain `resume` continues the same provider transcript. `--model` keeps the Task
Session, directive, worktree, and active PR, but gives the next body generation
to the selected agent. It refuses while another body is still writing; interrupt
that body first.

If a control returned a command id, inspect or wait for its durable receipt:

```bash
lf task receipt cc_123 --wait --timeout 30s --json
```

## Rate limits

**Symptom:** Tasks fail with rate limit errors.

Claude, Codex, Gemini, and OpenCode have usage limits. Resume the same Session
on another supported Session provider:

```bash
lf task resume INF-123 --model codex --reason "Claude quota exhausted"
lf project resume project-slug --model codex --reason "Claude quota exhausted"
```

Other options:

- Wait and retry
- Reduce parallel waves
- Switch a one-shot flow to a different model: `lf gate -m codex`

## Worktree issues

**Symptom:** Git worktree commands fail or show stale data.

List all worktrees:

```bash
lf wt list
```

Clean up stale entries:

```bash
lf wt prune
```

If the default branch looks stale after a PR operation you ran from a sibling worktree, rebase the current branch:

```bash
lf rebase
```

Loopflow updates the default-branch worktree as part of the rebase path.

## Project or Task is waiting

**Symptom:** Loopflow shows a Project or Task Session in `waiting`.

Waiting is deliberate: no provider process is running while a child or external
system must change the answer. Inspect the Wave's work map and the child's
state reason:

```bash
lf status <wave> --json
lf project status <project-id> --json
lf task status INF-123 --json
```

Typical owners are a pending decision, an active child Task, PR review, CI, or
merge. Steer, decide, or resume the named Project or Task. A relevant child
observation wakes its Project Session automatically; there is no runtime knob
or PR-limit counter to clear.

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

If an agent CLI is missing, install that vendor's CLI and rerun `lf init`.

## See Also

[Configuration](config.md) · [Waves](waves.md)
