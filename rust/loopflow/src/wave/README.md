# Wave runtime

```bash
lf wave product
lf chat --wave product "What needs attention?"
lf stop product
```

`lf wave <name>` starts one resident Wave as a listener and a resident process.
The split is runtime plumbing behind one command:

```text
lf wave <name>                     lf __resident <name>
┌──────────────────────┐ spawns   ┌──────────────────────┐
│ listener             │────────▶│ resident             │
│ journal · HTTP       │◀────────│ cadence · agent     │
└──────────────────────┘ deltas   └──────────────────────┘
```

The listener owns the durable thread, HTTP routes, local discovery, and typed
Project and Task observations. The resident owns the pass
scheduler and provider process. It reads the listener's inbox and returns
ordered deltas; it never writes the journal directly.

Repository mutations belong to Tasks in their own sibling worktrees.
The Wave coordinates Projects and Tasks from the canonical checkout.

## Persistence and discovery

The origin repository holds the Wave's durable files:

```text
.lf/journal/waves/<name>/journal.jsonl
wave/<name>/MEMORY.md
wave/<name>/.wave-endpoint
wave/<name>/.wave-resident-token
```

The journal rebuilds the thread, playhead, and loop state after restart. The
endpoint and resident token exist only while the listener owns that boot and
are removed on shutdown.

The shared `~/.lf/loopflow.db` stores the Wave row and typed Project and Task
observations. A Wave can still run when that store does not
exist, but child observations are unavailable. The live endpoint enforces one
listener per Wave; `--force` explicitly takes over a live endpoint.

## Thread

`lf chat` writes to the durable human thread. `--steer` injects into a compatible
active provider turn and otherwise queues the message for the next pass.
`lf chat --follow` replays the latest 12 turns and follows new turns. The
conversation shows human and Wave prose plus human-level failures; commands and
tools stay in the journal, which retains the complete thread.
`lf chat --history --json -w <wave>` folds the latest saved turns directly from
that journal, so a stopped listener does not make the conversation disappear.
The response distinguishes missing, partial, and unavailable evidence from a
valid empty thread.

## Listener HTTP surface

The endpoint file contains `127.0.0.1:<port>`. User-facing routes are local and
do not require the resident token:

| Method and path | What it does |
| --- | --- |
| `GET /health` | Reports listener and resident state. |
| `GET /conversation` | Returns the durable thread; `?limit=N` tails it. |
| `GET /events` | Replays the latest 12 human-thread turns, then streams turn, turn-delta, state, and playhead events over SSE; `?limit=N` overrides the replay tail. |
| `POST /messages` | Sends `message`, `steer`, or `interrupt`. |
| `GET /playhead` | Returns the durable pass cursor. |
| `POST /stop` | Gracefully stops the listener and resident. |

The hidden `/resident/*` routes carry listener/resident coordination and require
the per-boot token. Their Rust DTO fixture is
`tests/fixtures/dto/resident_deltas.json`.

Run the process-level smoke test with:

```bash
cargo test -p loopflow --test wave_live_smoke -- --ignored
```
