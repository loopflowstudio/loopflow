# export-memory v0: cron-steered compile of MEMORY.md

Targets main. First externalization step for the Memory wave. Linear:
"memory: export-memory step" (`16869842`). No restart (that's v1).

## What to build

A daily, local, `lf`-invoked compile of the wave's `MEMORY.md` base — plus the
launchd cron that runs it. Three pieces:

1. **`export-memory` step** (`engine/builtins/ops/step/export-memory.md`) — the
   compile prompt.
2. **`lf op cron` command** — installs/removes a launchd job that runs an `lf`
   command on a schedule (the "store crons as launchd, with an lf command" way).
3. **Register the memory wave's daily cron** with it.

## Piece 1 — the `export-memory` step

Run as `lf export-memory --wave <wave>`. The prompt steers the run to:

- Read the current base (`lf memory show`) and the recent add-stream
  (`lf memory log` — works offline via the journal).
- Compile a **reader-optimized** `MEMORY.md`: what the next mind (fresh session,
  a parent, a new worker) must know to act, with none of this run's context.
  Drop narrative ("tried X then Y"); keep durable conclusions ("X fails because
  Y; use Z"). Merge duplicates; recency wins on contradictions; keep it tight.
- **Write it**: through `lf memory update` when a wave server is live (that also
  resets the add-delta so the injected log empties); else write
  `wave/<wave>/MEMORY.md` directly (serverless — safe when no server holds the
  pen). Then commit (`lf op commit -m "export-memory: compile MEMORY.md"`).

No restart. The same processes keep running; only the base file refreshes.

## Piece 2 — `lf op cron`

A thin launchd wrapper, reusing the plist/`launchctl` helpers in
`lfd/service/macos.rs` and the `deploy/launchd/` convention.

```
lf op cron add --wave <wave> --flow <flow> --schedule <expr>   # write + load a plist
lf op cron list                                                # installed crons
lf op cron remove --wave <wave> --flow <flow>                  # unload + delete
```

`add` writes `~/Library/LaunchAgents/loopflow.cron.<wave>.<flow>.plist` whose
`ProgramArguments` is `lf <flow> --wave <wave>` with `WorkingDirectory` = the
wave repo, and `launchctl load`s it. `<schedule>` maps to
`StartCalendarInterval` (daily default). This replaces the dead GOAL.md-`crons:`
/ daemon-poller path for local `lf` invocation.

## Piece 3 — register the wave cron

Install the memory wave's daily export: `lf op cron add --wave memory --flow
export-memory --schedule daily`. (Commit any generated committed artifact if the
convention keeps plists in `deploy/launchd/`; otherwise it's a local install.)

## Constraints

- **Serverless-capable.** The scheduled run must not require a live wave server:
  `log` folds the journal offline, and the write falls back to a direct file
  edit. Don't add an offline path to `lf memory update` — branch in the step.
- **Bounded log is a server-time nicety, not a v0 requirement.** When the write
  goes through `update` (server live), the delta resets. When it's a direct file
  write, the injected log keeps overlapping the base until the next server-backed
  update — acceptable (the reading mind dedupes). Don't build a new reset marker.
- **No restart** (v1). Don't touch the resident-mind lifecycle.

## Demo

`lf export-memory --wave memory` rewrites `wave/memory/MEMORY.md` as a compiled,
reader-optimized base reflecting the recent `lf memory log`, and commits it.
`lf op cron add --wave memory --flow export-memory --schedule daily` installs a
launchd job; `lf op cron list` shows it; `launchctl list | grep loopflow.cron`
confirms it's loaded.

## Done when

```bash
cargo test -p loopflow          # export-memory step registered; lf op cron
                                # add/list/remove write/parse/unload a plist
cargo clippy -- -D warnings && cargo fmt --check
```

Checklist: `export-memory.md` step exists + registered in `builtins.rs` (+ its
test); `lf op cron` add/list/remove with a plist round-trip test (no real
`launchctl` in tests — mock/inject the loader like existing service tests);
serverless write path covered; `lf memory update`/`MemoryUpdated` untouched.
