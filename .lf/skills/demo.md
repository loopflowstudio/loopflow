---
extends: builtin
---
## Disposable Loopflow demos

Before launch, create two private roots outside build output and temporary
directories, which get purged; use any location the repo's cleanup guidance
names. The history root holds the private Machine, provider history and fixture
repository. The runtime root holds the whole copied runtime (the full app bundle
when there is one) and generated output. Fix absolute paths from the start and
never relocate them. Use the history root as `LF_HOME` and the copied CLI's
absolute path as `LF_BIN` for launch, fixture commands and teardown. Record both
roots, the Session, each demo process's exact identity, the teardown command and
how to reopen the private Machine's history in the note. Keep both roots through
rebuilds while those processes live.

When the demo ends, wait for live turns and pending requests to settle. Then
`lf resume` each demo Session and quit its provider, signaling only its verified
PID if it won't exit. Quitting discards unsent input; saved history cannot
recreate the editor.

Confirm the demo processes exited before removing the runtime root. Never delete
the history root. If teardown is incomplete, keep the runtime root and record
what survives and the next action. No other cleaner, no signaling by process name.
