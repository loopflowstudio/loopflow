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
The Wave coordinates Projects and Tasks from the canonical checkout. Its UUID
is stable identity; canonical checkout plus normalized name is its mutable
locator. Two repositories may each own a Wave named `product` without sharing
state.

## Persistence and discovery

The Wave's current canonical repository holds its durable files:

```text
.lf/journal/waves/<name>/journal.jsonl
wave/<name>/MEMORY.md
wave/<name>/.wave-endpoint
wave/<name>/.wave-resident-token
```

The journal rebuilds the thread, playhead, and loop state after restart. The
endpoint and resident token exist only while the listener owns that boot and
are removed on shutdown.

Expanded Wave plans stay pinned across restarts while their definitions remain
unchanged. Before opening a body, the listener compares every journaled stack
and queued plan with the current catalog. Any name, kind, order, policy, or
shape change appends one reset snapshot and starts the current root at step
zero. The reset drops cursors, iterations, nested invocations, and queued flow
continuations; pending chat and inbox messages remain available to the fresh
flow. An active body finishes against its pinned plan before this check runs.

Wave Chat is local when `GOAL.md` has no `chat` block. A Discord channel binding
replaces that backing on the next listener start. Each change appends one
conversation epoch; reopening the same backing resumes its epoch. The listener
preflights Discord before reserving a Run or opening the journal, then polls
after the committed cursor, journals external inputs before advancing it, and
journals deterministic send intents before posting. The resident receives only
source-tagged authored input and never inherits `LF_DISCORD_TOKEN`. Binding
ownership is explicit and must agree with the Wave's durable Home placement;
an OS-held lease prevents concurrent listeners across its checkouts.

The shared `~/.lf/loopflow.db` stores the Wave UUID, its repository-scoped
locator, and typed Project and Task observations. Human commands resolve the
name only inside the invoking repository; a diagnostic bare-name lookup fails
when several repositories own that name. A Wave can still run when that store
does not exist, but child observations are unavailable. The live endpoint and
locator lock enforce one listener per repository-scoped Wave; `--force`
explicitly takes over a live endpoint.

Rename or rehome a stopped Wave by UUID:

```bash
lf work relocate wave <wave-id> --name platform
lf work relocate wave <wave-id> --repo ../moved-repository
```

Relocation preserves the UUID, PM projection, Work and Run history, Home
placement, authored files, and journal. It moves a complete Wave chord when the
repository changes, carries nested descendants through a rename, requires
compatible configured PM Teams, and refuses live Work or divergent target
files. Retry completes verified source cleanup after a crash at the commit
boundary.

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
