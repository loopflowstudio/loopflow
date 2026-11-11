# LOO-823 open: which input holds a detached owner

Today any byte that isn't plain text or a recognized terminal reply makes the
owner's input `Uncertain`, which holds it past `end_detached_after` until
input is cleared. 78 of ~100 resident owners on 2026-11-10 were held this way.

1a. Keep it: any unknown key holds the owner; the troubleshooting doc tells
    people to `lf resume` and clear it.
1b. Cursor and navigation keys on an empty editor (Left, Right, Down, Home,
    End, PgUp/PgDn) leave input empty; Up, Esc, edits and text still hold
    (Up recalls the last prompt in Claude).
1c. Keep 1a's rule, but an owner held only by `Uncertain` (no recorded text)
    ends at rollover after a fixed age; a typed draft still holds.
1d. Keep 1a's rule and show "held by input" in `lf process list` and Desktop;
    nothing ends on its own.

The same hold reaches finished Tasks. Read 2026-11-10 18:55: LOO-775, LOO-776,
LOO-812, MTG-252 and MTG-268 are done, their `task_retirements` rows were due
26-33 h earlier and still `waiting` with "Requested work, a draft or a pending
request still holds this Task". Each Task's interactive owner (PIDs 80631,
4996, 63058, 78405, 38179) has `native_input`
`{"state":"Uncertain","escape":[],"working":false}`: no text. Six Desktop
shells and five checkouts stay behind them. 1b or 1c would release these;
1a and 1d keep them until someone resumes each conversation.

`native_input.rs` stays as designed until a person picks one.

## Concept review with a person (2026-11-11, at demo)

Page: `scratch/concept-review.html`. Its questions replace 1a-1d above.

Read from `native_input.rs` and the store, 2026-11-11 ~01:10:

- `Uncertain` has no way out except the agent exiting (`native_ended` →
  `clear_owner_input`). `accept` keeps it, `start` keeps it, `stop` clears only
  `Acknowledged`. A later clean prompt does not settle it.
- It is entered by Backspace, Enter on an empty editor, Ctrl-C, any escape
  sequence that is not a terminal reply (arrows, Esc, paste), and any key while
  a turn is working (a queued message, a permission answer).
- Live owners with an open interactive Session: 85, all `Uncertain` (79 idle,
  6 mid-turn). Owners ended in the last three days: 15 `Empty`, 13 no record,
  2 `Uncertain`, 1 `Text`. Idle ending works for untouched input; everything
  that accumulates is `Uncertain`.
- `docs/troubleshooting.md` in PR #1877 says "send or clear what is there; the
  owner ends on schedule once its input is empty". No code path does that.
  `docs/sessions.md` has it right: "until the provider exits".
- Old 1c said "a typed draft still holds". An edit discards the recorded text,
  so an age limit on `Uncertain` also ends every draft that was ever corrected.

Proposed, not built: a hold has a way out. A prompt the provider acknowledges
and finishes, with no key recorded since the Enter that sent it, leaves input
empty whatever came before. Prompts lf injects (steers, `desktop send`) never
settle it. Counterexamples kept: a draft typed after queuing a message; a
backslash-Enter newline followed by an injected prompt; providers with no
acknowledgement (Codex) still have no way out.

Open, numbered as on the page:
1. What releases a held conversation (1a nothing but quitting; 1b a sent
   prompt; 1c 1b plus an age limit; 1d only ignore navigation keys).
2. Whether `lf process list` and Desktop show what keeps each one (2a/2b).
3. Whether a done Task's cleanup waits on unsent input (3a/3b/3c).
4. Whether this lands in PR #1877 or a new Task (4a/4b).

### Direction given in the review (2026-11-11)

Jack Heart, on "An unviewed, settled conversation ends its processes after two
minutes of quiet": "Im not sure we need this nay more, or at least it should
change -- with bare lf, noting holds its at all"

Jack Heart: "with task sessions, i think we can just hold them until the task
is closed basically"

Proposed from that, not built: a conversation lives as long as what it belongs
to. Bare: its terminal (the unbuilt "Bare launches go direct" piece of
`docs/dev/server-first.md`; today every launch still gets a detached owner).
Task: until the Task is done or abandoned, then cleanup ends it whatever its
input. Idle ending, `session.end_detached_after`, the `native_input` ledger
and `OwnerRetention::NativeInput` go; questions 1 and 3 above dissolve.

Store read ~01:40, live owners with an open interactive Session by scope:
43 Task without a retirement row, 18 Task with one, 2 Task abandoned,
7 primary Wave, 2 primary repository, 14 no Task or Wave.

Still open: what holds Wave and repository conversations; what happens to
the 14 unscoped owners before bare-direct lands; whether this is LOO-823 or
its own Task. Numbered on the page.

Jack Heart, on question 1 (Wave and repository conversations): "nothing ends it
but i could be open to allowing soem sort of "idle" state for task and
wave/repo sessions on the server, if we ahve some cache limit, as long as only
cost is slightly slower spin up time next time youj switch to them"

Proposed from that, not built: a conversation runs until its Task closes (Task)
or until you close it (Wave, repository). Separately, the server may suspend a
conversation to stay under a resident limit, least recently viewed first.
Suspending stops the agent and keeps the saved screen; switching to it resumes
from provider history. It is only allowed when that loses nothing, so it skips
a live turn, a pending request, a running Graph and unsent input.

That brings back the one hard part: knowing the editor is empty. The byte
ledger can't (every live owner reads `Uncertain`). The server's terminal
emulator, planned in server-first, can read the agent's input line off the
screen. Question 5 on the page.

Jack Heart: "wave/repo should behave exactly like open tasks tho"

So one rule for all three while open: runs, may be suspended under the limit
when nothing would be lost. Only a Task has an end of its own (done or
abandoned); a Wave or repository conversation has none, so it is an open Task
that never closes.

Jack Heart on 5 and 6: "no opinion on 5" / "6a"

Proposed for 5, no one has picked it: 5c now (suspend only a conversation with
no keys since its last finished turn), 5a once the server's emulator exists
(read the input line off the screen, delete the byte ledger). 6a: a resident
agent count per Machine in config; the default is unnamed.
