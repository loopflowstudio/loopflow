# LOO-497 current decisions

Jack Heart accepted the prototype for implementation, requested integration with
LOO-489, corrected the invisible query aliases, then said “keep going.” The
materialized 489 source and current procedure are in native-console-integration.md.
Earlier decisions/readbacks are preserved in
`8d4f2d8ea1430fd41d946d328145ce87e26c7b7b:scratch/history/loo489-console-source-2026-10-17/loo497-prior-questions.txt`.

Repository scope is implicit. Cmd K finds Work across repositories; another repo
opens its own window. Separate Dashboards never replace ordinary repo/Wave views.
Unstarted matches show briefs and cause no Session, checkout or execution effects.
Use one Shell and one Files tool per Task, with tabs; higher scopes have neither.
File close preserves its Task/path draft. Shell tab ID owns terminal identity;
restored intent cannot replay launch arguments or imply a running process.

Shared grammar, clause IDs, expression-specific arrangement history and Dashboard
identity already exist. Direct predicates replace invented prototype aliases.
Genuine saved references must have a visible owner. Full maps preserve hidden
occurrences; first visible wins independently of focus. Newer edits survive restore.

497 now owns adoption of its Dashboard/editor/persistence callers in the combined
Console. Existing 489 navigation and 416 lifetime owners are reused, never cloned.
Source handoff is available locally; do not repeat that historical blocker.
Cold Process reattachment and native mounted retention still need real evidence.
Publish for demo review only. No merge or automatic completion authorization.

October 17 implementation assumption: unsaved ad hoc query destinations remain
read-only compositions until saved as an independently named Dashboard. Their
Task panes/drafts still share Work ownership; this avoids inventing another
arrangement store for anonymous queries.

October 18 (evening), Jack Heart on unstarted Tasks in queries: “i am ok with it
looking something like what you get with the "move this conversation here" -- the
task session window just becomes a thing you cna click to start the system”. An
unstarted Task's tile/workspace shows a start placeholder in the Session slot
(modeled on LOO-491's move-here offer); clicking it starts the Task. Selection
alone still starts nothing. Unstarted Tasks stay in queries; this replaces the
exclude-unstarted leaning and the empty Brief/Changes workspace.

October 18 (evening), repo chevron: of the three prototypes in
`scratch/chevron-ux/index.html`, Jack Heart said “i think B is the closest” —
the jump bar: a crumb name navigates; the › after it opens a filterable menu of
what's inside (machine › repositories, repo › Waves/Dashboards, Wave › Tasks);
typing in a menu searches that scope. Details beyond "closest" are not yet agreed.

October 18 (evening), scope: Jack Heart agreed LOO-497 becomes the Dashboards
outcome (demo, review and merge on PR #1580). The scripting-API acceptance from the
original brief (state readback oracle, exit statuses, scoped context) moves to a new
Task coordinated with LOO-453 (readiness wait/events).

October 19 implementation assumptions (agreed-UX pass): the jump bar's machine crumb
reads “This Mac” and does not navigate (no machines destination exists); a Dashboard
crumb keeps `/` before its Task because B puts › only after repository and Wave
levels. `TaskWorkflowHeader` (the old Workflow Start header) has no production caller
— only tests mount it; deleting it was left out of this pass's scope.
