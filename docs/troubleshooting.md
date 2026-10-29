# Troubleshooting

Something stopped. Three questions, in order: what survived, what went wrong,
what to run next. Every command here is complete and runnable as written.

## What survives

A crash, a reboot or a closed terminal loses less than it looks like.

```bash
lf resume --all                  # every conversation a crash or restart interrupted; never one you left
```

| Survives | Where it lives | Pick it up with |
|---|---|---|
| Conversations, interactive and headless | `~/.lf/loopflow.db` and the provider's own history | `lf resume --all`, `lf resume --task <ID>` |
| A Task's worktree, branch, PR and comments | Beside your repository, on the code host | `lf task run <ID>` |
| A stopped Graph | Its step Processes, as history | `lf graph list --processes --for-task <ID>` |
| Uncommitted files | The worktree; prune never removes them | `lf wt list` |

A stopped Graph never resumes. `lf task run` starts a fresh one in the same
checkout, with the Task's comments and its open PR.

## What went wrong

Read before you relaunch anything.

```bash
lf doctor                                    # installation, store and process health
lf task status <ID>                          # the Task's execution and the reason for it
lf graph show <GRAPH_ID> --processes         # each step, its Process and how it exited
lf history show <SESSION_ID> --final         # the agent's last word in that conversation
lf ps                                        # what is running right now
```

`lf task status` reports execution separately from the Task's own status:
starting, running, stalled, blocked, idle or unknown, with its reason. Unknown
liveness stays unknown; lf never guesses a process is dead.

## What to run next

| You see | Run |
|---|---|
| A question waiting for you | `lf resume --task <ID>` |
| The run is going the wrong way | `lf comment <ID> "address the latest feedback"` |
| It must stop now | `lf interrupt <ID>` |
| It stopped and should go again | `lf task run <ID> --reason "provider credentials repaired"` |

## Doctor reports a failure

```bash
lf doctor --json
```

Doctor checks installation selection, store compatibility and Process integrity
separately. It reads storage without initializing it or applying migrations, so
an incompatible store still reports readable Process evidence.

Binary freshness compares against locally cached `origin/main`, without
fetching. That reference may be stale, and merged source may be newer than the
latest published release. `lf install` installs that published release; Doctor
itself installs nothing.

Contention reads the last day of `logs/lf.log`: `database is locked` failures
and any write that held the store lock for 3 s or more, grouped by the lf
binary that logged them, with the holder's pids and the function that held the
lock. Lines from binaries older than this check group as `unidentified (older lf)`.

## A Task stopped advancing

**Symptom:** the Task is not done, but nothing is moving, or its agent
exited.

```bash
lf task status <ID> --json
lf usage --days 0 --task <ID> --json        # every Session input the Task ever made
lf session list --task <ID>
```

Dirty files under a live Graph are ongoing progress. An idle Task Graph does not
prove helper Sessions are idle, and a finished launcher does not prove its
interactive Session closed: check Sessions separately.

`lf interrupt <ID>` stops a running Graph: an agent step ends at once, an
operation finishes and nothing follows, and the run exits 130. With nothing
live it records nothing and says so.

If the Task's worktree never finished being created, status keeps the Task and
names the exact path and branch to restore before launching.

## The Task is idle, but waiting on something

Task state says what the Task is doing (`idle`, `active`, `moving`, `shipping`,
…); whether anyone owns it is separate (`lf task claim`, `lf task unassign`). What it is waiting on (a child Task, review, CI, merge) is its
`reason`; Desktop's condition, with the evidence behind it, is in `lf desktop task`:

```bash
lf task status <ID> --json
lf desktop task <ID> --json
```

Resolve the named fact: open the conversation, inspect the child, repair CI,
merge. There is no run slot or PR-limit counter to clear.

## Rate limits

**Symptom:** runs fail with rate-limit or capacity errors.

Headless runs retry transient capacity, rate-limit, availability and transport
failures four times, continuing the same provider conversation so partial work
survives; the backoff tops out at 30 seconds.

An exhausted subscription takes a different path: lf marks that login
unavailable until its reported reset and moves to your next login with room.

```bash
lf account                           # every login, its capacity and when it resets
lf gate -a codex                     # run this one on another harness
lf --only-account claude=work@ gate  # stay inside the logins you name
```

`--account` prefers a login and still falls back; `--only-account` never leaves
the logins it names.

## Agent not found

**Symptom:** `'claude' is not installed.`

The error names the install command. For Claude Code:

```bash
npm install -g @anthropic-ai/claude-code
lf account connect claude
```

With no `agent` configured, lf uses the first of Codex, Claude Code, OpenCode
and Pi it finds, so one is enough.

## Context too large

**Symptom:** a run fails on context or token limits.

```bash
lf --context qa --docs src/small/    # see what the input is made of, launch nothing
lf usage --context --task <ID>       # each step's input by source, against its budget
```

The provider loads `AGENTS.md` natively. lf adds `LOOPFLOW.md`, `scratch/` and
`.lf/wave/`, plus only what you pass with `--docs`: narrow the path or glob to
shrink it. Persistent docs belong in `docs:` in `.lf/config.yaml`.

## Disk filling up

```bash
scripts/disk-cleanup.sh            # reclaim now; each removal is logged to ~/.lf/logs/disk.log
scripts/disk-cleanup.sh install    # run the disk-cleanup skill hourly from crontab
lf monitor prune                   # only lf's records: dead Process rows and receipts, then vacuum
```

The script prunes every repository under `~/src`: finished worktrees, completed
Tasks' build output and merged branches through `lf wt prune`, and ignored
build output (`target`, `node_modules`, `.venv`, `.build`, `DerivedData`) that no process
holds and nothing wrote for 4 hours. In `~/.lf` it removes captures, logs
and every database backup but the newest once untouched for a week. The
database, account homes, Home material written in the last day and anything a
process holds open are never removed; when `lsof` fails, neither is anything else
that check protects.

Under cron, the `disk-cleanup` skill runs the script and reports what it freed,
what it kept and why, and worktrees still holding unpushed work; it deletes
nothing itself. Edit `scripts/disk-cleanup.sh` or
`.lf/skills/disk-cleanup.md` to change the policy. Provider transcripts follow
each provider's own retention; raise Claude's `cleanupPeriodDays` to keep more.

## Stale worktrees and branches

```bash
lf wt prune --dry-run              # finished worktrees
lf wt prune                        # remove them and their branches
lf wt prune --branches --dry-run   # finished branches on origin and locally, with reasons
```

Worktree pruning requires a merged or closed PR at the exact local head.
Unpublished commits survive even when their remote branch is absent or their
checkout has been idle for weeks. It removes only clean, unprotected checkouts
and their local branch refs.

With `--branches`, a branch goes only with evidence at its exact head: a merged or closed PR, a
finished Task already in the default branch, or a release candidate whose tag
exists. An open PR, a worktree or an unfinished Task always keeps it. Prune
always preserves uncommitted files; `lf wt delete <name> --force` is for when
you mean to discard one.

A branch behind `main`:

```bash
lf sync                            # refresh main, keeping its unpublished commits, then merge it in
```

## Orphaned agent processes

```bash
lf monitor prune --dry-run --json  # each candidate and why
lf monitor prune --json            # recheck, then reap
```

Pruning reports each agent process it settles or keeps, with the reason, then
closes Process rows whose exit is proven, forgets finished ones nothing
references, and compacts the store; a dry run changes none of it. A
missing PID, a completed Task or an old Session does not prove the agent
exited: unresolved records keep protecting their checkouts.

When `lf self doctor` fails `lineage` and says `lf monitor prune` repairs the
parents, an AgentProcess points at an lf process that was never recorded:

```bash
lf monitor prune --dry-run         # lists the ancestry it would release
lf monitor prune                   # releases it; the Session can resume
```

The release is kept as a Session fact in `lf history show <session>`.

## A Wave isn't moving

Wave passes are finite; nothing runs between them unless you schedule one.

```bash
lf wave status <wave> --json
lf --wave <wave> wave-operate      # one pass, now
crontab -l                         # passes you scheduled yourself
```

A quiet Wave needs no service restart.

Next: the [`lf` reference](lf-reference.md) for every command and flag.
