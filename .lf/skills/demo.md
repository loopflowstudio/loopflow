---
extends: builtin
---
## Desktop demos

Build an isolated `.dev` bundle (`.lf/qa/desktop.md#loopflow-dev-on-the-live-machine`),
not the shared `~/Applications/Loopflow Dev.app` other worktrees overwrite, and
confirm its stamped revision matches `HEAD`. Launch it with `open -n` (never the
bare binary under `nohup`) so macOS brings the window forward, and verify
`LF_DESKTOP=<bundle-id> lf desktop state --json` lists a window before handing
it over. When the change works with the installed server, point it at `~/.lf`
instead of building `lf` and a fixture repo, noting any server change waiting on
branch `lf`.

## Disposable Loopflow demos

Use a private Machine when the demo needs a branch `lf`/server or destructive
state. Keep two fixed roots outside build output and `/tmp`: a history root
(`LF_HOME`, provider history, fixture repo) and a runtime root (copied runtime,
generated output, copied `lf` as `LF_BIN`). Record both roots, the Session,
teardown command, and how to reopen the history in the note.

At teardown, let live turns and pending requests settle, then `lf resume` each
demo Session and quit its provider. Remove the runtime root only once every
demo process has exited; otherwise keep it and record what remains.
