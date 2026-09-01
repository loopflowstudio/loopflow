# Conducting

Running one agent is a conversation. Running many is conducting: knowing what
is playing, what is stuck, what needs you, what it cost — and redirecting a
performer without stopping the piece.

```bash
lf roadmap                 # planning across this repository
lf roadmap --all           # every repository on this machine
lf top                     # processes moving right now
lf runs --wave infra       # append-only launch evidence on this Home
lf ssh <home-id> roadmap   # ask another Home the same question
```

There is no hidden global conductor. Each command reads the machine where it
executes; `lf ssh` is the explicit remote boundary.

## See everything

```bash
lf ls                  # every wave, running and stopped, live servers marked
lf status <wave>       # one wave's Project → Task hierarchy, Runs, conditions
lf roadmap             # every open Task across this repository's Waves
lf roadmap --all       # every repository on this machine
lf activity            # what changed, newest first, with durable evidence
```

`lf status` and every `lf roadmap --json` Wave row carry the same
Project-owned `metric_portfolio`: current Met/Missed evidence, explicit
Unknown or Unavailable states, candidate instruments, and contract issues.
The Mac Wave detail renders that Rust-derived evidence without recomputing
targets or freshness.

`lf roadmap` buckets this repository's work by what it needs: **Now** (live
and advancing), **Waiting**, **Available**, **Later**. It overlays live evidence
on the Linear-backed plan. Each Task carries one semantic condition — clear,
waiting, blocked, or unknown — while `lf session list` is the separate list of
unresolved human conversations. Add `--all` for the machine-wide projection.

`lf activity` orders durable Work creation, Run, Task PR, and Steer facts.
Filter with `--wave`, `--project`, or `--task`; filters apply before `--limit`.
It is history, not another live process model: `lf ps` owns current motion.

## Drill down

```bash
lf runs --wave infra          # recent Home-local Run records for one Wave
lf runs --project parser      # one Project, filtered before the result cap
lf runs --task INF-123 --json # direct bundle evidence for one Task
lf runs run_ab12 --events     # raw append-only evidence for one Run
lf replay run_ab12            # repeat the recorded request as a child Run
```

`lf runs` scans bundles under the current Home. Each has an immutable manifest,
append-only evidence streams, and at most one exclusive terminal receipt. The
scan does not depend on the planning store. `lf usage` reduces provider-authored
counters from those same bundles; missing telemetry stays missing instead of
blocking the launch or becoming a synthetic zero.

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
much of CI repair happened without a human.

## Steer

Reading is half; the system stays steerable while it runs.

```bash
lf chat --steer "ship the parser fix first"   # reach the live wave body, else queue
lf chat --follow                              # replay and tail the conversation
lf task steer INF-123 "smaller PR"            # store direction; inject live when possible
lf task interrupt INF-123                     # end this turn and re-read direction
lf session list --json                        # unresolved human Sessions
lf session open <session-id> --json           # recover one exact conversation
lf ask list --user --json                     # requested sessions needing attention
lf ask open ask_...                            # open one Ask session
```

Task and Project steering appends durable Work comments and relaunches a stopped
controller. A running controller offers new comments to its provider at turn
boundaries; the next Skill seed is the fallback. Task interrupt ends the active
turn so the next boundary reads immediately. Neither a stored Steer nor
transport acceptance proves that an agent applied the direction — see
[The Agent API](agent-api.md#steer).

`lf ask` is a synchronous boundary with a human. It opens an ordinary TUI Run
against the caller's exact checkout, enters the Sessions surface, and blocks
the caller until the human completes the conversation.
Use `lf --as <work> : "<prompt>"` when only another agent perspective is needed.

A human Task node persists the exact `FlowPosition` and provider Run between
autonomous steps. Opening it stops the exact background client and resumes the
provider-native conversation for the authored `lf --as task:<id>` Skill. The
agent may mark the session ready, but that does not remove it or advance
anything. Human Approve advances the playhead; Iterate returns to autonomous
work with new direction; closing or provider exit never advances it.

## The Mac app

The Loopflow app is the podium. It opens on a repository rail, the wave list,
and the machine-wide roadmap, and it is a pure client over `lf --json` — no
second database, no machine-wide service. What the CLI reads, it renders:

- **Wave Chat** — the persistent conversation, with send, steer, and interrupt.
- **Roadmap** — every Task across every wave with lifecycle controls and one
  condition: waiting, blocked, clear, or unknown.
- **Sessions** — interactive Runs, Asks, and Task FlowSteps with their exact
  provider-native conversation and valid resolution actions.
- **Task workspace** — changed files, per-file patches, and local inspection tools.
- **Telemetry** — token spend, codebase growth, registry health.

## tmux

Detached Wave, Project, and Task processes run as named tmux sessions — that
is process lifetime and inspection, not the steering protocol:

```bash
tmux ls                     # live agent processes
tmux attach -r -t <name>    # read-only look inside one
```

Use the [Sessions lifecycle](../README.md#sessions) to open and explicitly
resolve every unresolved human Session.
Use `lf chat --steer` or `lf task steer` for unsolicited durable direction,
`lf --as` for another agent perspective, and `lf ask` for a new human boundary.

## Next

[The Agent API →](agent-api.md) · [Waves →](waves.md) · [`lf` reference](lf.md)
