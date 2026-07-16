# W2-175 — open questions & blockers

## Environment blockers (reported, not fixing as prerequisites)

- **`lf task acknowledge W2-175` cannot run here.**
  - The installed `lf` on PATH is `0.11.1` and is *diverged* from the shared
    `~/.lf/loopflow.db`: the db carries migration `0.11.009_profiles` (from #936)
    which that binary does not know. This is the cross-branch migration-collision
    hazard already recorded in wave memory.
  - The locally built `target/debug/lf` (from this worktree, which is rooted after
    #936) reads the db past the migration gate, but reports
    `no Task Session exists for "W2-175"` — the Task Session
    `ts_44e584330d5c47a09c68bf12dad708ee` is not registered in this machine's store.
  - Per loopflow operating rules I report this once and continue inline. The
    directive acknowledgement summary is captured in this branch's design note
    instead; the design work does not depend on the store.

- **Host disk was at 100%** (1.5 GiB free of 926 GiB) at run start, which broke
  Bash output capture (`ENOSPC` writing tool output to `/tmp`). Freed transient
  `claude-501` session caches to ~9 GiB to unblock tooling. Underlying disk
  pressure is a machine-health issue, not caused by this task.

## Design decisions resolved inline (executive calls)

- **Scope of this PR = the runtime rendezvous only.** The shared store + CLI +
  DTO handoff contract already merged as PR 1 (#935); flow interaction policy
  (#941) and task resume position (#942) merged separately. This PR wires the
  existing handoff store into the Task body's block/resume — the piece that is
  genuinely missing (`WaitInteractive` is emitted by the engine but consumed
  nowhere).

- **Task parent first; Project/Wave wiring is a follow-up.** The rendezvous helper
  is written parent-agnostic (the handoff parent enum already spans wave/project/
  task), but only the Task runner is wired and tested in this PR. Mac Active
  Sessions and external presentation adapters are explicitly deferred (per the
  Task's delivery note).

- **Failed handoff routes the parent to `Blocked`, not a silent retry loop.**
  Completed / HandedBack advance the flow past the interactive step; Failed wakes
  the parent once and parks it `Blocked` (operator-resumable) with the failure
  reason, so an unmet interactive obligation never loops or is silently skipped.
