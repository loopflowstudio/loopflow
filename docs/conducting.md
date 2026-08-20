# Conducting

Running one agent is a conversation. Running many is conducting: knowing what
is playing, what is stuck, what needs you, what it cost — and redirecting a
performer without stopping the piece. Loopflow answers all of that from the
local ledger. Use `lf ps --json` for a machine-readable activity frame and
`lf top` for the continuously refreshed terminal view.

## See everything

```bash
lf ls                  # every wave, running and stopped, live servers marked
lf status <wave>       # one wave's Project → Task hierarchy, runs, attention
lf roadmap             # every open Task across every wave
lf activity            # what changed, newest first, with durable evidence
```

`lf roadmap` buckets the whole machine's work by what it needs: **Now** (live
and advancing), **Needs attention** (a User-routed Ask or recovery),
**Available**, **Later**. It overlays live evidence on
the Linear-backed plan, so the answer to "where is my attention needed" is one
command.

`lf activity` orders durable Work creation, Run, Task PR, and Steer facts.
Filter with `--wave`, `--project`, or `--task`; filters apply before `--limit`.
It is history, not another live process model: `lf ps` owns current motion.

## Drill down

```bash
lf runs --wave infra          # recent agent-backed runs with token evidence
lf runs --project parser      # one Project, filtered before the result cap
lf execs                      # recent lf processes across all repos
lf trace <exec-id>            # reconstruct one process tree
lf trace <id> --content       # include prompt/conversation bodies (gated)
lf context                    # what each agent actually received
```

`lf context` is the Context Lab: for any run, the contributing assets, the
prompt-ordered lanes, and the trace addresses — the difference between "the
agent did something odd" and "the agent was told something odd." Filter by
wave, project, task, repo, flow, skill, provider, model, or outcome.

## Health and usage

```bash
lf ps              # one call-tree snapshot ranked by completed output
lf top             # continuously refresh rates, age, idle time, and health
lf prune --dry-run # inspect safely removable process state
lf usage           # subscriptions plus provider tokens, cache, and cost
lf telemetry-daily # repository maintainer health and budget report
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
much of CI repair happened without a human.

## Steer

Reading is half; the system stays steerable while it runs.

```bash
lf chat --steer "ship the parser fix first"   # reach the live wave body, else queue
lf chat --follow                              # replay and tail the conversation
lf task steer INF-123 "smaller PR"            # redirect one Task's active turn
lf ask list --user --json                     # requested sessions needing attention
lf ask open ask_...                            # open one Ask session
lf invocation list --active                   # reopenable provider/TUI invocations
```

Steering is durable-first: `steer` appends direction before attempting a live
send. Provider acceptance is only transport evidence; a later successful
boundary Basis proves application — see [The Agent API](agent-api.md#steer).

An intervention Ask is targeted tool I/O in one Turn. It does not enter the
Steer queue; the enclosing Turn advances only when it completes. A human Task
node uses a `FlowStep` Ask between provider Invocations and advances only after
explicit resolution.

Genuinely terminal-shaped work (a login, an opaque TUI) stays on the owning
AgentInvocation. `lf invocation present <id>` reopens its attach route, and `lf invocation
handback <id> --outcome ...` records explicit terminal evidence without
inventing a second Work identity.

Human Task nodes park the playhead and appear in the same User Ask queue.
Opening one runs the authored skill with fenced Task writer authority in its
worktree. Release or ordinary exit requeues it; decline returns to autonomous
work; presentation and handback never advance it.

## The Mac app

The Loopflow app is the podium. It opens on a repository rail, the wave list,
and the machine-wide roadmap, and it is a pure client over `lf --json` — no
second database, no machine-wide service. What the CLI reads, it renders:

- **Wave Chat** — the persistent conversation, with send, steer, and interrupt.
- **Roadmap** — every Task across every wave with lifecycle controls and
  attention lenses: green (advancing), blue (waiting on you), red (unhealthy),
  black (settled).
- **Task workspace** — changed files, per-file patches, and local inspection tools.
- **Context Lab** — the same `lf context` evidence, with "refine in a task"
  one click away.
- **Telemetry** — token spend, codebase growth, registry health.

## tmux

Detached Wave, Project, and Task processes run as named tmux sessions — that
is process lifetime and inspection, not the steering protocol:

```bash
tmux ls                     # live agent processes
tmux attach -r -t <name>    # read-only look inside one
```

Use `lf ask list` and `lf ask open` when Work asks an exact question. Use
`lf chat --steer` or `lf task steer` for unsolicited durable direction.

## Next

[The Agent API →](agent-api.md) · [Waves →](waves.md) · [`lf` reference](lf.md)
