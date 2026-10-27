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

6. People, Jack Heart Oct 26 ("3 people: (a)"): fix the solo count (one person per email, name-only owners join the person with that name) and build GitHub pictures and names in this PR, per scratch/questions.md #4.
7. Strip, Jack Heart Oct 26 ("(b)"): the strip shows only Tasks being worked on (a live Session or Graph): no unstarted, finished or abandoned tiles. Unstarted Tasks live in the board Backlog.
8. Active vs Backlog (agent default, Jack did not object; reversible): bucket by Task state, not roadmap section. Active = started and unfinished; Backlog = not started (no checkout), whatever roadmap section it sits in (the roadmap files unstarted Tasks under waiting).

9. Jack Heart Oct 26: "I think we need actaully a fourth seciton which is like Autonomous Tasks that are still working" and "somewhere earlier i had a 2x2 grid about defining Active and Available". Section definitions OPEN until the grid is found or redrawn; do not change bucketing in the next pass.

10. Found the grid (LOO-561 design, commit 427d91bfe8, Oct 24; now AGENTS.md "Task state … Session state (available · busy) is a separate axis"). Jack Heart, 2026-10-24: "so its Working vs Active and Available vs Busy as two different axes". Task axis today: unstarted · idle · active (at an interactive stage) · moving (Graph running, stage ahead) · shipping (Graph running to merge) · deferred · done · abandoned. Session axis: available · busy. Proposed board sections (awaiting Jack): Backlog = unstarted (+deferred?); Active = active (+idle?); Moving = moving + shipping (the fourth section, "autonomous Tasks still working"); Done = done. Session availability is a badge on the row, not a section.
11. Jack Heart Oct 26: "Yeah maybe Working = Active + Idle + Moving" (the existing Working Tasks rule). Agent reading, to confirm: sections Backlog (unstarted) · Working (idle + active + moving) · Shipping (shipping: autonomous, running to merge without stopping for chat; the fourth section) · Done. Session available/busy as a row marker.
12. Board sections decided (Jack Heart Oct 26: "Working = Active + Idle + Moving", "I think Autonomous might be better than Shipping"): Backlog = unstarted (+ deferred, agent default) · Working = idle + active + moving (same rule as the sidebar Working Tasks) · Autonomous = shipping (a Graph running to merge that will not stop for chat) · Done = done. Bucket by Task state (runtime.status), not roadmap section. Each Working/Autonomous row shows its Session as Available or Busy. The section selector offers these four; default Backlog.

## Third capture — October 26, 2026, branch 68419e094d (PR #1705, CLEAN)

`scratch/demo/after-wave-2.png` (Loopflow Dev, Product Wave): Session left, board right,
strip below. Board section selector `Backlog · 35 | Working · 3 | Autonomous · 0 | Done · 13`,
Backlog selected by default, one scrolling list. Strip: LOO-594, LOO-609, LOO-628 (the
three Working Tasks), one page. No initials (solo count fixed).

Agent observations (verified):
1. **Canceled Task in Backlog.** LOO-299 (Linear state `canceled`, title "[superseded by
   LOO-303 — cancel]") reads runtime `unstarted`, so the state-based bucketing puts it in
   Backlog. The earlier strip tile labelled it Abandoned, so Desktop has two answers for
   one Task. Backlog should exclude canceled planning state (or the Task state should read
   abandoned) — one rule.
2. **Render artifact.** A lighter rectangle spans the top of the window over the board's
   left edge (roughly x 980–1360 of 2000, top to y≈710) in the capture. Unknown cause;
   check in the live window.
3. Strip membership question still open (questions #17): Working only (built) vs
   Working + Autonomous.

## Jack's live walk-through — October 26, 2026 (branch 68419e094d)

- First open of the repository page (Jack's screenshot): "Couldn't update: Workspace
  reader sent no observation for ten seconds"; empty sidebar and strip, board from the
  last good read. Cause (measured): on the freshly made preview copy, `lf monitor work
  --watch --json` sends heartbeats (`answers: null`) every 2 s and its first answers
  (6.2 MB) after ~16 s, past Desktop's 10 s reader limit
  (`LocalLineObservation.swift:207`). The installed release hit the same limit earlier
  today on the shared store, so it is not specific to this branch.
- Second screenshot, after recovery: repository page works — sidebar 19 Working Tasks,
  board `Backlog · 106 | Working · 19 | Autonomous · 0 | Done · 58`, strip "Page 1 of 7"
  with live tiles (… menu and expand). Backlog still lists canceled/merged Tasks
  (LOO-299 "cancel", LOO-318 "Merged into LOO-309"), confirming the canceled-state gap.
- Third screenshot (Jack, 22:55): repository page with "Repository planning unavailable"
  and the same reader timeout. At that moment the Mac's load average was 122 (several
  `rustc`/`clippy-driver` builds of `loopflow` from other Task worktrees, e.g. LOO-630,
  LOO-609); the feed's own startup ran past the 10 s limit. Every line, heartbeats
  included, resets the limit (`LocalLineObservation.swift` `lastFrame`), so the stall is
  before the first heartbeat, under load. Not caused by this branch.
- **Paint bug (this branch, seen in both the agent capture and Jack's screenshot):** the
  board part's dark background covers only its left portion (to about x 1390 of 2000);
  the rest of the board pane shows the window background. Needs a fix: the board view
  should fill its part's frame.

## Canceled leftovers fixed in the store (October 27)

Jack Heart: no code for state users can't reach; edit the local store. Sixteen Tasks were `canceled` in Linear with no checkout and no `abandoned_at` (LOO-299, LOO-307/308/310, LOO-313/314/315, LOO-318, LOO-395, LOO-410, LOO-430/431/432, MTG-82, MTG-116, MTG-120). Set `abandoned_at` and `abandon_reason='canceled in Linear'` directly in `~/.lf/loopflow.db`; `lf task status LOO-299` now reads `abandoned`. Original rows saved as INSERTs in the session scratchpad (`canceled-tasks-backup.sql`).
