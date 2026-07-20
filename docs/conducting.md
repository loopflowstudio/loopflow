# Conducting

Running one agent is a conversation. Running many is conducting: knowing what
is playing, what is stuck, what needs you, what it cost — and redirecting a
performer without stopping the piece. Loopflow answers all of that from the
local ledger. Its read surfaces take `--json`; `lf top` is the human-oriented
exception because it renders a live terminal graph.

## See everything

```bash
lf ls                  # every wave, running and stopped, live servers marked
lf status <wave>       # one wave's Project → Task hierarchy, runs, attention
lf roadmap             # every open Task across every wave
```

`lf roadmap` buckets the whole machine's work by what it needs: **Now** (live
and advancing), **Needs attention** (a User-routed Ask or recovery),
**Available**, **Later**. It overlays live evidence on
the Linear-backed plan, so the answer to "where is my attention needed" is one
command.

## Drill down

```bash
lf runs --wave infra          # recent agent-backed runs with token evidence
lf execs                      # recent lf processes across all repos
lf trace <exec-id>            # reconstruct one process tree
lf trace <id> --content       # include prompt/conversation bodies (gated)
lf context                    # what each agent actually received
```

`lf context` is the Context Lab: for any run, the contributing assets, the
prompt-ordered lanes, and the trace addresses — the difference between "the
agent did something odd" and "the agent was told something odd." Filter by
wave, project, task, repo, flow, skill, provider, model, or outcome.

## Health and spend

```bash
lf top             # output-token throughput last hour + running processes
lf usage           # subscription state per account, spend by repo/provider
lf tokens          # lines and tokens per directory; --days walks history
lf ci --since 7d   # how failed CI was detected, repaired, and landed
lf doctor          # audit the ledger: continuity, attribution, lineage
```

`lf top` is the first move when work feels slow — machine-health evidence
before guessing. `lf ci` reads the local ledger, not GitHub: it reports how
much of CI repair happened without a human.

## Steer

Reading is half; the system stays steerable while it runs.

```bash
lf chat --steer "ship the parser fix first"   # reach the live wave body, else queue
lf chat --follow                              # replay and tail the conversation
lf task steer INF-123 "smaller PR"            # redirect one Task's active turn
lf work asks                                  # questions waiting on you
lf work answer ask_... "take the smaller PR"  # answer one exact question
lf invocation list --active                   # reopenable provider/TUI invocations
```

Steering is durable-first: `steer` appends direction before attempting a live
send. Provider acceptance is only transport evidence; a later successful
boundary Basis proves application — see [The Agent API](agent-api.md#steer).

Ask/Answer is targeted tool I/O in one Turn. It neither advances the flow nor
enters the Steer queue; the enclosing Turn advances only when it completes.

Genuinely terminal-shaped work (a login, an opaque TUI) stays on the owning
AgentInvocation. `lf invocation present <id>` reopens its attach route, and `lf invocation
handback <id> --outcome ...` records explicit terminal evidence without
inventing a second Work identity.

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

Use `lf work asks` and `lf work answer` when Work asks an exact question. Use
`lf chat --steer` or `lf task steer` for unsolicited durable direction.

## Next

[The Agent API →](agent-api.md) · [Waves →](waves.md) · [`lf` reference](lf.md)
