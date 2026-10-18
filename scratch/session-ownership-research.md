# Session ownership — accepted October 16, 2026

Jack Heart accepted per-Session owners, including shell PTYs, and shared lfdb
presentation intent. LOO-415 supplies transport; LOO-416 consumes it, preserving
Session/native identity, pending requests and output draining across viewer loss.
No shared daemon, alternate provider UI or second layout store is selected.

The complete research, ownership table, counterexamples and source references
are preserved in [the research archive](evidence/session-ownership-before-attach.txt).
The shell probe proved PID/cwd/environment/jobs and detached output survival,
not exact terminal restoration or provider continuity. Rex documentation and zmx
are research comparators, not implementation instructions or current proof.

SQLite owns durable identity and history; independent per-execution processes own
live channels and lifelines. Viewer claims are separate from provider authority.
Attach never submits a turn or resumes history. Explicit recovery may start a new
provider and must disclose lost live state. No automatic replay follows uncertainty.

LOO-415 now retains a bounded vt100 text snapshot; exact terminal state and
cross-installation layout synchronization remain unresolved. LOO-497 owns shared
layout/draft records; LOO-489 owns native save/restore. Current implementation,
remaining proof and accepted constraints live in [the plan](loo-416-keep-task-sessions.md)
and [caller readiness](evidence/session-readiness-before-stack.txt). Fresh-context restart remains later work.
