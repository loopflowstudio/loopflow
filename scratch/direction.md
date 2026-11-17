# Direction for LOO-959

Jack, 2026-11-17: "i do want you to start a task which is about responding to
killed task sessions and computer or lf server restarts"

Then, as "general thoughts" (the fourth was cut off as sent):

- "do we need to maintain some limits on the # of managed sessions [or managed + worker sessions]? what are the scalability concerns, and how does the system need to adapt to the possibility of eviciton or launch failire or something like that?"
- "does the system naturally recover from things like failed processses, given thte process maintainenance"
- "how can we make machine-server crash restarts and entire full on computer reboots graceful [while also making desirable cleanup last]"
- "in general, what kind of cleanup needs to be done on an ongoing basis that istn"

And: "should be stacked on LOO-953".

## What that adds to the brief

Kickoff answers these four before choosing an approach; each answer is
measured or read from the code, not assumed.

1. Limits. What one Machine can hold: managed conversations (each a held
   terminal, a PTY and a provider process) plus workers. Find what runs out
   first (memory, PTYs, file descriptors, provider rate limits, the SQLite
   write lock). Say whether a limit is needed at all, and what happens at it:
   a launch that waits, a launch that fails with its reason, or an idle
   conversation that is evicted and started again when it is next needed. A
   launch failure and an eviction must both be states the Task can show and
   recover from, never a silent stop.
2. Natural recovery. With liveness on Sessions and Graph runs (LOO-953),
   list each way a process fails and say for each whether something notices
   and what it does today. The gaps are this Task's work.
3. Graceful restarts. A server crash, an upgrade and a full reboot: what is
   written before going down, what is read on the way up, and in what order
   things come back. Cleanup that is wanted (ended holders, stale sockets,
   orphaned captures) must still happen when the shutdown was not orderly.
4. Ongoing cleanup. Inventory what accumulates while the system runs and
   who removes it today, if anyone. The thought was cut off; do the
   inventory and mark which items nothing removes.

## Base

Stacked on LOO-953 ("Remove Process rows: liveness on Sessions and Graph
runs"), branch `jack/loo-953-remove-process-rows-liveness-on-sessions-and`,
no PR yet. Its design is `scratch/base-loo-953-design.md`. Read liveness
from the Session and Graph-run rows it adds; add no Process table or
registry. The PR targets that branch until it merges.
