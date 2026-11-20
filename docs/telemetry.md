# Telemetry

Your agents have been working: launched by hand, running Tasks, landing PRs.
Every run left a record on this machine. This page reads it back: what each
conversation cost, what each command did, what is running right now, and
whether Loopflow's own records are sound. Desktop's Telemetry view shows the
same records; it keeps no copy of its own.

Missing evidence stays missing. Usage a provider never reported reads as `-`,
never as zero, and a process with no exit record is unknown, not still running.

## Cost and context budgets

```bash
lf session usage --task LOO-12            # every Session that worked on LOO-12
lf session usage --days 0 --wave infra    # all-time usage for one Wave (default window: 30 days)
lf session usage --task LOO-12 --context  # each step's input by source, flagging over-budget items with *
lf session usage --weekly                 # weekly trend by step, harness, Task, and turn time vs. context size
```

Each row reports input, output, and cache-read tokens, cost (where the provider
reports it: Pi and OpenCode report per-turn cost; Claude, Codex and Antigravity
report tokens, plus Claude's running `total_cost_usd`; an Antigravity row
counts the conversation lf started, not its subagents'), finished streams (`FINAL`),
and any recording gaps (`GAPS`).

For work sessions and workers, turn-end hooks ask the server to record each
turn from the provider's transcript; at exit, the launching process catches any
missed turns (and covers bare chats and batches without needing the server).
Raw conversations started outside `lf` are shown live without recording usage
rows.

`--context` breaks each step's input into instructions, memory, scratch, goal,
steers, carried conversation, and tool output. Check or adjust your authoring
targets with `lf context` ([Context budgets](config.md#context-budgets-and-assembly)).

## Task summaries

```bash
lf task status LOO-12 --json | jq .summary
```

Once a Task completes or is abandoned, its `summary` records how and when it
ended, the Graph it ran, how many times each loop iterated (`loop_passes`),
whether auto was on or off at the end (`auto`, `null` if never set), wall time from its first Graph run, and the
headline from its last `reflect` step. Every run in `lf session usage --json`
also carries `skill_version` (SHA-256 of the skill text) so you can compare
runs across skill edits.

## Conversation history and transcripts

```bash
lf session history --task LOO-12                              # chronological turns and outcomes
lf session history show <id> --final                          # the agent's final answer
lf session history show <id> --context                        # what input was assembled
lf session history show <id> --transcript                     # full conversation from the provider's files
lf session history show <id> --instructions                   # which skills/guidance were included and why
lf session history search "cleanupPeriodDays" --since 30d     # search recent conversations
lf session retention                                          # what providers still keep and what expires next
```

Conversation text stays in each provider's native store (Claude, Codex, Pi
or Antigravity files, or OpenCode's SQLite DB); Loopflow reads it on demand rather than
duplicating it. Claude removes conversations idle longer than
`cleanupPeriodDays` (30 days by default in Claude's `settings.json`); Codex,
OpenCode, and Pi keep everything.

Loopflow stores its own lightweight records separately from active state in
`~/.lf/history.db`: one row per provider turn (kept permanently) and 90 days of
Session history (retries, failures, provider switches), pruned hourly by the
server or on demand with `lf machine prune`.

Replay any captured request as new work (same prompt, model, and permissions,
current native login) without touching the original record:

```bash
lf replay <run-id>
```

## Live runs, codebase size, and health

```bash
lf session list                           # live Sessions on this Machine (OS-confirmed PIDs)
lf graph list --runs --state current      # live Graph runs
lf graph show <RUN_ID>                    # step-by-step results for any Graph run
lf repo tokens                            # lines and tokens per directory (tracked files, same tokenizer as budgets)
lf repo tokens --days 90                  # codebase token growth over time
lf machine doctor                         # installation, store, lock contention, PTYs, and stray processes
lf machine doctor --plan                  # read-only check of repository planning
```

`lf machine doctor` checks each subsystem (`ok`, `warn`, or `fail`, plus
`--json`) and lists any stray agent processes left behind after a crash so you
can inspect and stop them by PID.

## Reference

| Command | What it does |
|---|---|
| `lf session usage [--task\|--wave] [--days N]` | Recorded token/cost usage per Session |
| `lf session usage --context` / `--weekly` | Input breakdown by source / weekly context & latency trend |
| `lf session usage --parent SESSION` | Usage of child runs issued by one Session |
| `lf session history [--task\|--wave]` | Chronological Session activity |
| `lf session history show ID` | Inspect one Session (`--final`, `--context`, `--transcript`, `--events`, `--instructions`) |
| `lf session history search TEXT` / `lf session retention` | Search conversations / check provider retention |
| `lf replay ID` | Re-run a captured request as new work |
| `lf session list`, `lf graph list` | Live Sessions and Graph runs |
| `lf repo tokens [--days N]` | Codebase size in lines and tokens |
| `lf machine doctor [--plan]`, `lf machine prune` | Health audit / prune expired history and compact DBs |

`lf usage`, `lf history`, and `lf doctor` resolve to the same commands. Full
flags: [`lf session`](lf-reference.md#lf-session).

Next: [Orchestration](orchestration.md): customize Skills, compose Graphs,
loops, and human review checkpoints.
