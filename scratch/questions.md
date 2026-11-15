# LOO-515 open questions and assumptions

- Assumed (kickoff 2026-10-28): PR becomes part of a **Changes** page rather than its own page, and the GitHub patch list is deleted in favour of the local Diff plus `Open on GitHub ↗`. Reversible in review; Jack Heart's demo decides.
- Assumed: ⌥⌘1/2/3 for Brief/Changes/Files (no existing binding).
- Assumed: "relevant dependencies" = Task identifiers detected in the brief text; no Linear relations read in this Task.
- Assumed: page, scroll and tree expansion are retained in-app across Task switches, not across app restart (persisted intent belongs to LOO-497/416).

- 2026-10-28: Jack asked for three HTML prototypes before choosing; they are in `scratch/prototypes/` (A · Pages = this draft, B · Dossier, C · Navigator). The choice is pending and decides the layout for `pursue`.

- 2026-11-14: Jack requested `lf sync` and a review of T3 Code, Paseo, and Paradev, especially T3 Code. Sync completed without conflicts; research is in `scratch/task-details-research.md` and its HTML reading page. The earlier prototypes remain provisional. Research recommends revisiting the blanket removal of published PR diffs and exploring evidence-to-conversation context before choosing a layout.

- 2026-11-14: Jack selected PR-equivalent comparison as the Changes default (what `lf pr open` would show), named Kaleidoscope as his familiar diff viewer, and requested reusable repository/Wave filtering. Filtering scope and personal/shared defaults remain open. Jack wants the GitHub action to publish/open the current diff promptly and generate/update the description in the background; this is a related CLI outcome, not authorization to publish this research branch. See the Changes viewer section in `scratch/task-details-research.md`.

- 2026-11-14: Jack requested substantially heavier emphasis on polish and delight. The Changes prototype should demonstrate a complete polished review loop, including continuity, contextual feedback, live-update controls and revision-aware review progress. The increased quality bar is recorded in the design and research; exact layout remains in design review.

- 2026-11-14 clarification: Jack selected items 1/2/5/6/7 from the seven delight examples: cross-file next-change navigation, word-level highlighting, selection-to-conversation, live updates (“live, not pause”), and moved-code detection. This supersedes the broader prototype ordering; reviewed marks/pause are secondary, while basic continuity remains expected.

- 2026-11-14 exploration: Jack requested "$explore build me 3 prototypes". `scratch/explore.html` presents A Precision, B Review stream, C Change stories. All demonstrate the five selected priorities with explicitly simulated edits/PR effects. Reading mechanism and move presentation remain open; no vision is selected.

- Prototype feedback, November 14 — Jack: "probably mostly B review steream" and "maybe a little bit of A". B now offers side-by-side reading and temporary file focus as a concrete interpretation to try. Preference is tentative; the exact blend remains open.

- 2026-11-14 build: Jack: "yeah lets build it with (b)" — B review stream with A's side-by-side and file focus; moved code collapsed with expand. Recorded in the design under "Build direction — November 14".

- 2026-11-14 implement assumptions: next/previous change is `]`/`[` (Files pane focused) plus toolbar arrows; ⌥⌘arrows stay pane focus. **Ask in conversation** types a one-line `path:Lx-y (side, revision)` reference without the code, because terminal input can't carry a newline without submitting. Side by side falls back to unified under 640 pt. Moves are detected only between a pure removal and a pure addition in one file.

- 2026-11-14 tree assumptions: tree files open in Read (Diff is one picker away); badges are `M` changed, `R` renamed, `U` untracked, `•` folder holding changes, since `lf task changes` carries no added/deleted kind; ↓ from the search field moves into the tree; search stays over loaded entries plus changed paths (no server search).
