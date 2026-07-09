# Wave chat

Wave Chat is the human conversation surface for a wave: one executive thread
where the user steers work, receives reports, interrupts bad runs, and decides
what happens next.

## KRs

- Each wave has one steward thread that owns human-facing conversation.
- `lf chat` carries worker reports, escalations, and outcomes into that
  thread.
- Send, steer, interrupt, and resume actions target the right session without
  exposing runtime plumbing.
- The steward thread survives app restart, process reattachment, and replay in
  5/5 dogfood trials.
