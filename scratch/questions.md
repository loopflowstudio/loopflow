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
  Before this change, a warm pane could send `CSI O` to an agent that asked
  for focus reports as soon as it attached.
- **The relay sends its last known focus on reconnect**, not "a person is
  here". A terminal that never reports focus still reconnects as focused.
- **`--unfocused` is an ordinary flag** on `lf session resume`, usable by a
  person who wants to watch without taking the size.
- **SSH initial focus belongs to the relay running beside the viewer's TTY.**
  Prepared remote panes use a local `lf session attach --` prefix. The
  remote command bridges the protocol, so a flag appended to the remote
  command does not configure the local relay. The local relay reads
  `LF_RELAY_FOCUSED` and sends the optional field on the wire.
- **`owner` keeps both meanings** (Session owner process, sizing view). The
  rename touches events live holders send and would be its own Task.
- **A focus report takes effect immediately**, including Ghostty's initial
  report when mode 1004 turns on. `--unfocused` describes the attach. The
  passive QA pane starts in the background.
- **Resume and New share the viewer option.** Both commands already use the
  same viewer for a fresh thread.
- **A prepared argv carries no focus.** `--prepare --unfocused` no longer
  writes the flag into `open_argv`; whoever runs the command says so with
  `LF_RELAY_FOCUSED=0`. Passed over: keeping the argv route for a future host.
  It has no caller, and its output fails on an `lf` from before the flag.
- Dev eligibility follows the retained workspace through repository and Task
  ID changes. The existing visit timestamp records a deliberate visit in Dev;
  loading a saved selection or board does not set it.
- **Desktop marks a background pane in its environment, not in argv.**
  Passed over: keeping `--unfocused` in Desktop's argv (an installed `lf`
  from before the flag rejects it, and Desktop has to parse `lf`'s grammar
  to place it), and asking the CLI its version first (a precondition). The
  earlier objections to a variable were that it doesn't cross `ssh` and
  reaches every child of a pane; initial focus turned out to belong to the
  local relay, and the holder drops the variable.
