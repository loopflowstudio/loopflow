# Open choices (LOO-931 kickoff, headless)

- Phone Abandon: the brief's first bullet says Complete and Abandon on
  "Desktop and phone", and lists Complete, Start, Auto-Ship and Stop Auto-Ship
  as what the phone gains. Planned: phone gets the four named; Abandon stays
  Desktop-only. Adding it later is one row action.
- Next-Task rule: LOO-906 wrapped to the top of the list; main picks the row
  below, else above. Planned: below, else above, else the empty state (the
  brief says "or show the empty state", replacing main's fallback to the Wave).
- `WavePlanningNotice` is about the plan, not an action someone took. Revised in the concept review: a condition Notice per Wave while the reading lists the Task, replacing the earlier plan of
  it becomes a Notice raised once per board load with "Copy recovery", as the
  brief lists it among the five.
- Protocol 7 with `min_revision` 7 refuses older phones against a newer
  Machine (and the reverse) with the existing incompatible message.
- `command_answers` vs `creation_receipts`: two request-ID memories until
  LOO-932 folds creation into the generic one. Flag for LOO-932.
- `Reading.readAt`: the plan put `readAt` on `Reading<Value>`. Built instead:
  `PendingChanges.read(subscription, at:)` keeps each live part's `read_at`,
  so the reading wrapper needs no new field. `Reading` itself is still to do.
- An answer whose `revisions` is empty (changed nothing, or the CLI transport
  that cannot say) drops its patch at once rather than at the next reading.
- `PendingChanges` is per `WorkModel` (one per window), not per Machine; a
  second window sees the change at the Machine's next reading.
- Resend: Desktop resends a change under its request ID while
  `ServerLink.lostLink` holds (disconnected, unanswered, POSIX), backing off to
  4 s, with no cap. A Machine that never returns keeps the patch until quit.
- `outcome_unknown` without revisions counts as refused: the row returns and
  the Notice carries the message; the next reading shows what happened.
