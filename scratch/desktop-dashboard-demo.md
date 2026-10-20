# Desktop Dashboard demo attempt — October 18, 2026

Full note: `08253c4dda:scratch/desktop-dashboard-demo.md`.

A populated-clone demo at `e08cbb102d` failed before app launch: clone creation hit
SQLite `SQLITE_IOERR_GETTEMPPATH` (6410); a retry with clone-local `TMPDIR`/
`SQLITE_TMPDIR` progressed but fell below TESTING.md's 32 GiB disk reserve
(8.4 GB database + 6.9 GB WAL, not ready). The clone was disposed; no app was
replaced. The later synthetic-fixture review in
[dashboard-continuity-review.md](dashboard-continuity-review.md) superseded it.

Open engineering recommendations (not agreed UX, not launched work): clone creation
should choose a writable temp dir inside its sandbox and budget its peak footprint;
live `desktop state`/control target `com.loopflow.mac`, so a separately identified
preview cannot show candidate CLI↔native round-trip parity.

## Native demo on a copy of Jack's lfdb — October 19, 2026

Build: Loopflow Dev at `3dd981e645` + `79bf0d415b`, private `LF_HOME` holding a
VACUUM copy of `~/.lf/loopflow.db` with 0.14.8 and the `saved_selections` draft
applied by SQL (canonical preflight only takes fences). Launched with `open -n --env`.

Observed (Jack Heart's screenshots):
1. “Couldn't update planning: sqlite error: malformed JSON”, Working Tasks 0, warnings
   on every Wave. Cause: `process_input` (#1600, shipped in v0.14.8) ran `json_extract`
   on `processes.command`, which is bare `codex` for 3,113 agent Processes. Fixed
   `79bf0d415b` (regression in `step_input_is_read_through_transport_delegation`); also
   landing on main via LOO-522's PR #1604. After the fix Waves load with counts.
2. Dashboard `booo` (`state:available`) lists ~24 lines “Task task_… activity is
   unavailable”. Cause: `selections.rs` marks a Task's activity unknown when any
   uncompleted Session has `ProcessIdentityEvidence::Unknown`; real history has many
   never-completed Sessions. Defects: stale Sessions poison activity, and `unavailable`
   is rendered as raw IDs instead of one summary. Applies to real data. Not fixed.
3. Working Tasks still 0 after the fix. Unconfirmed: the private home may be a
   different Machine, so no started Task is local. Needs checking before judging.
4. LOO-426 (Unstarted) tile shows an empty box, not the Start offer. Not diagnosed.

Not yet exercised: jump-bar menus, Machine control, Start, Dashboard arrangement.
