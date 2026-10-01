# Troubleshooting

Each section: symptom, cause, fix. Commands are complete and runnable as
written.

## A Wave has no active conversation

Wave operations are finite. Read its plan and invoke the next pass explicitly,
or inspect its cron schedule:

```bash
lf wave status <wave> --json
lf --wave <wave> wave/operate
lf cron list
```

A quiet Wave needs no service restart.

## Task Work stops advancing

**Symptom:** A Task is still `ready`, but no useful work is advancing or its
provider process stopped.

Read its durable state before restarting anything:

```bash
lf task status INF-123 --json
lf usage --days 0 --task INF-123 --json
lf session list
```

`ready` means the Task is nonterminal. Status reports `execution` separately:
starting, running, waiting for review, blocked, idle, or unknown. Read its
reason and selected execution before recovery. Wave status and roadmap use that same
execution evidence for their recommendations. Dirty files under a live worker
are ongoing progress.

Task execution history includes independent helpers, whether recorded with the public
issue identifier or internal Task ID. An idle Task Flow does not prove those
helpers are idle; a completed launcher does not prove its interactive Session
is closed. Inspect Sessions separately. Recover advancement through Task
controls; reserve bound helper conversations for distinct contributions.

Task status and `lf monitor list` show up to 50 Runs started in the last seven days.
Inspect an exact Run ID for older evidence; an empty recent list does not prove
that no worker or Session remains active.

Answer an exact pending question, send unsolicited durable direction through
Steer, or resume a stopped process through the same Task Work:

```bash
lf session connect <session-id>
lf session complete <session-id>       # return saved review feedback
lf comment INF-123 "address the latest feedback"
lf interrupt INF-123
lf --task INF-123 flow start
lf --task INF-123 flow start --reason "provider credentials repaired"
```

`flow start` continues the captured Flow with the Task Work, Steers, worktree, and
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

One-shot headless runs retry transient capacity, rate-limit, availability, and
transport failures four times. Codex and Claude continue the same provider
session, preserving partial work; the backoff ladder tops out at 30 seconds.

Managed-account subscription exhaustion takes a different path: Loopflow marks
the account unavailable until its reported reset and immediately tries the next
account in the grant. `--account` retains the normal route as fallback;
`--only-account` stays inside the accounts it names.

After repairing provider access, retry the Task or Project operation:

```bash
lf --task INF-123 flow start --reason "provider credentials repaired"
```

Wave planning uses finite conversations. Invoke `lf --wave <wave> wave/operate`
for another planning pass; it has no resident Project process to resume.

Other options:

- Wait and retry
- Reduce parallel waves
- Switch a one-shot flow to a different model: `lf gate -m codex`

## Worktree issues

**Symptom:** Git worktree commands fail or show stale data.

List all worktrees, then clean up stale entries:

```bash
lf wt list
lf wt prune --dry-run                  # show clean terminal or week-stale worktrees
lf wt prune                            # remove those worktrees and their branches
```

Prune always preserves uncommitted files. Without terminal evidence, an open PR
or branch activity in the last seven days also prevents cleanup. Use
`lf wt delete NAME --force` only when intentionally discarding a worktree.

```bash
lf task sync
```

Refreshes the local default branch, preserving its unpublished commits and edits,
then merges it into the feature branch. Stacked Tasks merge their live parent
until it lands, then merge the default branch using their recorded fork.

## Status says `ready`, but the Task is waiting

**Symptom:** Project or Task Work is `ready`, while its condition says it is
waiting on a child, review FlowStep, CI, or merge.

Work status is deliberately small: `ready`, `done`, or `abandoned`. Task
condition summarizes process liveness, review FlowStep, child progress, CI, and
merge evidence; unresolved conversations appear under Sessions.
Inspect the focused projection instead of inferring a control state from one
field:

```bash
lf wave status <wave> --json
lf task status INF-123 --json
```

Resolve the named fact: open the session, inspect the child, repair CI, merge, or
resume the provider. There is no Run slot or PR-limit counter to clear.

## Context too large

**Symptom:** Task fails with context/token limit errors.

The provider loads `AGENTS.md` natively. Loopflow adds `LOOPFLOW.md`, `scratch/`, and `wave/`. Reduce further:

```bash
lf qa --no-loopflow                    # skip LOOPFLOW.md
lf qa --docs src/small/                # limit --docs to a narrower path or glob
```

`--docs` only adds what you pass—drop paths or narrow globs to shrink it further.

For persistent docs, set `docs:` in `.lf/config.yaml`.

See [Configuration](config.md) for context options.

## Claude Code not found

**Symptom:** `lf` fails with "'claude' is not installed" or similar.

Loopflow drives an AI coding tool and needs one installed. The error names
the install command. For Claude Code:

```bash
npm install -g @anthropic-ai/claude-code
lf account connect claude
```

With no `agent` configured, Loopflow uses the first of Codex, Claude Code, and
OpenCode it finds, so installing any one of them is enough.

## See Also

[Configuration](config.md) · [Waves](waves.md)
