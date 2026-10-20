# Agreed UX changes — October 18–19

Jack Heart agreed these in demo review; all three are implemented. PR #1580 is
for review only — no merge, no completion.

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
