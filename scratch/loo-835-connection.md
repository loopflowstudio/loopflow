# LOO-835: OpenCode and Pi keep one server connection

Docs written for this change (read these first; they are the intended behavior):

- `rust/loopflow/src/server/README.md`, "OpenCode and Pi keep one connection"
  (wire example, Pi `moment: "event"`, OpenCode `tool` per request, gaps and reconnect)
- `rust/loopflow/src/harness/README.md`, "lf never breaks the conversation"
- `docs/troubleshooting.md`, "A steer arrived late, or a turn is missing from history"
- `docs/dev/server-first.md`, Pieces table row

Choices the docs take (reversible, headless):

- Steers: the plugin asks at each boundary over the open connection; no
  subscription. Pi asks with every event it reports (message end, tool
  start/end, UI prompt, `working` every 5 s while streaming), so a steer
  waits at most for a tool to end, as on Claude.
- Pi events go to the server as `session.boundary` with a new
  `BoundaryMoment::Event`, callback = the Pi event; server records it as
  `pi::native_event` did and answers steers. Replaces `--moment pi-event` and
  the 1 s `setInterval`.
- OpenCode: block rendered by `lf __context-block` at start and after
  compaction (fixes `compacted` never resetting); every request sends `tool`
  over the connection for steers. Turn end `stop` over the connection.
- Gap and `printed` batches kept in plugin memory, sent with the next
  answered request; no `.store-gap` / `.printed` files for these plugins.
- hello `client.kind` is `"plugin"`; `ClientIdentity` grants nothing.
- Unchanged: Pi bash guard (`--moment tool-group`, one lf per bash tool),
  Pi Task-update watcher (one lf child per conversation).
