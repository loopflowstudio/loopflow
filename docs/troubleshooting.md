# Troubleshooting

Each section: symptom, cause, fix. Commands are complete and runnable as
written.

## A Wave is not running

**Symptom:** The app or `lf ls` shows the Wave stopped; `lf chat` reports no
listener.

**Cause:** No resident process is serving the Wave — nothing starts one
automatically except the app, `lf start`, or a cron wake.

```bash
lf status <wave> --json    # current registry + runtime evidence
lf home probe <wave>       # reachable? stopped? running? — with the next action
lf start <wave>            # idempotently start the Wave on this machine
```

## Task Work stops advancing

**Symptom:** A Task is still `ready`, but no useful work is advancing or its
provider process stopped.

Read its durable state before restarting anything:

```bash
lf task status INF-123 --json
lf session list
```

Answer an exact pending question, send unsolicited durable direction through
Steer, or resume a stopped process through the same Task Work:

```bash
lf session open <session-id>
lf session complete <interactive-or-ask-id>
lf session approve <flowstep-id> "Verified summary"
lf session iterate <flowstep-id> "Narrow the design"
lf task steer INF-123 "address the latest feedback"
lf task interrupt INF-123
lf task resume INF-123
lf task resume INF-123 --model codex --reason "Claude quota exhausted"
```

Plain `resume` continues the same provider transcript. `--model` keeps the Task
Work, Steers, worktree, and active PR, but gives the next attempt to the selected
agent. It refuses while another executor is still writing. A Task Steer is a
durable Work comment. A live controller offers new comments to the provider at
turn boundaries; the next Skill seed remains the fallback. `task interrupt`
ends the active turn so that boundary arrives immediately. Neither command's
receipt proves that the provider applied the direction.

During new-Task placement, status reports the declared worktree as initializing.
If creation does not finish, status keeps the Task identity and names the exact
path and branch to restore before resuming.

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
lf wt prune --dry-run    # show clean terminal or week-stale worktrees
lf wt prune              # remove those worktrees and their branches
```

Prune always preserves uncommitted files. Without terminal evidence, an open PR
or branch activity in the last seven days also prevents cleanup. Use
`lf wt remove NAME --force` only when intentionally discarding a worktree.

Feature-worktree integration fetches and pins `origin/<default>` without
moving the default-branch checkout:

```bash
lf rebase
```

The feature branch uses the current remote base even when the sibling default
checkout has not moved.

## Status says `ready`, but the Task is waiting

**Symptom:** Project or Task Work is `ready`, while its condition says it is
waiting on a child, human FlowStep, CI, or merge.

Work status is deliberately small: `ready`, `done`, or `abandoned`. Task
condition summarizes process liveness, human FlowStep, child progress, CI, and
merge evidence; unresolved human conversations appear under Sessions.
Inspect the focused projection instead of inferring a control state from one
field:

```bash
lf status <wave> --json
lf project status <project-id> --json
lf task status INF-123 --json
```

Resolve the named fact: open the human session, inspect the child, repair CI, merge, or
resume the provider. There is no Run slot or PR-limit counter to clear.

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
