# Troubleshooting

Each section: symptom, cause, fix. Commands are complete and runnable as
written.

## Doctor reports a failure

```bash
lf doctor
lf cron list
lf cron history --wave infrastructure --flow telemetry-daily --days 2
lf cron sync --wave infrastructure
```

Doctor checks installation selection, store compatibility, Process integrity and
scheduled receipts separately. Installation commands can have no repository; a
recorded repository must be an absolute path. Missing scheduled receipts remain
failures even when ordinary commands work. A receipt proves invocation, not
successful completion of its Flow or Skill.

For a missing receipt, inspect the executable and log path Doctor prints. Jobs
installed before the stable entry gate may pin a superseded binary, which a
later install removes, and fail before recording a receipt. After installing a release with this repair,
run `lf cron sync --wave <wave>` from that Wave's repository to refresh its jobs;
use `lf cron sync --repo` for repository Task checks. New scheduled invocations
follow the selected installation across promotions. Existing Sessions
retain their runtime ownership.

Doctor reads storage without initializing it or applying migrations. An incompatible
store still reports readable Process evidence, scheduler obligations and installation
selection.

Binary freshness compares against locally cached `origin/main`, without fetching.
That reference may be stale, and merged source may be newer than the latest
published release. `lf install` installs that published release; Doctor
itself installs nothing.

## A Wave has no active conversation

Wave operations are finite. Read its plan and invoke the next pass explicitly,
or inspect its cron schedule:

```bash
lf wave status <wave> --json
lf --wave <wave> wave-operate
lf cron list
```

A quiet Wave needs no service restart.

## Task Work stops advancing

**Symptom:** A Task is still `ready`, but no useful work is advancing or its
provider process stopped.

Read its durable state before launching anything:

```bash
lf task status INF-123 --json
lf usage --days 0 --task INF-123 --json
lf session list
```

`ready` means the Task is nonterminal. Status reports `execution` separately:
starting, running, stalled, blocked, idle, or unknown. It observes the Task's
most recently launched Flow. Read its reason before recovery. Wave status and
roadmap use that same execution evidence for their recommendations. Dirty files
under a live Flow are ongoing progress.

Task execution history includes independent helpers, whether recorded with the public
issue identifier or internal Task ID. An idle Task Flow does not prove those
helpers are idle; a completed launcher does not prove its interactive Session
is closed. Inspect Sessions separately. Reserve bound helper conversations
for distinct contributions.

Wave status shows up to 50 recent Session inputs from the last seven days.
Read complete Task history with `lf usage --days 0 --task <task> --json`, or inspect
one input with `lf mon show <session> --input <capture>`. An empty recent list
does not prove that no Flow or Session remains active.

Answer an exact pending question, send durable direction through Steer, or
launch fresh work for the same Task:

```bash
lf session connect <session-id>
lf comment INF-123 "address the latest feedback"
lf interrupt INF-123
lf task run INF-123
lf task run INF-123 --reason "provider credentials repaired"
```

`task run` runs a fresh Flow in the Task checkout with its Steers and
active PR. It never continues an earlier Flow; a stopped one leaves its
Processes as history. Unknown liveness stays
unknown. A Task Steer is a Linear Task comment; the running Flow receives new
comments when possible and the next Skill seed always reads them. `task interrupt` ends the
active boundary so the next one re-reads direction. Neither command's receipt
proves that the provider applied the direction.

During new-Task placement, status reports the declared worktree as initializing.
If creation does not finish, status keeps the Task identity and names the exact
path and branch to restore before launching.

## Rate limits

**Symptom:** Tasks fail with rate limit errors.

One-shot headless runs retry transient capacity, rate-limit, availability, and
transport failures four times. Codex and Claude continue the same provider
session, preserving partial work; the backoff ladder tops out at 30 seconds.

Managed-account subscription exhaustion takes a different path: Loopflow marks
the account unavailable until its reported reset and immediately tries the next
account in the grant. `--account` retains the normal route as fallback;
`--only-account` stays inside the accounts it names.

After repairing provider access, launch the Task or Project operation again:

```bash
lf task run INF-123 --reason "provider credentials repaired"
```

Wave planning uses finite conversations. Invoke `lf --wave <wave> wave-operate`
for another planning pass; there is no long-running Project process to resume.

Other options:

- Wait and retry
- Reduce parallel waves
- Switch a one-shot flow to a different model: `lf gate -a codex`

## Worktree issues

**Symptom:** Git worktree commands fail or show stale data.

Preview cleanup before removing settled checkouts:

```bash
lf wt list
lf wt prune --dry-run --json           # explain removal and retention decisions
lf wt prune                           # remove eligible checkouts and exact-head local refs
```

```bash
lf wt timing                           # count, median, p95 and failures per lf version
lf wt timing --json
```

Every `lf wt list` appends its durations to `~/.lf/perf/wt-list.jsonl`: total,
startup (launch through the start receipt), local Git, the remote call, and the
time spent writing Process receipts to SQLite. Local Git and the remote overlap.
Interrupted runs are recorded; a run killed outright is not. The file holds
durations, counts, the repository root and the `lf` version, and never more
than 1,000 samples.

Prune requires Loopflow ownership and a recorded settlement of the current head.
It preserves primary and persistent checkouts, unfinished Tasks, live or unknown
execution, uncommitted files and unclassified ignored data. Age, a closed PR and a
missing remote branch are not deletion authority. Completed Tasks retain PR-less
checkouts and checkouts with unresolved delivery or follow-through, even if a
separate landing records a merge. Only wholly ignored directories
with a valid `CACHEDIR.TAG` are disposable; a familiar name such as `target` or
`.venv` is not enough. Unknown size estimates are `null`, not zero.

Repository reconciliation (`lf task reconcile`) retries deferred cleanup locally,
even if delivery observation fails. Installed releases enable the repository tick on
first work and repair its executable on later work. Explicit disable survives those
repairs. Without launchd, work-producing commands start at most one fallback check
per minute. Cleanup leaves
remote branches and Task outcomes intact and retains recognized local Session
evidence, including registered provider homes inside declared caches. Full referenced-payload protection remains unfinished. Use `lf wt delete NAME --force` only when deliberately
discarding a checkout.

```bash
lf sync
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
