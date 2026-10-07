# v0.13.9

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.9 reduces the disk cost of new session history and restores concurrent Git reads in `lf wt list`. Routine install checks avoid unnecessary database copies, while worktree listings spend less time reading local Git state without sacrificing fresh dirty-state checks.

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

Install preflight validates an already-matching database schema read-only in place. Pending migrations still use a private snapshot.

- A checkpoint that resets the write-ahead log reduces retained WAL space to 64 MiB. This is not a limit on an active WAL.
- After a successful migration, Loopflow keeps the two newest fingerprinted backups. Hand-named and unfingerprinted backups are preserved.
- If a process dies with buffered streaming increments, those increments remain available only in the capture file.

The worktree benchmark used a branch release build on a busy host; installed performance remains unverified. GitHub responses are now the measured bottleneck, and the target of online warm p95 at or below one second is still unmet. Use `lf wt timing` to inspect the local and remote phases on an installed build.