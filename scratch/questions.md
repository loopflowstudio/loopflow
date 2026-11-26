# LOO-1114 kickoff: choices made with nobody here

- Titles re-read for every hosted terminal when `sessions`, `planning` or `graphs` moves (throttled to one sweep per 500 ms), not per terminal. Assumes a warm-statement read is cheap enough; pass 1 measures.
- A title can stay "Available" between an agent dying and its host recording the exit: process liveness has no wake. Is that acceptable, or should the reader also re-read on a slow clock?
- Viewer processes (cmux/tmux) keep their 500 ms check but read only revisions unless something moved. Worth giving them the kqueue wake too?
