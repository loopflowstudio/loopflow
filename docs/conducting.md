# Conducting

Read this when work is running and you want to know where it stands.

Working with one AI agent is a conversation. Working with several means
keeping track: what is moving, what is stuck, what is waiting on you, and
what it cost. Conducting is how you see all of that and step in without
stopping everything else.

## In the Mac app

The app opens on your projects, their Waves, and a roadmap of every task on
this computer. It shows the same information the commands on this page
print, and keeps no separate copy.

- **Wave conversations**: ordinary Sessions with Wave context.
- **Roadmap**: every task across every Wave, each marked waiting, blocked,
  clear, or unknown.
- **Sessions**: the conversations that are open, with the ones waiting
  on you listed first. Each one is a terminal inside the app, where you talk to
  the AI directly.
- **Task workspace**: the files a task changed, and what changed in each.
- **Telemetry**: what the work cost, how the project grew, and whether
  Loopflow's own records are healthy.

## From the command line

```bash
lf roadmap                             # tasks in this project, sorted by what they need
lf roadmap --all                       # every project on this computer
lf top                                 # what is running right now
lf usage --days 0 --wave infra                   # the record of each time the AI was started
lf ssh <home-id> roadmap               # ask another computer the same question
```

Each command reads the computer it runs on. Nothing watches every machine
at once; `lf ssh` is how you ask a different one.

## See everything

```bash
lf wave list                  # every registered Wave and its placement
lf wave status <wave>       # one wave's Project → Task hierarchy, execution and conditions
lf roadmap             # every open Task across this repository's Waves
lf roadmap --all       # every repository on this machine
lf activity            # what changed, newest first, with durable evidence
```

`lf wave status` and every `lf roadmap --json` Wave row carry the same
Project-owned `metric_portfolio`: current Met/Missed evidence, explicit
Unknown or Unavailable states, candidate instruments, and contract issues.
The Mac Wave detail renders that Rust-derived evidence without recomputing
targets or freshness.

`lf roadmap` buckets this repository's work by what it needs: **Now** (live
and advancing), **Waiting**, **Available**, **Later**. It overlays live evidence
on the Linear-backed plan. Each Task carries one semantic condition — clear,
waiting, blocked, or unknown — while `lf session list` is the separate list of
unresolved conversations. Add `--all` for the machine-wide projection.

`lf activity` orders durable Work creation, execution, Task PR, and Steer facts.
Filter with `--wave`, `--project`, or `--task`; filters apply before `--limit`.
It is history, not another live process model: `lf ps` owns current motion.

## Drill down

```bash
lf session list --task INF-123 --json
lf session list --orphan --json          # Sessions without Task association
lf session list --interactive false --history --task INF-123 --json
lf session history SESSION --json
lf session connect SESSION
lf mon show --task INF-123 --json
lf mon show run_ab12 --final
lf replay run_ab12
```

Sessions in a Task checkout belong to that Task, including subdirectories and
symlink aliases. Explicit repo/Wave Sessions remain separate. `--orphan` only
filters inventory; it cannot opt a new Session out of Task membership.

Use Session identity to return to a conversation and its native history. A
successful provider turn, the command's outcome and the Flow's progress can
differ: the engine can finish after its driver dies, and a finished Flow
leaves its Task at a workflow node, waiting on you.

`lf mon show` and `lf replay` retain their historical selectors during the conversion.
Their current fields and reader limitations are recorded in
[cutover status](architecture-reference.md#cutover-status) and the
[CLI reference](lf-reference.md#monitor-history-and-live-activity). Replay uses the captured prompt,
provider/model, account and permission boundary. It creates new work and requires
writable admission; it does not alter the original conversation's evidence.

Missing usage is unknown, not zero. An absent terminal receipt does not prove
that a provider is still running; inspect exact OS evidence separately.

## Health and usage

```bash
lf ps              # one OS-live process and call-tree snapshot
lf top             # continuously refresh elapsed time and process state
lf mon prune --dry-run # inspect safely removable process state
lf usage --days 30 # recorded provider usage
lf usage --task INF-123 # the same evidence drilled to one Task
lf usage --json    # current usage wire, newest first
lf tokens          # lines and tokens per directory; --days walks history
lf ci --since 7d   # how failed CI was detected, repaired, and landed
lf ci watch --status # the CI watcher: live or not, last poll, what it started
lf doctor          # check installation, Process integrity and scheduled receipts
```

`lf top` is the first move when work feels slow — live machine-health evidence.
Use `lf ps --json` when another tool or agent needs one stable, parseable frame.
Both contain only OS-live process trees; completed calls disappear. Run
`lf mon prune --dry-run` before cleanup. Plain `lf mon prune` removes stale Process
receipts and registered orphan OpenCode groups, never unclaimed provider PIDs.
`lf ci` reads the local ledger, not GitHub: it reports how
much of CI repair happened without a person.

## Steer

Reading is half; the system stays steerable while it runs.

```bash
lf --wave <wave> wave/operate "ship the parser fix first"
lf session ensure -w <wave>                # the Wave's ongoing conversation and operator
lf --wave <wave> : "Review this plan"          # start a conversation
lf comment INF-123 "smaller PR"            # post a Linear comment; deliver to its running Flow
lf interrupt INF-123                     # end this turn and re-read direction
lf session list --waiting --json              # conversations waiting on you
lf session connect <session-id> --json           # recover one exact conversation
```

Task steering posts a Linear Task comment; commenting in Linear also steers the
Task's running Flow. Independent conversations receive no live injection. Idle Tasks retain
comments without starting execution. Task interrupt ends the active turn so
the next step re-reads direction. Publication and transport acceptance do not
prove that the agent applied the correction. See [The Agent API](agent-api.md#steer).

Headless work that lacks required input explains its failure and stops. Read
its existing status and logs; discuss unresolved judgment in the ongoing Wave
chat. Review happens in the Task conversation; Flows hold autonomous steps only.
Use `lf --task <task> : "<prompt>"` when another agent perspective is needed.

A Flow whose driver died stays as history. Nothing resumes it. Read
`lf task status <task>`, then launch fresh work with `lf task run <task>`.

## Inspect and resume

```bash
lf top                                 # live Loopflow process activity
lf wave status shipper                 # work and its current conditions
lf session connect <id>                   # start or resume the selected conversation
```

Open a Session in the app or CLI to return to its provider-native conversation.
Task workspace shells run directly in the app's terminal.

See the [Sessions lifecycle](../README.md#sessions) for opening and replacing
conversations.
Use `lf comment` for durable Task direction,
and `lf --task` for another agent perspective.

## Next

[The Agent API →](agent-api.md) · [Waves →](waves.md) · [`lf` reference](lf.md)
