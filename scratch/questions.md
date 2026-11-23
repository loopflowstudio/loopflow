# Open choices (made headless, for review)

- A steer from anyone but the Run's own Graph does not wake a finished Run
  (non-interactive Session); it is saved and reported unread. Passed over:
  any named steer wakes a finished Graph step.
- The Machine server's wake watcher starts a stopped interactive Session in a
  held terminal (`session.continue`'s launch); the steering command only
  saves. Passed over: the kickoff's detached `lf -b session wake ID` started
  by the steering command, with its own lock and loop (a second waker).
- Each steer starts its Session at most once; existing undelivered steers are
  marked as already started, so a server restart or the upgrade wakes nothing
  old.
- With no Graph step running, a Task steer wakes only the primary
  conversation. Passed over: holding it for the next step; waking the last
  finished step.
- `lf -b steer` to a running Session saves and returns.
- `lf steer` exits 0 once saved. `lf -b steer` exits with the turn's failure.
- A foreground turn's prompt carries each steer's full text, not the hook's
  400-byte excerpt.
- `lf -b steer` marks the steers delivered when it builds the prompt, before
  the provider starts; a launch that then fails leaves them read. Passed
  over: claim at save, deliver after launch (a claim lapses in 15 s and a
  slow start would print them again through the turn's hooks).
- `lf -b steer` holds the Session's launch lock for the whole turn, so a
  `lf resume` of the same Session meanwhile waits 30 s and fails as busy.
- A resumed interactive conversation takes its unread steers as its first
  input whenever one names it, whoever resumes it (`lf resume` too), not only
  a server start. Passed over: a server-only signal to the held start (a
  second, hidden way to pass a message to `resume`).
- The server's record of what it has read for starts is one cursor over
  `steers.seq`, not a per-steer column: a steer to a running Session is read
  past too, so if that Session quits before reading it, nothing starts it.
- Steer 119ddcde (2026-11-17) settles the launch names: "managed+typing = work
  session, managed+headless = worker, bare+typing = chat, bare+headless =
  batch", "Use these words in docs". User-facing text here now says worker or
  batch (slice 5); the `harness::Run` type's rename to `LaunchKind` is
  LOO-924's.
- QA 2026-11-17: a held start takes every unread steer as its first input and
  marks them delivered before the provider runs. A start that never runs a
  turn (killed at a dialog, or a resume of a thread Claude never wrote, which
  exits 1 with "No conversation found") leaves them reading `delivered`, and
  nothing starts the Session again for them. Same trade as `-b steer`'s; the
  plan's "a held start that fails leaves the steer undelivered" holds only for
  a start that fails before launch.
- Post-build concept review 2026-11-17: the start set and the take rule
  are one SQL predicate (`naming()`, slice 4, built). It also counts an
  unlapsed claim as read, so a steer a hook is printing right now starts
  nothing. Passed over: leaving both.
- QA 2026-11-17 (slice 4 rerun): `lf steer TASK` prints only "Saved …"; it
  does not say the server starts the Task's quit primary conversation, as
  `lf steer SESSION` does. Left as is; a reader line for Task steers is a
  small follow-up if wanted.
- Steer 474316e8 item 2 (from the LOO-924 concept review): "If a Desktop pane
  or lf --machine can create a work session that is neither [bound nor
  primary], lf resume ID from a terminal would reopen it as a chat."
  `SqliteStore::is_bare` and `transport::session_run` exist only on LOO-924's
  branch, so nothing here can change until it merges. Reading of that branch
  (2026-11-17, not run): a fresh Session is a work session only when
  `delegate_current` runs inside a held terminal (`transport::current()`); a
  Desktop pane's own shell and `lf --machine` (ssh dispatch runs `lf` remotely
  outside any held terminal) both start chats. The one unbound, non-primary
  work-session launch is a held start of a chat: the phone's Continue, or this
  branch's server start for a steer (`session.continue` → `lf session resume`
  in a held terminal, where `session_run` keeps its parent's choice). A later
  `lf resume` from a terminal reopens it as a chat, which is what the Session
  was. Left as is; recheck after LOO-924 lands.

- Sync onto main: use main’s launch kinds and Session-owned agent identity for
  steer delivery. Crash recovery and named steers share the existing held
  terminal start. The old stacked harness implementation is superseded.
