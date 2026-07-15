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
│ journal · HTTP · bus │◀────────│ cadence · agent     │
└──────────────────────┘ deltas   └──────────────────────┘
```

The listener owns the durable thread, HTTP routes, local discovery, Project and
Task observations, and the bus subscription. The resident owns the pass
scheduler and provider process. It reads the listener's inbox and returns
ordered deltas; it never writes the journal directly.

Repository mutations belong to Task Sessions in their own sibling worktrees.
The Wave coordinates Projects and Tasks from the canonical checkout.

## Persistence and discovery

The origin repository holds the Wave's durable files:

```text
.lf/journal/waves/<name>/journal.jsonl
.lf/journal/waves/<name>/.wave-endpoint
.lf/journal/waves/<name>/.wave-resident-token
wave/<name>/MEMORY.md
```

The journal rebuilds the thread, playhead, and loop state after restart. The
endpoint and resident token exist only while the listener owns that boot and
are removed on shutdown. Loopflow excludes `.lf/journal/` through Git's local
exclude file, so runtime state never requires a project `.gitignore` entry.

The shared `~/.lf/loopflow.db` stores the Wave row, bus messages, and typed
Project and Task observations. A Wave can still run when that store does not
exist, but child observations are unavailable. The live endpoint enforces one
listener per Wave; `--force` explicitly takes over a live endpoint.

## Thread and bus

`lf chat` writes to the durable human thread. `--steer` injects into a compatible
active provider turn and otherwise queues the message for the next pass.
`lf chat --follow` replays the thread and follows new turns.

`lf radio pub` and `lf radio sub` use the SQLite bus. The bus is a short-lived,
prefix-addressed transport between Wave, Project, and Task work; it is not a
second journal. A listener folds messages from its channel family into its
Wave journal and advances a durable cursor.

## Listener HTTP surface

The endpoint file contains `127.0.0.1:<port>`. User-facing routes are local and
do not require the resident token:

| Method and path | What it does |
| --- | --- |
| `GET /health` | Reports listener and resident state. |
| `GET /conversation` | Returns the durable thread; `?limit=N` tails it. |
| `GET /events` | Streams thread, state, playhead, and memory events over SSE. |
| `POST /messages` | Sends `message`, `steer`, or `interrupt`. |
| `GET /playhead` | Returns the durable pass cursor. |
| `GET /memory` | Reads the Wave checkpoint. |
| `GET /memory/log` | Reads facts added since the checkpoint. |
| `POST /memory` | Updates the checkpoint or adds a fact. |
| `POST /stop` | Gracefully stops the listener and resident. |

The hidden `/resident/*` routes carry listener/resident coordination and require
the per-boot token. Their Rust DTO fixture is
`tests/fixtures/dto/resident_deltas.json`.

Run the process-level smoke test with:

```bash
cargo test -p loopflow --test wave_live_smoke -- --ignored
```
