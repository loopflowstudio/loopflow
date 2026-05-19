path: demo

Two observable changes ship on this branch:

1. **Native chat rendering** — assistant messages in Concerto now render as styled markdown blocks (headings in burgundy, bulleted lists, bold/italic, syntax-colored code, diff views). The before/after is immediately visible to anyone opening a wave session.

2. **lfd-owned terminal sessions** — palette launches create an lfd terminal session and bind its id into the focused pane. Flow output persists across daemon/app restarts; the session header reflects the real lfd session lifecycle.

These are experience changes, not internal refactors. `demo` is the right lens.
