# Troubleshooting

Read this when the Mac app shows work as stopped, waiting, or failed. Check
its current state before restarting it: unfinished work may already have an
active worker or be waiting for a review.

The commands below run in a shell. Replace `<wave>` with a Wave name,
`INF-123` with a Linear Task identifier, and `<session-id>` with an identifier
from `lf session list`. A [Wave](glossary.md#loopflows-words) keeps an objective;
a Task is one piece of work, a Run records one launch, and a Session is an open
conversation. These have separate states.

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
lf runs --task INF-123 --json
lf session list
```

`ready` means the Task is unfinished; it does not mean a worker is running. Status reports `execution` separately:
starting, running, waiting for review, blocked, idle, or unknown. Read its
reason and worker Run before recovery. Wave status and roadmap use that same
execution evidence for their recommendations. Uncommitted files under a live worker may be ongoing progress; do not restart it to clean them up.

Task Run history includes independent helpers, whether recorded with the public
issue identifier or internal Task ID. An idle Task Flow does not prove those
helpers are idle; a completed launcher does not prove its interactive Session
is closed. Inspect Sessions separately. Recover advancement through Task
controls; reserve bound helper Runs for distinct contributions.

Task status and `lf runs` show up to 50 Runs started in the last seven days.
Inspect an exact Run ID for older evidence; an empty recent list does not prove
that no worker or Session remains active.

Answer an exact pending question, send unsolicited durable direction through
Steer, or resume a stopped process through the same Task Work:

```bash
lf session open <session-id>
lf session complete <session-id>    # return saved review or Ask feedback
lf task steer INF-123 "address the latest feedback"
lf task interrupt INF-123
lf task resume INF-123
lf task resume INF-123 --reason "provider credentials repaired"
```

`resume` starts a fresh boundary from the Task Work, Steers, worktree, and
active PR. It refuses while another exact Task worker is live. A Task Steer
is a Linear Task comment; the active Task worker receives new comments when
possible and the next Skill seed always reads them. `task interrupt` ends the
active boundary so the next one re-reads direction. Neither command's receipt
proves that the provider applied the direction.

During new-Task placement, status reports the declared worktree as initializing.
If creation does not finish, status keeps the Task identity and names the exact
path and branch to restore before resuming.

## Rate limits

**Symptom:** Tasks fail with rate limit errors.

A [rate limit](glossary.md#borrowed-from-software-engineering) restricts how
many requests or how much usage an account can make. One-shot headless Runs
retry temporary capacity, rate-limit, availability, and connection failures
four times. Codex and Claude continue the same provider
session, preserving partial work; the backoff ladder tops out at 30 seconds.

Managed-account subscription exhaustion takes a different path: Loopflow marks
the account unavailable until its reported reset and immediately tries the next
permitted account. `--account` retains the normal route as fallback;
`--only-account` stays inside the accounts it names.

After repairing access to the coding tool, resume the Task:

```bash
lf task resume INF-123 --reason "provider credentials repaired"
```

For a stopped planning pass, rerun `lf --wave <wave> wave/operate` after
inspecting its previous Run. This starts a new planning pass; it does not
resume or replace a Task worker.

Other options:

- Wait and retry
- Reduce parallel waves
- Switch a one-shot flow to a different model: `lf gate -m codex`

## Worktree issues

A [worktree](glossary.md#borrowed-from-software-engineering) is a separate
checkout for edits on a branch. Keeping Task work apart lets several Tasks
change files without sharing the same working copy.

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

To fit a branch onto its current base, inspect the rebase plan and apply it:

```bash
lf rebase --plan
lf rebase
```

[Rebase](glossary.md#borrowed-from-software-engineering) replays a branch's
changes on its base. The plan is read-only; applying it may publish the updated
branch. See [`lf rebase`](lf.md#lf-rebase) for conflict recovery.

## Status says `ready`, but the Task is waiting

**Symptom:** Project or Task Work is `ready`, while its condition says it is
waiting on a child, review FlowStep, CI, or merge.

Work status is deliberately small: `ready`, `done`, or `abandoned`. Task
condition summarizes process liveness, review FlowStep, child progress, CI, and
merge evidence; unresolved conversations appear under Sessions.
Read the explanation alongside the status instead of deciding from one field:

```bash
lf status <wave> --json
lf task status INF-123 --json
```

Resolve the named fact: open the session, inspect the child, repair CI, merge, or
resume the provider. There is no Run slot or PR-limit counter to clear.

## Context too large

**Symptom:** Task fails with context/token limit errors.

[Context](glossary.md#loopflows-words) is the information supplied to the AI.
The model has an input limit measured in tokens, units of text. By default,
Loopflow includes the agent guide (CLAUDE.md/AGENTS.md), `LOOPFLOW.md`,
`scratch/`, and `wave/`. Large working notes can still exceed the limit.
Remove unneeded additions or shorten notes while keeping decisions and evidence:

```bash
lf qa --no-loopflow         # skip LOOPFLOW.md
lf qa --docs src/small/     # limit --docs to a narrower path or glob
```

`--docs` adds files; it does not exclude default context or clear configured
lists. A [glob](glossary.md#borrowed-from-software-engineering) is a filename
pattern. Narrow patterns and remove unneeded `docs:` entries from repository
and personal config to reduce those additions.

For persistent docs, set `docs:` in `.lf/config.yaml`.

See [Configuration](config.md) for context options.

## Claude Code not found

**Symptom:** `lf` fails with "'claude' is not installed" or similar.

Loopflow drives an AI coding tool and needs one installed. The error names
the install command. For Claude Code:

```bash
npm install -g @anthropic-ai/claude-code
lf auth claude
```

With no `agent` configured, Loopflow uses the first of Codex, Claude Code, and
OpenCode it finds, so installing any one of them is enough.

## See Also

[Configuration](config.md) · [Waves](waves.md)
