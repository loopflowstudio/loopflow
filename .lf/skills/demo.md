---
extends: builtin
---
## Desktop on the live Machine

For Loopflow Desktop changes that work with the installed server, demo against
live data instead of building `lf` and a fixture repo. Run
`uv run python scripts/loopflow-dev.py run` (or an isolated `.dev` bundle
against `~/.lf` when `Loopflow Dev` is already running), and note any
server-side change that waits for the branch `lf`.

## Disposable Loopflow demos

Use a private Machine when the demo needs a branch `lf`/server or destructive
state. Create two fixed roots outside build output and `/tmp`: a history root
(`LF_HOME`, provider history, fixture repo) and a runtime root (copied runtime,
generated output, copied `lf` as `LF_BIN`). Record both roots, the Session, each
process's exact identity, the teardown command, and how to reopen the history in
the note; keep both roots while those processes live.

At teardown, wait for live turns and pending requests to settle, then
`lf resume` each demo Session and quit its provider (signal only its verified
PID if it won't exit). Confirm every demo process exited before removing the
runtime root; if any survive, keep it and record what remains.
