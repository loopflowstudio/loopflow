# Troubleshooting

Each section: symptom, cause, fix. Commands are complete and runnable as
written.

## A Wave is not running

**Symptom:** The app or `lf ls` shows the Wave stopped; `lf chat` reports no
listener.

**Cause:** No resident process is serving the Wave — nothing starts one
automatically except the app, `lf home start`, or a cron wake.

```bash
lf status <wave> --json    # current registry + runtime evidence
lf home probe <wave>       # reachable? stopped? running? — with the next action
lf home start <wave>       # idempotently start the Wave on its Home
```

## Task Session stops advancing

**Symptom:** A Task remains waiting, blocked, failed, or submitted without an
obvious next action.

Read its durable state before restarting anything:

```bash
lf task status INF-123 --json
lf queue
lf work review task task_...
```

The Review client does not write bytes into a provider terminal. Send direction
as a Steer, continue the flow, or resume a stopped process through the same
Task Work:

```bash
lf task steer INF-123 "address the latest review"
lf task interrupt INF-123
lf task resume INF-123
lf task resume INF-123 --model codex --reason "Claude quota exhausted"
```

Plain `resume` continues the same provider transcript. `--model` keeps the Task
Work, Steers, worktree, and active PR, but gives the next Launch to the selected
agent. It refuses while another executor is still writing; interrupt that
boundary first. A Steer is durable before live delivery is attempted. Provider
acceptance is not incorporation; the Basis of a later successful boundary is.

## Rate limits

**Symptom:** Tasks fail with rate limit errors.

One-shot headless runs retry transient capacity, rate-limit, availability, and
transport failures four times. Codex and Claude continue the same provider
session, preserving partial work; the backoff ladder tops out at 30 seconds.

Managed-account subscription exhaustion takes a different path: Loopflow marks
the account unavailable until its reported reset and immediately tries the next
account in the grant. `--account` retains the normal route as fallback;
`--only-account` stays inside the accounts it names.

If the retries exhaust for a managed Task, resume it on another provider:

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

List all worktrees, then clean up stale entries:

```bash
lf wt list
lf wt prune --dry-run    # show what would be removed
lf wt prune              # force-remove unprotected worktrees and their branches
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
