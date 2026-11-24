# Choices & open threads

## Decided for this pass (B1)

- Jack Heart: “ok i think  i like b.  i think we could do something liek what rex does with a button that sorts by terminal status, or alternatively we could sort by what step theyre at” / “lets add b1” / “but also show the current step in the leftp ane, just not sort by it now”
- Default Working Task order is **Start-Time Stack**: newest-started first (`started_at` desc, canonical Task ID tiebreaker), decoupled from backlog `planning_rank`. New arrivals not yet in `task_orders` prepend at the top ahead of saved rows.
- Jack Heart (during `/demo`): “whqt is that arrows button” / “well, it needs a on/off state, and maybe an on-hover” / “1a” / “build it and return when ready”
- **Sort by status** is now a non-destructive **on/off toggle** in the sidebar header (with active highlight + hover fill/tooltip) and `⌘K`: **On** sorts by `Blocked → Error → Available → Idle → Busy` (tie-broken by stack/drag order, latching the currently selected Task in place until you switch away, drag disabled while On); **Off** restores the Start-Time Stack / saved `task_orders` drags. The toggle lives in the repository window and starts Off on relaunch.
- Show each Task's current Graph step in the left bar's row metadata (display only; do not sort by step in this pass).
- Keep PR #2025's immediate drag feedback, side-aware `DropSide` drops, and refusal recovery.

- Turning sorting on with a Task already open holds its pre-toggle slot. A selection made while sorting holds the slot that was clicked. Switching filters starts a new slot in the visible list.
- Drags for one Working list reach the Machine in click order across retries. Each drag previews immediately. Saving the drop uses the same ordered Task membership as the views and can finish while Session activity is unavailable.
- Start time uses the first recorded claim, then the first recorded start event, then creation time. Later handoffs keep the Task’s original start time.
- The status grouping uses the shared activity flags. A Task with no Blocked, Error, Busy or Available flag occupies the Idle group. Busy wins over Available when both are present.
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

## From QA round 4 (2026-11-23)

- A returning Desktop launch opened by link stays on saved planning (“Updating…”), highlights the saved selection and ignores sidebar clicks; moving `desktop-cache/workspace.json` aside clears it. Left unfixed here: the stuck read is in workspace restore, not Working order. Worth its own Task if it reproduces unlocked.

## From sync onto main (2026-11-23)

- `server_operations_match_their_lf_commands` fails after the merge at `config scope --json`: the server reports the personal config path as `/private/var/folders/…`, the CLI as `/var/folders/…`. The `config scope` read and that test arrived from main; this branch doesn't touch config code. Left for gate/CI to confirm whether main fails the same way.
