# Conducting

Running one agent is a conversation. Running many is conducting: knowing what
is playing, what is stuck, what needs you, what it cost — and redirecting a
performer without stopping the piece. Loopflow answers all of that from the
local ledger — every command below reads machine-local truth and takes
`--json`.

## See everything

```bash
lf ls                  # every wave, running and stopped, live servers marked
lf status <wave>       # one wave's Project → Task hierarchy, runs, attention
lf roadmap             # every open Task across every wave
```

`lf roadmap` buckets the whole machine's work by what it needs: **Now** (live
and advancing), **Needs attention** (waiting on a human — a handoff, a
decision, a recovery), **Available**, **Later**. It overlays live evidence on
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

Reading is half; the orchestra stays steerable while it plays.

```bash
lf chat --steer "ship the parser fix first"   # reach the live wave body, else queue
lf chat --follow                              # replay and tail the conversation
lf task steer INF-123 "smaller PR"            # redirect one Task's active turn
lf task attach INF-123                        # writable, audited control terminal
lf handoff list                               # interactive work waiting on a human
```

Steering is receipted: a `steer` returns a command id, and
`lf task receipt <id> --until incorporated` proves the redirect actually
entered the Session — see [The Agent API](agent-api.md#steer).

Handoffs are how agents hand a human genuinely interactive work (a login, a
judgment call, a demo) without losing the thread: the handoff is durable,
listed, and completes back into the owning Session.

## The Mac app

The Loopflow app is the podium. It opens on a repository rail, the wave list,
and the machine-wide roadmap, and it is a pure client over `lf --json` — no
second database, no machine-wide service. What the CLI reads, it renders:

- **Wave Chat** — the persistent conversation, with send, steer, and interrupt.
- **Roadmap** — every Task across every wave with lifecycle controls and
  attention chips: green (advancing), red (needs a human), black (settled).
- **Task workspace** — changed files, per-file patches, and an embedded
  terminal attached to the running agent.
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

Steer through `lf chat --steer` and `lf task steer`; attach writable control
through `lf task attach`. Never type into an agent's tmux session directly.

## Next

[The Agent API →](agent-api.md) · [Waves →](waves.md) · [`lf` reference](lf.md)
