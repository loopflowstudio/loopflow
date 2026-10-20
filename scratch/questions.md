# LOO-522 open questions and assumptions

- Assumption (2026-10-19 kickoff): the brief's acceptance item "regression test
  with another attached/live conversation" is replaced by the gate-path handoff
  test. Store evidence shows the Task conversation played no part. The real
  failure was steps re-delegating after the entry-gate exec dropped the handoff.
  This is reversible: add a sibling-conversation test if Jack Heart still wants one.
- Assumption: installing the fix (and v0.14.8's #1600) is Release's work, not
  this Task's. Until a build containing #1600 is installed, installed Flows keep
  stopping at routing steps.
