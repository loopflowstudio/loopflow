# Wave chat

Wave Chat is the human conversation surface for a wave: one executive thread
where the user steers work, receives reports, interrupts bad runs, and decides
what happens next. It is also the retention surface: the conversation, reports,
and curated memory that let a wave continue across land, branch, machine, and
cold starts without growing a second brain.

## KRs

- One steward thread stays coherent through a month of real use: reports
  folded, decisions traceable, no reset required and no second thread
  spawned to escape the first.
- The thread survives every boundary it meets in a week of dogfood — app
  restart, process reattach, replay, land, branch, machine move — 5/5
  trials each, with zero learnings lost.
- Every retained fact cites its source: for a month of memory changes, each
  one points to the chat event, worker report, or run trace that justified
  it — unsourced facts are failure events.
- MEMORY.md stays prompt-sized while facts accumulate for a month, and
  still answers "decided / constrained / in flight" at a glance.
- Send, steer, interrupt, and resume hit the right session 100% of a
  week's uses without exposing runtime plumbing.

## Open questions

**What does "steer" mean on a harness that cannot be steered?** Only Codex
exposes a true mid-turn steer today; Claude and OpenCode take input only at a
turn boundary, so a message to a live pass queues for the next body. The
send/steer/interrupt KR above is written as if steering is universal, and it is
not. Two readings, and the choice is a product decision rather than a schedule
item:

- *Steer degrades honestly.* On an unsteerable harness the message queues, and
  the surface says so — the thread shows "will reach it at the next step."
  Costs latency; keeps one verb.
- *Steer means steer.* Interrupt the body, fold the message into its successor's
  birth context. Uniform semantics; pays a restart per steer.

Unblocking the first reading needs nothing from a vendor. The second is worth
pricing before assuming the vendors will close the gap for us.

**Steering is a Mac capability, not a chat capability.** `lf chat` always posts
`op:"say"`; only the Mac composer emits `op=steer`. Whatever the answer above,
the CLI and the Mac must reach the same session with the same verb, or the KR
is measuring one surface and claiming both.
