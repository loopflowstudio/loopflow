# Review: Rust lfd stimulus loops + session connect

## What was implemented
- Added Rust daemon stimulus model (loop/watch/cron/recovery) with background loops, scheduling, and recovery handling.
- Extended control protocol and server endpoints to support stimulus CRUD and wave updates.
- Implemented SQLite store changes for stimuli, loop tick tracking, and fork/wave runtime bookkeeping.
- Added Rust core runtime support for fork execution paths and worktree handling.
- Updated Python lfd CLI/protocol bindings to match new stimulus semantics.

## Key choices
- Store-first state: loops read/write SQLite rather than keeping in-memory wave state, keeping daemon recovery simple.
- Event-driven internal state + polling external stimuli (git/cron) on fixed intervals for parity with Python behavior.
- Fork branches use per-branch worktrees with synthesize after branch completion; retries are wave-level.

## How it fits together
- `lfd` exposes protocol endpoints, uses `Scheduler` to start loops, and `RunStore` for durable state. Loop tasks query stimuli, tick flows via `lf-core::tick_flow`, and write results back to SQLite.
- The Python CLI and protocol wrappers map wave/stimulus operations to the Rust daemon API so existing workflows can target the new daemon.

## Risks and bottlenecks
- Watch polling assumes `origin/main` and may miss repos with different default branches.
- Cron polling scans step runs each pass; may need indexing or caching at scale.
- Session connect is still PTY-based and local-only; remote streaming is not yet wired.

## What's not included
- Choose/loop_until_empty flow items (fork only).
- Multi-node/Postgres persistence.
- Production-grade auth/tenant isolation and full observability stack.
