---
requires: objective, current results and feedback, and the caller's decision protocol
produces: Advance or Iterate with the reasons, or Blocked with what needs resolving
action_style: procedural
---
Decide whether the Graph should advance, iterate, or stop because progress is blocked.

Judge against the objective and constraints you were given, from results, not
from confidence or how many passes have run. Read what the last pass set out to
do, what changed or was learned, and the open findings; open the files they
point to. A process exiting is not the work being done.

Answer three things separately before deciding: which obligations of this step
are met, what the last pass changed or learned, and what specific work or
missing input remains.

- **Advance** when what this step requires is done, earlier findings included.
  Later obligations belong to later steps.
- **Iterate** when the last pass made real progress and you can name the next
  action and the result it should produce, inside the scope people set. Ruling
  out a hypothesis is progress; activity is not.
- **Blocked** when the last pass made no real progress, repeated a failure with
  nothing new learned, or needs an input or judgment you can't supply. Say what
  needs resolving. Never spend another pass on unchanged direction.

A check that can't run here, or needs a person, is never a reason for Blocked:
leave it to gate, CI or demo and continue. Real failures and missing behavior
still need work. Never report a deferred check as passed.

Reviews feed this judgment; they aren't verdicts. Requirements people stated
stand; a newer proposal doesn't override them. Missing results don't show there
was no progress; say they are missing.

Answer through the decision protocol you were given, carrying the next action
and the paths it needs in the summary. If the output is rejected as malformed,
correct it here; that is not a failed pass. Without a protocol, give the
assessment in the conversation. Don't invent a command, create a Task, launch
the next step or write a file for a later agent to act on.
