# Open choices (LOO-931 kickoff, headless)

- Phone Abandon: the brief's first bullet says Complete and Abandon on
  "Desktop and phone", and lists Complete, Start, Auto-Ship and Stop Auto-Ship
  as what the phone gains. Planned: phone gets the four named; Abandon stays
  Desktop-only. Adding it later is one row action.
- Next-Task rule: LOO-906 wrapped to the top of the list; main picks the row
  below, else above. Planned: below, else above, else the empty state (the
  brief says "or show the empty state", replacing main's fallback to the Wave).
- `WavePlanningNotice` is about the plan, not an action someone took. The
  brief lists it among the five surfaces to remove. Planned (concept review,
  item 7): one condition Notice per Wave while its reading lists
  `unavailableTasks`, action "Copy recovery".
- Protocol 7 with `min_revision` 7 refuses older phones against a newer
  Machine (and the reverse) with the existing incompatible message.
- `command_answers` vs `creation_receipts`: two request-ID memories until
  LOO-932 folds creation into the generic one. Flag for LOO-932.
- `Reading.readAt`: the first pass kept revisions beside readings
  (`WorkModel.readAt[part]`, `PendingChanges.read(subscription, at:)`).
  Concept review item 3 puts `readAt` on `Reading<Value>` and deletes the
  side dictionary.
- An answer whose `revisions` is empty (changed nothing, or the CLI transport
  that cannot say) drops its patch at once rather than at the next reading.
- `PendingChanges` is per `WorkModel` (one per window) today; concept review
  item 5 makes it one per Machine, so a second window sees the change in the
  click too.
- Resend: Desktop resends a change under its request ID while
  `ServerLink.lostLink` holds (disconnected, unanswered, POSIX), backing off to
  4 s, with no cap. A Machine that never returns keeps the patch until quit.
- `outcome_unknown` without revisions counts as refused: the row returns and
  the Notice carries the message; the next reading shows what happened.
