# Desktop Dashboard demo attempt — October 18, 2026

Full note: `08253c4dda:scratch/desktop-dashboard-demo.md`.

A populated-clone demo at `e08cbb102d` failed before app launch (SQLite temp path,
then disk reserve); the synthetic-fixture review in
[dashboard-continuity-review.md](dashboard-continuity-review.md) superseded it.

Main's #1612 (October 20) replaces clone creation with an APFS snapshot and runs
draft builds on `~/.lf/copies/<digest>/` automatically. Still open (not agreed UX):
live `desktop state`/control target `com.loopflow.mac`, so a separately identified
preview cannot show candidate CLI↔native round-trip parity.

## Native demo on a copy of Jack's lfdb — October 19, 2026

Build: Loopflow Dev at `3dd981e645` + `79bf0d415b`, private `LF_HOME` holding a
VACUUM copy of `~/.lf/loopflow.db` with the draft applied by SQL (#1612 now automates this).

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
   is rendered as raw IDs instead of one summary. Applies to real data.
   Fixed October 20: Jack's lfdb had 100 uncompleted Sessions whose agent Process had
   no OS identity, all begun before the last boot, plus 12 never attached. A restart
   now proves such a Process dead (`agent_process_evidence` `began_at`); unattached
   Sessions no longer hide activity; messages name the Task (`FIX-2`), and a Dashboard
   shows one line (`+N more`, all reasons in help).
3. Working Tasks still 0 after the fix. Superseded October 20: Working Tasks is now
   the repository plan's started Tasks (agreed-ux.md §4), not visits.
4. LOO-426 (Unstarted) tile shows an empty box, not the Start offer. Likely cause,
   not reproduced: planning refresh had failed (finding 1), so Swift showed saved
   planning in which LOO-426 was unstarted (it started October 18 per Jack's lfdb),
   and the 24-line wall squeezed the tile's offer out of view. October 20: the tile's
   Start offer now follows the plan's `backlog` state from the same frame, and the
   wall is one line. Needs demo confirmation.

Not yet exercised: jump-bar menus, Machine control, Start, Dashboard arrangement.
