# Open choices (LOO-894)

- The brief's acceptance asks for a CI run "with a deliberately wrong key id"
  showing the 401. Adding a `workflow_dispatch` key-id input exists only for that
  test, and dispatch only works once the file is on `main`. Plan instead: the
  local probe already shows the 401 text; on CI, the still-unsigned agreement
  gives a real red run (403) that exercises the same fail-and-open-issue path.
  QA (2026-11-13): the agreement is accepted now, so no red CI run will come on
  its own; the wrong-key-id 401 path ran locally against Apple instead.
  Reversible: add the input later if a CI-side 401 run is still wanted.
