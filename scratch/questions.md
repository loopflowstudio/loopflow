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

October 17 implementation assumption: unsaved ad hoc query destinations remain
read-only compositions until saved as an independently named Dashboard. Their
Task panes/drafts still share Work ownership; this avoids inventing another
arrangement store for anonymous queries.

October 18 (evening): Jack Heart's unstarted-Task Start offer and jump-bar (B)
decisions, with quotes, are in `agreed-ux.md`; implemented October 19. The Start
offer replaces the exclude-unstarted leaning.

October 18 (evening), scope: Jack Heart agreed LOO-497 becomes the Dashboards
outcome (demo, review and merge on PR #1580). The scripting-API acceptance from the
original brief (state readback oracle, exit statuses, scoped context) moves to
LOO-519, coordinated with LOO-453 (readiness wait/events).

October 19 assumptions: a Dashboard crumb keeps `/` before its Task (B puts › only
after repository and Wave). `TaskWorkflowHeader` has no production caller on `main`;
its deletion belongs to that header's owner. Machine control choices beyond Jack
Heart's direction (local “This Mac” label, no reachability, no control on repository
or Dashboard views) are flagged in agreed-ux.md §3.

October 20 assumptions (agreed-ux.md §4): paused Tasks stay in Working Tasks,
superseding the October 17 Console rule. The Wave sidebar/built-in plan lists
uncompleted Tasks, unstarted included, plus child Waves. Flag for demo: plan order
(waiting first) moves ⌘1–9 positions. “Started” is defined by the October 21 note below.

October 20 assumptions (agreed-ux.md §5): binary Dashboard splits apply the focus rule per
split (one sibling gets 38.2%); the default layout is n-ary data so the bottom band is one
four-way group, matching the prototype. A Dashboard always has a focused tile (the
registry focuses the first member), so slot 1 starts focused, which equals the golden
base. Terminal views take their final frame rather than interpolating; confirm no
flicker in the demo. Plans now report `slots`.

October 20 order/slot assumptions: the person's Task order comes first; Tasks it never
named append after it, waiting first. It is one repository order, so Wave sidebars and
unarranged Dashboards follow it too. A drop puts the dragged Task in the target's place.
Every saved Dashboard renders through slots; its visible occurrences fill them in
order (October 21 concept review, design.md).

Historical notes removed October 20 (full text at `f492be7f19:scratch/`): the October 16
Arrange handoff (owners: LOO-416 Session retention, LOO-453 readiness/events, LOO-422/415
status transport, LOO-414 remote launch, LOO-498 external multiplexers) and the October
18 operator readback (PR #1580 demo-only; demo review in Session
`session_421decfc3b31459fb10e4771e231a704`).

October 21 sync onto main (#1610, Task ownership): main deleted placement, admission and
`started_at`, so this branch's `lf task place`, `lf wave place` readback, `WorkMachine`
DTO and the Desktop breadcrumb Machine menu were removed rather than ported. "Started"
for Working Tasks now means a current ownership-log owner or a published PR. Settled by
the October 21 concept review (design.md): Jack's October 19 request (“a place to both view
and set the machine”) still stands, so the control returns over main's `lf machine use`.

October 21 implementation assumptions: the Machine control appears only on started Tasks
(an unstarted Task's Machine is chosen at start from its Wave) and on Waves; repository
and Dashboard views have none. "This Mac" heads the choices; a Wave with no default
reads as this Machine. Auto-mode Dashboards follow query order (the old 2:1 attention
weighting sized nothing once slots owned geometry).
