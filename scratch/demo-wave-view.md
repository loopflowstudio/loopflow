# LOO-594 Wave view demo — October 26, 2026

Experience reviewed: the Desktop Wave view on the repository page (product/loopflow),
built from branch `jack/loo-594-refine-the-desktop-wave-view` at 15d2615160 (PR #1705)
as the separate **Loopflow Dev** app (`uv run python scripts/loopflow-dev.py run`).
Design: `scratch/loo-594-refine-the-desktop-wave-view.md`; decisions: `scratch/questions.md`.

## Observed (agent captures; Jack has not yet looked)

- `scratch/demo/after-wave.png` (Loopflow Dev, repository selected): the new layout is
  present — board left with the repository title, the repository Session pane right
  ("Not running here · Resume"), the footer strip below. The Dashboards sidebar section
  is gone. The board and strip are **empty**: planning fails with
  `serialization error: missing field 'interactive' at line 1 column 141`, shown in the
  sidebar and as `Refresh failed · …` / `Repository planning unavailable` on the board
  (the absent-state copy works as designed).
- `scratch/demo/before-wave.png` (installed release Loopflow): planning also failed
  there (`Workspace reader sent no observation for ten seconds`), so there is no usable
  "before" image of the old Wave view.

## Cause (verified in-session; not this branch)

`lf wave show product --json` with the branch's bundled lf fails the same way. The branch
lf runs on a preview copy (`~/.lf/copies/cfc2652ee52cd4e6`) because main carries
unreleased migration drafts. `GraphNode.interactive: bool`
(`rust/loopflow/src/graph/compiled.rs:35`) became required in e636ab6397 (#1652, "Ship a
Task with one click"); compiled graphs stored with earlier GraphProcess rows lack it, and
no draft migration backfills it (`drafts/ship_it.sql` does not mention it). This branch
does not touch `interactive`. Any main-built lf reading a store with pre-#1652 Graph
history hits it, so it will break planning for everyone at the next release unless fixed.

## Status (superseded below)

First attempt **not completed**: the populated board, people pictures, Done-days control, strip
paging and tile entry could not be observed on live data. No human feedback yet.

## Recommended next action

1. Fix the #1652 regression in its own Task (backfill `interactive` in the draft
   migration, or read old compiled graphs tolerantly), then rerun this demo on live data.
2. Or demo now against an `lf-world` world built by the branch lf (no pre-#1652 rows).

## Fix and second capture — October 26, 2026 (Jack Heart: "fix it here then keep demoing")

Fixed on this branch in 381df6246e: draft `drafts/graph_node_interactive.sql` renames
`"human"` to `"interactive"` in every recorded Graph step (XOR paths included), with
`graph_node_interactive_renames_recorded_human_steps` and fixture
`tests/fixtures/migrations/graph_node_interactive_released.sql`. All 796 recorded Graphs
in the live store's copy deserialize after the rename (one-off check, not committed).
The branch lf now works on a fresh preview copy `~/.lf/copies/a74234046a359c73`; the
stale `cfc2652ee52cd4e6` copy (10 GB) can be removed.

`scratch/demo/after-wave.png` (Loopflow Dev, Product Wave, 1440×900): board with
objective, chapter Project and both KRs; Done · 12, Active · 32, Backlog · 2; Wave
Session on the right; strip "Page 1 of 13" with three live tiles; no Dashboards section.

Agent observations (interpretation, each verified against `lf wave show product --json`):
1. **People shown while solo.** Initials appear on Done/Active rows. Jack is counted
   twice: owners recorded as `("Jack Heart", jack@loopflow.studio)` (39) and as
   `("Jack Heart", no email)` (16), so the ≥2-people rule fires.
2. **No GitHub pictures.** Rows show initials; GitHub identity (PR author, email→login)
   was not built (`docs` say "initials (no GitHub avatars yet)").
3. **Unstarted Tasks land in Active.** Active is `section ∈ {now, waiting}`; the roadmap
   puts 89 *unstarted* Tasks in `waiting`, so Active lists "Unowned · ready" rows and
   Backlog shows only 2.
4. **Strip leads with an abandoned Task** (LOO-299, Abandoned) and spans 13 pages: the
   strip includes terminal Tasks.

Jack has not yet looked at the running app; his observations go below.

## Jack's feedback during the demo — October 26, 2026

1. Jack Heart: "we need to make sure every pane has a hide button on hover that adds a
   Show... in the task header". Agreed change (interpretation): every pane in every view
   — Task (`session`, `shell`, `browser`) and Wave/repository (`board`, `session`,
   `tiles`) — shows a hide button when hovered. Hiding uses the existing hidden state
   (`MultiplexerStore.collapsedPaneIds` for workspace panes; the same `hide`/`show`
   verbs `lf desktop` uses, reported in `hidden_panes`/`view_parts`). The view's header
   (the Task header; the board header row on a Wave view) gains a `Show…` menu listing
   hidden panes; choosing one restores it. The menu is absent when nothing is hidden.

## More demo direction (steer, Oct 26)

Jack Heart, October 26, 2026 (LOO-594 demo), in addition to hover-hide panes with a Show… menu in the header:

1. "we want the section borders resizable": the dividers between the board and the Wave Session, and between the top band and the Task strip, drag to resize, like Task pane dividers (same ratio state that lf desktop resize writes; reported by lf desktop state).
2. "the task lists also need to auto paginate and scroll": board Task lists scroll, and load the next page automatically as you reach the end, replacing "+N more".
3. "I prefer turning it in to just showing one section but the section is selectable. Defaults to backlog": the board shows one section at a time, Done, Active or Backlog, picked by a selector in the board header, defaulting to Backlog. The Done-days control stays on Done. The selected section is reported in the board part state (replacing done_days-only state) and settable via lf desktop do.
4. "maybe we can switch the session and the tasks pane back": the Wave/repository Session goes back on the left, the board on the right (strip stays below).
5. "but add a zoom on every pane / so that panel can expand to whole page, and add sometjhing to breadcrumb / with an easy zoom otu": every pane (Task: session, shell, browser; Wave: board, session, tiles) has a hover zoom button beside hide. Zoom fills the whole view; the breadcrumb gains a segment naming the zoomed pane (e.g. "Product › Board"), and clicking the parent crumb, a zoom-out control, or Esc returns. Same zoom state as lf desktop zoom, reported by lf desktop state.
