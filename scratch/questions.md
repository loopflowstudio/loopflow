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
- An answer whose `revisions` is empty (changed nothing, or the CLI transport
  that cannot say) drops its patch at once rather than at the next reading.
- Resend: `PendingChanges` resends a change under its request ID while
  `ServerLink.lostLink` holds (disconnected, unanswered, POSIX), backing off to
  4 s, with no cap. A Machine that never returns keeps the patch until quit.
- Planning rows: Complete marks the row done (it stays in Done lists, as the
  table says) rather than removing it; Abandon removes it.
- Claim's owner comes from a `viewer: TaskOwner?` on the repository's
  `task_list` reading (concept review pass 2, item 1); with no Git identity
  the Claim moves the section only.
- Rename refusal closes the edit and the typed name is gone; the Notice says
  why. Before, the typed name stayed in the field with its error.
- Desktop counts any end of the Work observation as the link lost; the next
  frame returns it.
- `WorkModel.change`/`request` stay as two-line private helpers binding the
  window's Notice and checkpoint to `PendingChanges.make`/`request`; the
  send/answer/refuse switch lives only in `PendingChanges`.
- `outcome_unknown` without revisions counts as refused: the row returns and
  the Notice carries the message; the next reading shows what happened.
- Read failures left inline, each outside a `WorkModel` reading: the Graph
  step's on-demand history (`GraphRunStep`, no model) and the telemetry
  window's doctor read. Action errors (Warp open, New session) post Notice
  events; `conversationError` stays as the conversation pane's content.
- New session failure text: "<ID> New session failed: <reason>", keyed per
  Task, so a second Task's failure replaces the first in the one event slot.
- A failed read no longer disables Task controls; only away and a saved,
  unconfirmed workspace do.
- `RemoteMobileClient(savedMachine:endpoint:)`: the endpoint is where control
  connections open, which `TaskFeedTests` replace with a stand-in carrier.
- `viewer` takes the display name from `participant_name()` as the claim does
  (the server's cwd), not `participant_name_in(repo)`.
- "Blocked Sessions" result stays an alert (multi-line report of outcomes,
  not a failure); only its failure moves to the Notice. `ProjectHistoryView`'s
  read failure stays inline with the other model-less on-demand reads.
- Pass 5 Notice texts: "Could not move blocked Sessions: …", "<ID> Could not
  prepare checkout: …", "Could not complete the action: …", "Could not change
  pane: …", "Could not run on <Machine>: …". The Machines list is one
  `WorkModel` reading per window, shared by every Machine control in it.
