# Agreed UX changes — October 18 (evening)

Jack Heart asked a pursue worker to make the UX changes agreed in this review.

## 1. Unstarted Task tiles offer Start

Jack Heart: “i am ok with it looking something like what you get with the "move
this conversation here" -- the task session window just becomes a thing you cna
click to start the system”.

- Where a started Task shows its primary Session (Dashboard tile or zoomed Task
  workspace), an unstarted Task shows a start offer in that same slot, styled like
  LOO-491's move-here offer. Reuse that view; don't build a
  second one.
- Clicking it starts the Task through the existing start path (the same one the
  CLI/Desktop already uses to start Work). Selection, query membership and opening
  the tile still start nothing.
- Unstarted Tasks stay in queries. The empty Brief/Changes/“Select a file” state
  without a Session area must no longer be reachable.
- Not agreed: what a completed Task with no remaining Session shows. Leave its
  current behavior; note the question.

## 2. Repo chevron becomes a jump bar (prototype B)

Jack Heart, of `scratch/chevron-ux/index.html`: “i think B is the closest”.

- Breadcrumb names navigate as today (repo name → repository conversation, Wave →
  Wave, Task → Task).
- Replace the separate repo chevron with › separators. The › after a crumb opens a
  menu of what is inside it, with a filter field: leading machine crumb › →
  repositories (another repository opens its own window); repo › → its Waves and
  Dashboards; Wave › → its Tasks. Typing in a menu searches that scope.
- ⌘K keeps opening Find Work. The chevron no longer opens Find Work.
- “Closest”, not final: keep to B's behavior; flag details for demo review rather
  than inventing extras.

## Implemented — October 19

1. `TaskStartOffer` fills the Session slot of unstarted, open Tasks (Dashboard tile
   and Task workspace, beside the brief) through `SessionSlotOffer`, the view Move
   here now shares. Start calls `WorkModel.startTask(nil, …)` (`lf task run`).
   Shell/Files/new-terminal commands are off until the Task starts, so the empty
   Files workspace is unreachable for unstarted Tasks. Completed Tasks unchanged.
2. `WorkBreadcrumbs` renders `This Mac › repo › Wave › Task`; each › is a
   `WorkJumpSeparator` popover over `WorkModel.jumpRows`. Choosing goes through
   `WorkModel.jump(to:)`, which Find Work's `navigate` now shares. The chevron
   that opened Find Work is gone; ⌘K is unchanged.

Flag for demo review (choices beyond “closest”): “This Mac” is plain text, not a
destination; a Dashboard crumb keeps `/` before its Task; menus have no Open/Clone
repository rows; the start offer takes 62% beside the brief, like a Wave's split.

## Proof

Headless Swift view tests for both (see AGENTS.md Verification Cadence). Native
demo stays demo-owned. Publish to PR #1580 for review only — no merge, no completion.

## 3. No machine crumb; a Machine control on the Work — October 19

Jack Heart: “This mac shouldnt be in our design. Our model is different. the model
is not grouped by machine, model is grouped by work, machine is somethign you can
set on a per wave or per task basis. What we want is a place to both view and set
the machine that this work is happening on”.

- Remove the `This Mac ›` crumb and its repositories menu. Breadcrumbs are
  `repo › Wave › Task`. Other repositories stay reachable through Find Work (⌘K),
  which already opens them in their own window. (Agent choice; flag in demo.)
- Add one Machine control for the current Work (Wave or Task), at the trailing end
  of the breadcrumb row. It shows the effective Machine and where it comes from:
  set on this Task, inherited from the Wave (name it), or recorded for a started Task.
- Setting it: the menu lists saved Machines (`lf machine list`, with reachability
  where known). Choosing one places the current Wave or unstarted Task on that
  Machine; “Inherit from <Wave>” clears a Task override (null restores inheritance).
  Wave placement uses `lf wave place`. Add the missing Task equivalent over the
  existing `DurableStore::place_work` (e.g. `lf task place <issue> <machine>`), the same
  path the Desktop control calls. No second placement store.
- Accepted model (product memory, October 9): unstarted/future Tasks inherit the
  nearest explicit Machine; narrower Task overrides win; started Tasks stay on their
  recorded Machine. So for a started Task the control shows the recorded Machine
  read-only and says moving it is explicit takeover, which this Task does not build.
- The control shows the execution Machine, never the Machine Desktop is displaying on.
