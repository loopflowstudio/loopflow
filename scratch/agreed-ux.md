# Agreed UX changes — October 18–20

Jack Heart agreed these in demo review; §1–4 are implemented, §5 in part. PR #1580
publication is for demo review; merge and completion are separate.

## 1. Unstarted Task tiles offer Start (October 18)

Jack Heart: “i am ok with it looking something like what you get with the "move
this conversation here" -- the task session window just becomes a thing you cna
click to start the system”.

Implemented: `TaskStartOffer` fills the Session slot of unstarted, open Tasks
(Dashboard tile and Task workspace) through `SessionSlotOffer`, shared with Move
here. Start calls `WorkModel.startTask(nil, …)` (`lf task run`); selection and query
membership start nothing. Shell/Files stay off until start. Not agreed: what a
completed Task with no remaining Session shows (unchanged).

## 2. Repo chevron becomes a jump bar (prototype B, October 18)

Jack Heart, of `scratch/chevron-ux/index.html`: “i think B is the closest”.

Implemented: crumbs navigate as before; each › is a `WorkJumpSeparator` popover over
`WorkModel.jumpRows` (repo › Waves and Dashboards; Wave › Tasks; typing searches that
scope), sharing `WorkModel.jump(to:)` with Find Work. ⌘K is unchanged.

## 3. No machine crumb; a Machine control on the Work (October 19)

Jack Heart: “This mac shouldnt be in our design. Our model is different. the model
is not grouped by machine, model is grouped by work, machine is somethign you can
set on a per wave or per task basis. What we want is a place to both view and set
the machine that this work is happening on”.

Implemented: breadcrumbs are `repo › Wave › Task`; the machine crumb and its
repositories menu are gone (other repositories open through ⌘K). `WorkMachineControl`
sits at the trailing end of the breadcrumb row for the current Task or selected
Wave. It names the execution Machine and its source: set on this Task/Wave, from
its Wave, default, or — for a started/admitted Task — started there, read-only
(moving it is explicit takeover, not built here). The menu lists the local Machine
and saved Machines, plus “Inherit from <Wave>” for Tasks.

Both surfaces use one CLI owner over `DurableStore::place_work`: `lf wave place
<wave> [machine]` and new `lf task place <issue> [machine | --inherit]`, each
printing `WorkMachine` (`tests/fixtures/dto/work_machine.json`). Task placement
refuses started or admitted Tasks atomically; the execution source is the shared
`task_execution_in` read that `task_execution_route` also uses.

Flag for demo review: the local Machine reads “This Mac” inside the menu; no
reachability is shown (only `lf machine status` probes, over the network);
repository and Dashboard views have no control; a Dashboard crumb keeps `/` before
its Task; the start offer takes 62% beside the brief.

## 4. Working Tasks is every started Task of yours (October 20)

Jack Heart: “The default sidebar is WOrkign Tasks = All tasks that are started on
any of your machines we know about by you (and not completed)”.

Observed on installed 0.14.9 (October 20): Working Tasks showed 0. It is built
from visits (#1594, `WorkNavigation.retainWorkingTask` + `WorkCache`), so a fresh
window never lists running work; the cache held empty lists for all 8 repositories.

Jack Heart, October 20: “that should be a Query in the same sense as the dashboard.
If you click through a wave then the sidebar should have a back button to go back
home, to the repo, or it should show all the work items. Once you click into a wave
you should see all the work items in that wave as well as all subwaves And there are
no subways yet” (“subways”: subwaves).

Implemented: the home sidebar lists the repository built-in plan's Task members
(Rust `SelectionExpression::Repository`: started and not done), in plan order
(attention first, then rank). Visit retention (`WorkNavigation.retainWorkingTask`,
`WorkCache.saveWorkingTasks`) is deleted. Selecting a Wave scopes the sidebar to
that Wave's built-in plan — its uncompleted Tasks (started and unstarted) plus child
Waves — with a back control to the repository. Explicitly paused Tasks now stay in
Working Tasks: they are started and not completed.

“By you”: lfdb records no starter; it is one person's store, so every Task started
in it counts as Jack's. Other Machines' starts count once this store records them
(`tasks.started_at`); peer-only readings are not merged. See questions.md.

## 5. The default Dashboard is Working Tasks as a grid of slots (October 20)

Jack Heart: “can we make so that the default dashboard is just all the tasks in that
sidebar, but as a grid view”; “lets set a limit of 8 per screen or something and then
institute (side to side) paging beyond that”, corrected to “the lay out is 7 panes at a
time”. Layout, verbatim: “(1 | (2 | 3)) | (1 | 2 | 3 | 4)”; “the first goup / second
group is a horizontal split”; “inner splits are vertical”. So: a top band of slot 1
beside the pair 2 | 3, over a bottom band of four columns. Jack chose the golden
proportions in `scratch/slot-layouts/index.html` (“i like the golden layout”): top band
61.8% of the height, slot 1 61.8% of its width, 2 and 3 split the rest; bottom four equal.

Slots: “you can add slots and rearrange them to your dashboard, but theres always a
fixed number of slots”. Tasks fill slots in query order and overflow to further pages;
they never grow or shrink the layout. Order: “you can reorder the tasks but they always
get arranged in order”; “the order of tasks should also apply to the sidebar and
reordering in sidebar should do same thing” — one shared order, nobody pins a Task to a slot.

Focus: “we will need this built into our sizing system so that even after you rearrange
the setup arbitraily, theres a way to expand and shrink everything else”; then “lets
build it” for the tree rule (prototype tab “7 slots, golden, tree rule”): at each split
on the path to the focused pane its branch takes 61.8% and the siblings share the rest
by base weight; unfocused, base weights apply; focus never rewrites them; nested panes
grow less. Animation: “make sure to add some nice animation to smooth out the
transiitons” — one ~0.25 s spring for focus resizing and page turns. Jack is still
weighing an 8-slot 4×2 alternative; not chosen, not built; geometry stays data.

Implemented: sidebar Dashboards › Working Tasks (`WorkNavigation.Destination.workingTasks`)
shows the repository built-in plan's Tasks, in sidebar order, through
the plan's `slots` (Rust `SlotLayout::golden`, `DashboardSlotGrid.swift`); 7 per page, ⌥⌘←/→ or the
chevrons turn pages with a push transition; empty slots stay as dashed outlines.
`FocusSizing` grows the focused tile at every split of any slot layout, never writing
weights. Order: lfdb `task_orders` per repository, written by `lf desktop order task:A
--to task:B` (A takes B's place); sidebar rows and grid tiles drag to it. Slots: Rust
`SlotLayout` (golden default) on every plan; saved Dashboards store theirs in
`dashboards.slots_json` via `lf desktop slots`, edited under Edit Slots (add beside/below,
remove, drag to exchange places, Default Slots). Remaining work is in design.md.
