# Open choices, made for review

- **No second-class viewer.** The brief says "Consider an optional passive /
  second-class viewer flag". The plan has one rule instead: an attach says
  whether a person is at it (`focused`), and focus or input takes the size
  from anyone. A Dev pane someone types into resizes the Session.
- **Release Desktop keeps warm conversations**; only Dev stops opening
  unvisited Work. Removing warmth everywhere would also drop saved-tab restore
  and worktree retention by open views.
- **Dev's restored selection waits for a click.** At launch Dev shows the
  placeholder for the selected Work and starts no terminal until a deliberate
  visit.
- **A relay whose stdin is not a terminal attaches unfocused.** A piped or
  headless `lf session resume` no longer takes the size.
- **Old holders stay exposed to release Desktop's warm attaches** until their
  Sessions restart; nothing migrates or restarts them.
- **Focus-out from a view that isn't in use is dropped** before the agent.
  Today any view's `CSI O` reaches an agent that asked for focus reports; a
  warm pane sends one the moment it attaches.
- **The relay sends its last known focus on reconnect**, not "a person is
  here". A terminal that never reports focus still reconnects as focused.
- **`--unfocused` is an ordinary flag** on `lf session resume`, usable by a
  person who wants to watch without taking the size.
- **SSH initial focus belongs to the relay running beside the viewer's TTY.**
  Prepared remote panes use a local `lf session attach --` prefix. The
  remote command bridges the protocol, so a flag appended to the remote
  command does not configure the local relay. Use the local prefix for
  initial focus and keep the optional field on the wire.
- **`owner` keeps both meanings** (Session owner process, sizing view). The
  rename touches events live holders send and would be its own Task.
- **A focus report takes effect immediately**, including Ghostty's initial
  report when mode 1004 turns on. `--unfocused` describes the attach. The
  passive QA pane starts in the background.
- **Resume and New share the viewer option.** Both commands already use the
  same viewer for a fresh thread. On either command, `--prepare --unfocused`
  returns an argv that opens with the requested focus.
