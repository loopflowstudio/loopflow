# Open questions

- Assumption: non-tmux terminal sessions are not attachable. `POST /v0/terminal-sessions/{id}/attach` now returns `412 Precondition Failed` for non-tmux-backed sessions instead of fabricating a launch command or connection payload.
