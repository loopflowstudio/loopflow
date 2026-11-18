# Verify LOO-932 on main (b0d867620d)

Post-merge CI for b0d867620d2c1d08becc080f3ab6106aae020e6e:
- push `CI` run 44218359722: success
- push `macOS on main` run 44218359673: success
- workflow_run `Packages (main)` 44218501493: success
- (merge queue CI 44217124220: success, reported separately)

Headless, main's debug `lf` in `lf-world build one-task /tmp/v932`, private `lf server run`, `qa` Graph (`cmd: help task`, `interactive: demo`):
- `lf task create '…' --run --json </dev/null`: exit 0 in 1.23 s including post-answer sync; `milestone: admitted`, answer carries `task`, `session_id`, `session`.
- `lf task status`: worktree `repo.13810d-verify-start-opens`, Graph qa at demo, Session open; `repo.ready` replacement present afterwards.
- `start_timing` spans keyed by request ID: creation steps (file_task 76 ms, hold_conversation 173 ms) then setup checkout 391 ms, graph 144 ms, sync_planning 817 ms after the answer.
- Repeat with `--request-id` of the first: same Task and Session, `admitted`, still one graph log.

Not checked here: Desktop and phone walkthroughs, remote-Machine journey (need a display/second Machine).
