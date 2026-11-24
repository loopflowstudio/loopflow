---
extends: builtin
---
## Desktop demos

Build an isolated `.dev` bundle (`.lf/qa/desktop.md#loopflow-dev-on-the-live-machine`),
not the shared `~/Applications/Loopflow Dev.app` that other worktrees overwrite,
and confirm its stamped revision matches `HEAD` before launching or relaunching.
When the change works with the installed server, point it at `~/.lf` instead of
building `lf` and a fixture repo, noting any server change waiting on branch `lf`.

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
