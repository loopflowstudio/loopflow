Choices made for review (recorded in `scratch/plan.md` and `scratch/concept-review.html`):

1. Pause applies to open, started Tasks only (refuses `unstarted` Backlog and terminal Tasks); `unassign`, `complete`, `abandon`, and `reopen` clear the pause hold.
2. Explicit `lf task unpause` (with `--run` shortcut) lifts the hold; `lf task run`, `lf task auto on`, `lf resume --task`, recovery, and steer wakes respect the hold while paused.
3. Pause closes the primary Session via `Departure::Paused` (retaining thread and history) and lets the server bring it back on unpause unless the person had already quit it beforehand.
4. `TaskSection::Paused` sits outside `Working` and `Backlog`, wired into `TaskList.paused` on iPhone and a discoverable Paused view in Mac's sidebar and Wave board.
5. Descendant-aware worker teardown is shared by `pause`, `interrupt`, and `move`.

## Implementation choice

Prove worker teardown through `lf task interrupt` first. Use the worker’s OS session to find builds whose launching shell already exited.
