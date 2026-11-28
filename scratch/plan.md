# LOO-1126: links in agent Sessions open from Loopflow Desktop

Brief, word for word: "links in agent sessions that work in terminals like rex
dont work in loopflow desktop".

## What is wrong

Two separate faults. Either alone leaves a link dead.

**1. Desktop drops every OSC 8 click.** An agent prints a link as an OSC 8
hyperlink (text with a hidden target: a Markdown `[PR](https://…)`, a file
path). ⌘-click reaches Ghostty, which asks the embedder to open it with
`GHOSTTY_ACTION_OPEN_URL`. Desktop's `action_cb`
(`swift/LoopflowMac/Services/Ghostty/GhosttyManager.swift:271`) has no case for
it and returns false. At the pinned revision (a60e9e2) Ghostty then refuses on
macOS: `src/os/open.zig` returns `error.UnsafeOSC8Link` for `kind == .osc8`
("If a macOS embedder declines the action, fail closed"). The click is
swallowed and a warning is logged. Plain URLs Ghostty finds by regex still open
through its `open` fallback, which is why this reads as "some links".

Desktop also ignores `GHOSTTY_ACTION_MOUSE_SHAPE` and
`GHOSTTY_ACTION_MOUSE_OVER_LINK`: no pointing-hand cursor and no target preview
on ⌘-hover, so nothing says a link is there.

**2. Many Sessions never print OSC 8.** Claude Code 2.1.332 decides from the
environment (read from its binary): `FORCE_HYPERLINK`, else `TERM_PROGRAM` in
`ghostty, Hyper, kitty, alacritty, iTerm.app, iTerm2, rex, WarpTerminal`, plus
a few others. A held terminal's environment is the caller's
(`transport.rs` `start_held`), and `registry.rs:548` fills in only `TERM` and
`COLORTERM`. Desktop strips `TERM_PROGRAM` from its own environment
(`ProcessEnvironment.swift:68`), and the server has none. Live count on this
Machine, agent processes inside a held terminal:

| `TERM_PROGRAM` | processes | started from |
| --- | --- | --- |
| unset | 29 | the server: Graph workers, Desktop actions |
| `ghostty` | 7 | a Desktop pane (embedded Ghostty sets it) |
| `rex` | 2 | a Rex pane |

The 29 print link text with no target, in every terminal. Started from Rex they
carry `rex`, which is the "works in rex" half.

## Outcome

In a Desktop Session or shell pane, ⌘-hover over a link shows the pointing hand
and the target; ⌘-click opens it. Web and mail links go to the default app,
existing local files and folders open with theirs. A Session the server starts
prints the same clickable links as one started from a terminal.

## Design

### Pass 1: Desktop opens links (this one could prove the plan wrong)

`TerminalLink` in `swift/LoopflowMac/Services/Ghostty/TerminalLink.swift`: a
value built from the action's `url` and `kind`, with one `decision`:

- `http`, `https` with a host; `mailto` with an address: open.
- `file`: local host only, no query or fragment after an optional `#L<n>` is
  dropped; resolve symlinks; must exist as a regular file or directory; refuse
  what Launch Services would run (app bundles, scripts, executables, by
  extension, content type and the executable bit). Open with
  `NSWorkspace.shared.open`.
- any other scheme: an alert naming the target and asking before dispatch.
- malformed, relative, or carrying control/bidi characters: refuse, with a
  one-line reason in the pane's existing notice.

The policy is Loopflow's own code, written against the same threat Ghostty's
`macos/Sources/Helpers/UntrustedURL.swift` names: the target is whatever the
program printed. `kind == .osc8` always takes this path and the callback always
returns true for it, so the unrestricted fallback can never run. Regex-matched
links (`.unknown`, `.text`, `.html`) take the same `decision` after a
scheme-less value becomes a file path; one opener, no second policy.

`action_cb` gains three cases: `OPEN_URL` (copy the bytes inside the callback,
decide and open on the main actor), `MOUSE_SHAPE` (set the view's `NSCursor`
through `resetCursorRects`/`cursorUpdate`), `MOUSE_OVER_LINK` (a target preview
in the pane's bottom corner, cleared on the empty URL and on `mouseExited`).

Shows: in the Dev app, an agent Session's Markdown link turns the cursor to a
hand under ⌘ and opens the browser on click. If the action never arrives for a
relayed Session, the fault is in the relay and pass 2 changes shape.

Tests: `TerminalLinkTests` (headless) over the decision table, including
`file:///tmp/x.command`, a symlink to an app, `https:relative`, a bidi override,
`#L12`. One `.requiresDisplay` case in `TerminalFeedbackTests`: feed
`ESC ] 8 ;; https://example.com ESC \ text` to a surface, ⌘-click the cell,
assert the opener closure received the URL.

### Pass 2: server-started Sessions print links

`registry.rs:548` already says "A GUI or service launch may have no terminal
environment of its own" and defaults `TERM` and `COLORTERM`. Add
`TERM_PROGRAM=ghostty` to that list, only when absent or empty. The program's
terminal is lf's held libghostty-vt terminal, which keeps OSC 8 through repaint
and snapshot (`ghostty-vt-patches/0001-vt-hyperlinks.patch`), so the name is
what the program is talking to. An inherited `rex` or `ghostty` stays.

Made for review. Passed over:
- `FORCE_HYPERLINK=1`: the agent's children inherit it, and
  `supports-hyperlinks` honors it with no TTY, so tool output piped back to the
  model would fill with escape sequences. `TERM_PROGRAM` is checked only on a
  TTY.
- a `loopflow` name: no program recognizes it, so nothing changes.
- setting it from Desktop per pane: the 29 are started by the server, not a
  pane.

Test: extend the env case in `transport/registry_tests.rs:157` (empty, set,
partial) with `TERM_PROGRAM`.

Codex: its link emission was not probed. The default applies to it too; QA
notes what a Codex Session prints.

## Acceptance check

Headless: `cargo test -p loopflow-execution --lib transport::registry_tests::`
and the `TerminalLinkTests` suite pass.

Demo (Dev app): start a Task from Desktop so the server launches its Session,
ask the agent for `[the PR](https://github.com/…)` and a file path. ⌘-hover
shows the hand and the target; ⌘-click opens the browser and the file.

## Absent and error states

- Target refused: nothing opens, the pane says why, the callback still returns
  true.
- File gone or on another Machine (a remote Session's path): refused as
  missing. No attempt to fetch it.
- Sessions already running keep their environment; links appear on the next
  launch.

## Security and privacy

Link targets are untrusted input: any program output, including text an agent
relays from the web, can carry one. The rule that must hold: no OSC 8 target
reaches Launch Services without the decision above, and no executable local
file opens from a click. `lf security-review` runs before delivery.

## Exclusions

- iPhone (`swift/LoopflowiOS/GhosttySurface.swift`): the same action is
  unhandled and Ghostty's opener is unimplemented on iOS, but a tap carries no
  ⌘, so it needs a touch gesture designed first. Not here.
- Opening a checkout file in Desktop's Files pane at its line instead of the
  default app. Parity with Rex first.
- Changing Ghostty's link modifiers or regexes.

## What goes

Nothing is replaced. No GhosttyKit patch or rebuild: every action used is in
the pinned header.

## Done when

⌘-click on an agent's link in a Desktop Session opens it, the cursor and
preview announce it, and a Task started from Desktop prints clickable links
without having been opened from a terminal first.
