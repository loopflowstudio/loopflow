# Telemetry

Your agents have been working: launched by hand, running Tasks, landing PRs.
Every run left a record on this machine. This page reads it back: what each
conversation cost, what each command did, what is running right now, and
whether Loopflow's own records are sound. Desktop's Telemetry view shows the
same records; it keeps no copy of its own.

Missing evidence stays missing. Usage a provider never reported reads as `-`,
never as zero, and a process with no exit record is unknown, not still running.

## What it cost

```bash
lf session usage --task LOO-12          # every conversation that worked on one Task
lf session usage --days 0 --wave infra  # all time, for one Wave
```

Each row is one Session's recorded input: input, output and cache-read tokens,
cost, and how many of its provider streams finished (`FINAL`). `GAPS` counts the
places the record is incomplete. The window defaults to the last 30 days.

Find where the tokens went:

```bash
lf session usage --task LOO-12 --context   # each step's input, broken down by source, against your budgets
lf session usage --weekly                  # context cost and turn time, week by week
```

`--context` splits each step's submitted input into its sources (instructions,
memory, scratch, goal, steers, carried conversation, tool output) and marks the
ones over budget with `*`. The
weekly report shows the trend, then breaks the latest week down by step, harness
and Task, and shows how turn time grows as context grows. `lf context` shows
each budget, where it was set and current usage;
[configuration](config.md#context-budgets) says how to change them.

## What happened

```bash
lf session history --task LOO-12                  # what its Sessions did: input, turns, outcomes
lf session history list --task LOO-12             # every command that ran for it, newest first
lf session history list --search "land" --all     # one command, across every repository
lf session history show <id> --final              # what the agent concluded
lf session history show <id> --context            # what its input was made of
lf session history show <id> --transcript         # the conversation, as its providers kept it
lf session history search "cleanupPeriodDays" --since 30d   # text across recent conversations
lf session retention                              # what providers still keep, and what goes next
```

`lf session history` orders what Sessions did — input captured, provider turns,
recorded outcomes — each tagged with its Task or Wave. Changes no Session made
(PRs, Steers, Workflow moves, sync) are on the Task: `lf task status`.
`session history list` is the command record underneath it, one row per lf
process, with its parent and outcome. Narrow it with `--parent`,
`--caller SESSION` or `--outcome`.

`session history show` opens any process or Session by ID. `--events` prints a
Session's raw input events, including which account it actually ran on.
`--instructions` explains which instructions were included and which were left out.

Conversation text stays with its provider: `session history show` reads it from Claude,
Codex or Pi transcript files or `opencode session export`, and lf stores none of
it. When a transcript is gone, it prints the recorded turns (time, status, final
answer) and why: Claude removes conversations idle longer than
`cleanupPeriodDays` (30 days unless set in its `settings.json`); Codex, OpenCode
and Pi keep everything. Raise `cleanupPeriodDays` to keep more Claude history.

Replay a captured request as new work, with the same prompt, model, account and
permissions; the original record is left alone:

```bash
lf replay <run-id>
```

## What's running now

```bash
lf process top                  # live call trees, refreshing; the first move when work feels slow
lf process list --json          # one parseable snapshot, for another tool or agent
lf process prune --dry-run      # receipts, orphaned providers and Process rows safe to remove
```

Both show only processes the OS says are alive; finished calls drop out and move
to `lf session history`. `lf process prune` removes stale receipts and provider
processes whose lf parent is provably dead, never an unclaimed provider, then
forgets finished Process rows nothing references and compacts the store.

## How big the codebase is

```bash
lf repository tokens            # lines and tokens per directory, tracked files only
lf repository tokens --days 90  # the codebase's size on each day it changed
```

Tokens are counted with the same tokenizer the prompt assembler budgets with,
so this tree and your context budgets measure the same thing. A lockfile is
cheap in lines and expensive in tokens; this shows it.

## Is Loopflow healthy

```bash
lf machine doctor             # store, process records, lineage, attribution, installation, lock contention
lf machine doctor --planning  # repository planning, read without changing it
```

Each check reports ok, warn or fail with its detail. `--json` returns the same audit.

## Reference

| Command | What it does |
|---|---|
| `lf session usage [--task\|--wave] [--days N]` | Recorded provider usage per Session |
| `lf session usage --context` / `--weekly` | Input by source / context cost and turn time by week |
| `lf session usage --parent SESSION` | Usage of the inputs one Session issued |
| `lf session history [--task\|--wave]` | What Sessions did, newest first |
| `lf session history list` | Recorded commands; filter by Task, Wave, parent, caller, text, outcome |
| `lf session history show ID` | One process or Session; `--final`, `--context`, `--transcript`, `--events`, `--instructions` |
| `lf session history search TEXT` / `lf session retention` | Find text in recent conversations / what providers still keep |
| `lf replay ID` | New work from a captured request |
| `lf process list`, `lf process top` | Live process trees |
| `lf process prune` | Remove dead receipts, orphaned providers and finished Process rows |
| `lf repository tokens [--days N]` | Codebase size in lines and tokens |
| `lf machine doctor [--planning]` | Installation, storage and process integrity |

`lf usage`, `lf history` and `lf doctor` resolve to the same commands. Every
flag is in the [`lf` reference](lf-reference.md#lf-session).

Next: [Orchestration](orchestration.md): run Graphs on other machines and agents, and customize the built-in ones.
