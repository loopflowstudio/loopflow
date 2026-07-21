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

Wave Chat is local when `GOAL.md` has no `chat` block. A Discord channel binding
replaces that backing on the next listener start. Each change appends one
conversation epoch; reopening the same backing resumes its epoch. The listener
preflights Discord before reserving a Run or opening the journal, then polls
after the committed cursor, journals external inputs before advancing it, and
journals deterministic send intents before posting. The resident receives only
source-tagged authored input and never inherits `LF_DISCORD_TOKEN`. Binding
ownership is explicit: the configured Home is the only Home allowed to attach,
and an OS-held lease prevents concurrent listeners across its checkouts.

The shared `~/.lf/loopflow.db` stores the Wave row and typed Project and Task
observations. A Wave can still run when that store does not
exist, but child observations are unavailable. The live endpoint enforces one
listener per Wave; `--force` explicitly takes over a live endpoint.

## Thread

`lf chat` writes only when the active epoch is local. `--steer` injects into a
compatible active provider turn and otherwise queues the message for the next
pass. A Discord epoch rejects authored text with a typed Open-in-Discord action;
it never writes a parallel local turn. A bare interrupt remains available.

`lf chat --follow` replays the latest 12 source-bearing messages and follows new
ones, printing epoch boundaries and local/Discord provenance. Local history
folds from the journal even while stopped. Discord history is projected from
the provider through the active listener without copying transcript pages into
the journal. `lf chat --history --json -w <wave> --epoch <id>` reads one earlier
epoch without stitching it into the active conversation.

## Listener HTTP surface

The endpoint file contains `127.0.0.1:<port>`. User-facing routes are local and
do not require the resident token:

| Method and path | What it does |
| --- | --- |
| `GET /health` | Reports listener/resident state plus the active chat epoch and backing health. |
| `GET /conversation` | Returns one source-bearing epoch; `?limit=N` tails it and `?epoch=<id>` selects history. |
| `GET /events` | Emits epoch and backing health, replays source-bearing messages, then streams message, local-only message-delta, state, and playhead events. |
| `POST /messages` | Sends locally, or returns `409` with Open in Discord when Discord is active. |
| `GET /playhead` | Returns the durable pass cursor. |
| `POST /stop` | Gracefully stops the listener and resident. |

The hidden `/resident/*` routes carry listener/resident coordination and require
the per-boot token. Their Rust DTO fixture is
`tests/fixtures/dto/resident_deltas.json`.

Run the process-level smoke test with:

```bash
cargo test -p loopflow --test wave_live_smoke -- --ignored
```
