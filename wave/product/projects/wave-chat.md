# Wave chat

Wave Chat is the human conversation surface for a wave: one executive thread
where the user steers work, receives reports, interrupts bad runs, and decides
what happens next. It is also the retention surface: the conversation, reports,
and curated memory that let a wave continue across land, branch, machine, and
cold starts without growing a second brain.

## KRs

- Each wave has one steward thread that owns human-facing conversation.
- `lf chat` carries worker reports, escalations, and outcomes into that
  thread.
- Send, steer, interrupt, and resume actions target the right session without
  exposing runtime plumbing.
- The steward thread survives app restart, process reattachment, and replay in
  5/5 dogfood trials.
- No learning is lost across land, branch, machine move, or compaction.
- MEMORY.md answers "decided / constrained / in flight" at a glance and stays
  prompt-sized as facts accumulate indefinitely.
- Memory changes can point to a cited chat event, worker report, or run trace
  that justified the retained fact.
