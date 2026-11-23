# Choices & open threads

## Decided for this pass (B1)

- Jack Heart: “ok i think  i like b.  i think we could do something liek what rex does with a button that sorts by terminal status, or alternatively we could sort by what step theyre at” / “lets add b1” / “but also show the current step in the leftp ane, just not sort by it now”
- Default Working Task order is **Start-Time Stack**: newest-started first (`started_at` desc, canonical Task ID tiebreaker), decoupled from backlog `planning_rank`. New arrivals not yet in `task_orders` prepend at the top ahead of saved rows.
- One-shot **Sort by status** button in the sidebar header (and shared Machine command) snapshots `Blocked → Error → Available → Idle → Busy` (newest-started first within each bucket) into `task_orders`, then leaves order static until the next drag, sort, or new arrival.
- Show each Task's current Graph step in the left bar's row metadata (display only; do not sort by step in this pass).
- Keep PR #2025's immediate drag feedback, side-aware `DropSide` drops, and refusal recovery.

- A drag and a status sort for the same Working list reach the Machine in click order, including retries. Each drag still previews immediately.
- Start time uses the first recorded claim, then the first recorded start event, then creation time. Later handoffs keep the Task’s original start time.
- The status snapshot uses the shared activity flags. A Task with no Blocked, Error, Busy or Available flag occupies the Idle group. Busy wins over Available when both are present.
- The sidebar uses the Task’s saved Graph cursor while a run carries it, the same position used to resume that Task. A standalone Graph process in the checkout does not move the Task.
- The terminal's own completed program report reads “Finished”. On a Task row a live finished turn reads “Available” when the Machine reports availability.
- Swift uses the selection enum’s declaration order when patching rows. That also keeps newly added activity flags in pending edits.
- A missing bundled face falls back at the same type size, with a system serif for the New Task heading.

- Font registration takes its inventory from the packaged TrueType files,
  with the Fonts directory taking precedence over the bundle root.

- A pinned selection read shares Task details across all its plans. Retained tiles without current Work keep their unavailable explanation.

## Open threads (not in B1)

- Sorting by Graph step (`B2` in `explore.html`).
- Smart “heading to the end” sink for `Auto` / `deliver` / settling PRs (`B3` in `explore.html`) — Jack Heart: “i think we need something like moving auto to the bottom, or being able to 'close' certain tasks, but auto has to be smart enough to know like if its moving towards the end, that coutn even if its not explicitly auto”.
- Manual “Close / Park” for individual Working Tasks (`B4` in `explore.html`).
- LOO-937's Project-rank redesign remains separate from Working sidebar order.
