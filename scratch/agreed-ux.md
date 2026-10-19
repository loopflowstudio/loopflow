# Agreed UX changes — October 18 (evening)

Jack Heart asked a pursue worker to make the UX changes agreed in this review.
Source statements are quoted in `questions.md` and `dashboard-continuity-review.md`.

## 1. Unstarted Task tiles offer Start

Jack Heart: “i am ok with it looking something like what you get with the "move
this conversation here" -- the task session window just becomes a thing you cna
click to start the system”.

- Where a started Task shows its primary Session (Dashboard tile or zoomed Task
  workspace), an unstarted Task shows a start offer in that same slot, styled like
  LOO-491's “move this conversation here” offer. Reuse that view; don't build a
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

## Proof

Headless Swift view tests for both (see AGENTS.md Verification Cadence). Native
demo stays demo-owned. Publish to PR #1580 for review only — no merge, no completion.
