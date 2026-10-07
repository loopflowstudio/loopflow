# v0.13.9

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.9 keeps Task design and review in one conversation, with a visible Workflow showing where the work stands and what can run next. It also reduces the database cost of new session history and restores concurrent Git reads in `lf wt list`. Settled Tasks leave the Desktop sidebar once execution is resolved and no open Sessions remain, keeping ongoing work easier to find.

## Keep the conversation as the Task moves forward

Task design and review previously depended on separate review Sessions and saved Flow progress. Tasks now have Workflows whose conversation nodes are joined by Flow edges, so the conversation stays available while execution advances the work.

- Desktop opens a Task on its primary Session, with the Workflow graph in the header. The **+** menu adds shells, Files, or the Flow exec log.
- Tasks take up their Project's Workflow unless one is explicitly selected. `lf task run` runs an edge; `lf task move` changes position without executing work. Reaching `end` completes the Task.
- Ordinary Flow execution replaces Task workers and resumable FlowSession state. Each driver records its launched graph and command steps in append-only FlowExec history; a Task run makes up to three attempts when its Flow fails.
- Launching an existing Task resolves its Flow in that Task's checkout, keeping execution tied to the work being run.
- Locally completed or abandoned Tasks leave the sidebar once execution is settled and no open Sessions remain, even when planning status is still open. Open Tasks remain in the Wave plan, and Tasks with open Sessions remain reachable.

## Keep session history with less database overhead

Streamed sessions previously stored every token twice in SQLite. New history writes coalesce streaming increments and retain each Turn's latest cumulative diff, while preserving complete items, usage, input and outcomes.

- `events.jsonl` continues to preserve every event verbatim.
- A replay simulation across 2,046 captures retained 27.5% of event rows and about 72% of payload bytes. These are simulated results; installed savings have not yet been observed.
- Existing history is not compacted. Capture and artifact retention remain unchanged, so total storage growth is still unbounded.

## List busy worktrees faster

A shared lock had forced `lf wt list` to run its Git reads one at a time. Reads now run concurrently again, reducing the local portion of listing time in repositories with many worktrees.

- In a 54-worktree benchmark against installed v0.13.6, the branch release build reduced median local Git time from about 1.28 seconds to 0.31 seconds.
- Median listing time fell from 1.48 to 1.23 seconds for text and from 1.40 to 1.12 seconds for JSON. Text tail latency changed little; the improvement was strongest in the median and JSON output.
- Every listing still reads dirty state from each checkout. Git process counts and the four GitHub requests are unchanged, and results are not retained between listings.

## Operational notes

The Task Workflow transition removes saved Flow resume, automatic recovery, and Session ready/complete controls. Migration retains legacy conversation evidence, but old Flow progress cannot be resumed; existing unfinished Tasks acquire a Workflow on their next run.

- Populated-store migration and real-provider execution remain unverified in the supplied release evidence. Disposable-installation proofs passed, while the CLI rehearsal used provider-free stand-ins.
- Reopened Tasks that cannot run and clipped Flow exec diagrams remain known follow-ups.
- Install preflight validates an already-matching database schema read-only in place. Pending migrations still use a private snapshot.
- A checkpoint that resets the write-ahead log reduces retained WAL space to 64 MiB. This is not a limit on an active WAL.
- After a successful migration, Loopflow keeps the two newest fingerprinted backups. Hand-named and unfingerprinted backups are preserved.
- If a process dies with buffered streaming increments, those increments remain available only in the capture file.

The worktree benchmark used a branch release build on a busy host; installed performance remains unverified. GitHub responses are now the measured bottleneck, and the target of online warm p95 at or below one second is still unmet. Use `lf wt timing` to inspect the local and remote phases on an installed build.

## Small changes

- Provider-stream observations and `--waiting` surface conversations waiting on input. Waiting detection does not cover native Claude or OpenCode terminals.
- Authored loops use `loop: <target>` with `loop-or-next`; builtin workflows, skills and documentation follow the new Task model.