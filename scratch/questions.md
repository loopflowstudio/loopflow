# Choices (LOO-865)

- 2026-11-13 implementation diagnostic: process command-line output included Session capture tokens in the run log. Invalidate the exposed tokens; no values are repeated here. Subsequent process inspection uses PID, state and executable name only.

- Earlier Graphs remain unattached when a Task adopts their checkout; no backfill or UPDATE path. QA 2026-11-13 (`scratch/qa.html` §5) found that adoption while a Graph runs recommended `run` and allowed a second Graph. Iteration direction: "Update Task status and `lf task run` to account for a live Graph in the checkout without changing the recorded Task association or adding an update path". Open Process execution now prevents overlap; Graph history still reads only the recorded association. Task stop/session moves still address only the recorded Task Graphs.
