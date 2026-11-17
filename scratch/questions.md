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
- **A saved layout keeps every Session it names** (concept review): the
  first-page filter destroyed valid references, and a Mac-local file needs no
  ownership check. See the design's next implement.
- **`--diff` still adds Changes** beside the conversation in Rex/Herdr/cmux; it
  is an explicit request. A Task without a local checkout shows Task & workflow.
- **`lf desktop layout` stays as a read of `query:<id>`**; `--save` is gone
  (`lf desktop dashboard slots` writes slots).
- **Dashboards stay on the Machine** with their planning triggers dropped, so
  edits wake no planning reader. Still open, for LOO-940 rather than this PR:
  is a dashboard shared per repository (a saved view everyone sees) or one
  client's screen? `dashboard_arrangements.arrangement_json` is screen state
  keyed by repository, the shape the outcome says screen state should leave;
  if dashboards are per client, both tables move to Desktop like pane layouts.
  Are they used at all?
- **`tasks.regressed_in` stays** (LOO-730): released peers refuse a Task that
  arrives without `task_regressed_in`, and a peer edit writes the column.
- **`due_removals.py` schedule**: `install`/`uninstall`, Monday 09:00, log at
  `~/Library/Logs/loopflow-due-removals.log`, run through `uv` because macOS's
  system Python SQLite rejects the released migrations.
- **SCHEMA.md names Task and marking migration, no date**: a date would change
  at merge and fail `--check` on main.
