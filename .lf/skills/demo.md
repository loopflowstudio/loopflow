---
extends: builtin
---
## Desktop demos

Build an isolated `.dev` bundle (`.lf/qa/desktop.md#loopflow-dev-on-the-live-machine`),
not the shared `~/Applications/Loopflow Dev.app` other worktrees overwrite, and
confirm its stamped revision matches `HEAD` before launching. Launch it with
`open -n` (never the bare binary under `nohup`) so macOS brings the window
forward, and verify `LF_DESKTOP=<bundle-id> lf desktop state --json` lists a
window before handing it over. When the change works with the installed server,
point it at `~/.lf` instead of building `lf`, noting any server change waiting
on branch `lf`.

## Disposable Loopflow demos

Use a private Machine for a branch `lf`/server or destructive state, with two
fixed roots outside build output and `/tmp`: a history root (`LF_HOME`, provider
history, and a disposable clone of this repository instead of a toy fixture) and
a runtime root (copied runtime, output, copied `lf` as `LF_BIN`). Set
`default_agent` in that `LF_HOME` and verify the window opens without errors
before handing it over; fix setup errors and relaunch yourself instead of asking
someone to click Retry. Record both roots, the Session, each process's identity,
and teardown in the note.

At teardown, let live turns settle, `lf resume` each demo Session and quit its
provider (signal only its verified PID if needed). Remove the runtime root only
once every demo process has exited; otherwise keep it and record what remains.
