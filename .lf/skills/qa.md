---
extends: builtin
---
## Desktop

Check whether the display is locked first
([display lock preflight](../qa/desktop.md#display-lock-preflight)): when
locked, skip interactive GUI/WindowServer clicks and use the headless checks
there instead of retrying.

Use the [isolated Dev bundle](../qa/desktop.md#loopflow-dev-on-the-live-machine)
and disposable Work. Check whether launch can attach to or resize personal
terminals before opening the live Machine. For visual acceptance, capture the
changed UI and a real update in the foreground, and click through it watching
the rest of the window: choosing or hovering an item never moves or resizes
its neighbors. Report the check as open if
it cannot run safely.

For Session attachment, join an already-running disposable Session on its
owning Machine from a second view. Check shared output and input, then close
and reopen the second view while the original provider keeps running.

When Desktop must work with the installed server, exercise the newer client
with older responses, including an extra field and an unsupported operation.
Verify that the failure stays local and supported reads and actions still work.
Keep the installed server and schema for the live run.

## CLI and server

For CLI and server changes, run the branch `lf` on a private Machine following
[served launches](../qa/served-launches.md) and [server QA](../qa/server.md).
