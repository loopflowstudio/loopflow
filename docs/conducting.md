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

- **Wave chat**: the ongoing conversation with a Wave. Send a message, or
  interrupt it.
- **Roadmap**: every task across every Wave, each marked waiting, blocked,
  clear, or unknown.
- **Sessions**: the conversations that are open, including the ones waiting
  for your review. Each one is a terminal inside the app, where you talk to
  the AI directly.
- **Task workspace**: the files a task changed, and what changed in each.
- **Telemetry**: what the work cost, how the project grew, and whether
  Loopflow's own records are healthy.

## From the command line

```bash
lf roadmap                 # tasks in this project, sorted by what they need
lf roadmap --all           # every project on this computer
lf top                     # what is running right now
lf runs --wave infra       # the record of each time the AI was started
lf ssh <home-id> roadmap   # ask another computer the same question
```

Each command reads the computer it runs on. Nothing watches every machine
at once; `lf ssh` is how you ask a different one.

## See everything

```bash
lf wave list                  # every wave, running and stopped, live servers marked
lf wave status <wave>       # one wave's Project → Task hierarchy, Runs, conditions
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

`lf activity` orders durable Work creation, Run, Task PR, and Steer facts.
Filter with `--wave`, `--project`, or `--task`; filters apply before `--limit`.
It is history, not another live process model: `lf ps` owns current motion.

## Drill down

```bash
lf runs --wave infra          # recent Home-local Run records for one Wave
lf runs --project parser      # one Project, filtered before the result cap
lf runs --task INF-123 --json # direct bundle evidence for one Task
lf runs --parent run_ab12 --json # every direct child, without the recent cap
lf runs run_ab12 --final      # the last durable provider conclusion
lf runs run_ab12 --events     # raw append-only evidence for one Run
lf replay run_ab12            # repeat the recorded request as a child Run
```

`lf runs` scans bundles under the current Home. Each has an immutable manifest,
append-only evidence streams, and at most one exclusive terminal receipt. The
scan does not depend on the planning store. `lf usage` reduces provider-authored
counters from those same bundles; missing telemetry stays missing instead of
blocking the launch or becoming a synthetic zero.

`--parent` resolves one exact Run and returns all of its direct children rather
than sampling the recent global list. `--final` reads the provider-neutral
final-answer receipt. Runs without those receipts are labeled and return
streamed prose from their last completed provider turn so recovery remains
possible. It fails explicitly when neither form exists.

Replay reads the source manifest and launches its recorded prompt, agent/model,
turn limit, permission mode, capability flags, and provider account ID through
the ordinary harness. It creates a new Run whose parent names the source. It
does not mutate the source or open planning SQLite.

## Health and usage

```bash
lf ps              # one OS-live process and call-tree snapshot
lf top             # continuously refresh elapsed time and process state
lf prune --dry-run # inspect safely removable process state
lf usage --days 30 # direct cumulative provider evidence per Run
lf usage --task INF-123 # the same evidence drilled to one Task
lf usage --json    # RunSnapshot rows ordered newest first
lf tokens          # lines and tokens per directory; --days walks history
lf ci --since 7d   # how failed CI was detected, repaired, and landed
lf doctor          # audit the ledger: continuity, attribution, lineage
```

`lf top` is the first move when work feels slow — live machine-health evidence.
Use `lf ps --json` when another tool or agent needs one stable, parseable frame.
Both contain only OS-live process trees; completed calls disappear. Run
`lf prune --dry-run` before cleanup. Plain `lf prune` removes stale Exec
receipts and registered orphan OpenCode groups, never unclaimed provider PIDs.
`lf ci` reads the local ledger, not GitHub: it reports how
much of CI repair happened without a person.

## Steer

Reading is half; the system stays steerable while it runs.

```bash
lf --wave <wave> wave/operate "ship the parser fix first"
lf chat --follow                              # replay and tail the conversation
lf task comment INF-123 "smaller PR"            # post a Linear comment; deliver to the advancer
lf task interrupt INF-123                     # end this turn and re-read direction
lf session list --json                        # unresolved Sessions
lf session open <session-id> --json           # recover one exact conversation
lf ask list --user --json                     # requested sessions needing attention
lf ask open ask_...                            # open one Ask session
```

Task steering posts a Linear Task comment; commenting in Linear also steers the
advancing worker. Independent Runs receive no live injection. Idle Tasks retain
comments without starting execution. Task interrupt ends the active turn so
advancement re-reads direction. Publication and transport acceptance do not
prove that the agent applied the correction. See [The Agent API](agent-api.md#steer).

`lf ask` is a synchronous boundary with a person. It opens an ordinary TUI Run
against the caller's exact checkout, enters the Sessions surface, and blocks
the caller until the user completes the conversation.
Use `lf --as <work> : "<prompt>"` when only another agent perspective is needed.

A Task review node persists the exact `FlowPosition` and provider Run between
autonomous steps. Opening it stops the exact background client and resumes the
provider-native conversation for the authored `lf --as task:<id>` Skill. The
agent may mark the session ready, but that does not remove it or advance
anything. Approve advances the playhead; Iterate returns to autonomous
work with new direction; closing or provider exit never advances it.

## Inspect and resume

```bash
lf top                      # live Loopflow process activity
lf wave status shipper           # work and its current conditions
lf session open <id>        # start or resume the selected conversation
```

Open a Session in the app or CLI to return to its provider-native conversation.
Task workspace shells run directly in the app's terminal.

Use the [Sessions lifecycle](../README.md#sessions) to open and explicitly
resolve every unresolved Session.
Use `lf task comment` for durable Task direction,
`lf --as` for another agent perspective, and `lf ask` for a new review boundary.

## Next

[The Agent API →](agent-api.md) · [Waves →](waves.md) · [`lf` reference](lf.md)
