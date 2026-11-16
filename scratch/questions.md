# Open choices (LOO-931 kickoff, headless)

- Phone Abandon: the brief's first bullet says Complete and Abandon on
  "Desktop and phone", and lists Complete, Start, Auto-Ship and Stop Auto-Ship
  as what the phone gains. Planned: phone gets the four named; Abandon stays
  Desktop-only. Adding it later is one row action.
- Next-Task rule: LOO-906 wrapped to the top of the list; main picks the row
  below, else above. Planned: below, else above, else the empty state (the
  brief says "or show the empty state", replacing main's fallback to the Wave).
- `WavePlanningNotice` is about the plan, not an action someone took. Planned:
  it becomes a Notice raised once per board load with "Copy recovery", as the
  brief lists it among the five.
- Protocol 7 with `min_revision` 7 refuses older phones against a newer
  Machine (and the reverse) with the existing incompatible message.
- `command_answers` vs `creation_receipts`: two request-ID memories until
  LOO-932 folds creation into the generic one. Flag for LOO-932.
