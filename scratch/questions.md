# LOO-935 open choices

## Steers (2026-11-16, word for word)

- Jack Heart: "You can drop everything old in this task as a special case".
- Jack Heart: "we can drop the thing where lf open does anything other than
  just open the task session in other surfaces."
- Jack Heart: "we dont need to try to copy our desktop layouts".

## Picks made headless

- **Desktop layouts live in `$LF_HOME/desktop-cache/`, not Application Support.**
  The Mac app has no Application Support directory; `workspace.json` and its
  other caches already live under `WorkCache.homeDirectory`.
- **An unknown Session in a saved layout clears that pane's Session on load.**
  "Known" is `WorkModel.sessions.value`, the first page of the interactive
  Session listing. A valid older Session or a headless Run outside that page is
  cleared, and the next save keeps it cleared. The server used to check the
  Session against the Work. Open: check against the Work's Sessions instead,
  or stop clearing.
- **`--diff` still adds Changes** beside the conversation in Rex/Herdr/cmux; it
  is an explicit request. A Task without a local checkout shows Task & workflow.
- **`lf desktop layout` stays as a read of `query:<id>`**; `--save` is gone
  (`lf desktop dashboard slots` writes slots).
- **Dashboards stay on the Machine** with their planning triggers dropped.
  Still open: per person, per client or per Machine, and are they used at all?
- **`tasks.regressed_in` stays** (LOO-730): released peers refuse a Task that
  arrives without `task_regressed_in`, and a peer edit writes the column.
- **`due_removals.py` schedule**: `install`/`uninstall`, Monday 09:00, log at
  `~/Library/Logs/loopflow-due-removals.log`, run through `uv` because macOS's
  system Python SQLite rejects the released migrations.
- **SCHEMA.md names Task and marking migration, no date**: a date would change
  at merge and fail `--check` on main.
