# LOO-1126: links in agent Sessions open from Loopflow Desktop

Brief, word for word: "links in agent sessions that work in terminals like rex
dont work in loopflow desktop".

Base 6deb3b0450.

## What is wrong

Two separate faults. Either one is enough to leave a link dead.

**1. Desktop drops every OSC 8 click.** ⌘-click on an OSC 8 hyperlink reaches
Ghostty, which asks the embedder through `GHOSTTY_ACTION_OPEN_URL`. Desktop's
`action_cb` (`swift/LoopflowMac/Services/Ghostty/GhosttyManager.swift:271`)
has no case for it and returns false. At the pinned revision (a60e9e2)
Ghostty's `src/os/open.zig` then returns `error.UnsafeOSC8Link` for
`kind == .osc8` on macOS, so the click is swallowed. Plain URLs found by regex
still open through Ghostty's `open` fallback, which is why the bug looks like
"some links". `MOUSE_SHAPE` and `MOUSE_OVER_LINK` are ignored too, so there is
no hand cursor and no target preview.

**2. Many Sessions never print OSC 8.** Claude Code 2.1.332 checks
`FORCE_HYPERLINK`, else `TERM_PROGRAM` in `ghostty, Hyper, kitty, alacritty,
iTerm.app, iTerm2, rex, WarpTerminal` (read from its binary). A held terminal
inherits the caller's environment (`transport.rs` `start_held`).
`transport/registry.rs:548` defaults only `TERM` and `COLORTERM`. Desktop
strips `TERM_PROGRAM` (`ProcessEnvironment.swift:68`) and the server has none.
Live count on this Machine, agent processes inside a held terminal:

| `TERM_PROGRAM` | processes | started from |
| --- | --- | --- |
| unset | 29 | the server: Graph workers, Desktop actions |
| `ghostty` | 7 | a Desktop pane |
| `rex` | 2 | a Rex pane |

lf prints its own Work links as OSC 8 too (`work::link::osc8`, used at
`rust/loopflow/src/lf/commands/open.rs:1490`), so `loopflow://` links show up
in Sessions from the start.

## Outcome

In a Desktop Session or shell pane, ⌘-hover over a link shows the hand cursor
and the target; ⌘-click opens it in the right place. Web and mail links open in
the default app. A Loopflow Work link opens in this Desktop window. A file in
the Task's active checkout opens in the Files pane at its line, on this Mac or
a remote Machine. Any Session prints clickable links, whichever terminal
started it.

## Design decisions made for review

Two headless concept reviews asked these questions; nobody answered them.
Each recommended direction is adopted and marked for `ship-review` to raise.
Numbers refer to the second review (`scratch/concept-review.html`, after
Pass 1).

- **Checkout files (Q2 → 2a).** A file inside the Task's active checkout opens
  in the Files pane at `#L<n>`. Passed over: the default app for every file,
  as Rex does. That drops the line, has to refuse repo scripts like
  `scripts/test.py`, and cannot open a file on a remote Machine.
- **Panes with no matching Files pane (Q1 → 1a).** Repository and Wave
  Sessions, and a shell left on this Mac after its Task moved, open safe local
  documents and folders in the default app; scripts and executables stay
  refused. Passed over: refusing every checkout link there, which is what
  Pass 1 does today ("Checkout links are unavailable in this pane.").
- **Remote hostnames (Q3 → 3a).** On a remote pane, a `file://<host>/…` path
  inside the remote Task's active checkout opens in Files whatever the host,
  since `ls --hyperlink` and `rg` print the remote OS hostname and Files reads
  through the server, never that host. Local panes keep requiring this Mac's
  hostname. Passed over: only empty or `localhost` hosts remotely, which
  leaves those tools' links dead.
- **Held terminal identity (Q4 → 4a).** Every held terminal sets
  `TERM_PROGRAM=ghostty` and drops `TERM_PROGRAM_VERSION`. The program always
  talks to libghostty-vt. Passed over: setting it only when unset, which
  leaves Apple Terminal and tmux starters without links. Sessions started
  from Rex lose `rex`; `terminal_host.rs` reads `REX_SESSION`/`REX_BLOCK`.
- **Work links (Q5 → 5a).** A `loopflow://` URL in `WorkUrl`'s grammar opens
  in this window with no confirmation. Passed over: treating it as an unknown
  scheme, which warns on lf's own links and can wake the installed release app.
- **FORCE_HYPERLINK** stays rejected: children inherit it, and
  `supports-hyperlinks` honors it with no TTY, so piped output fills with
  escapes.

## Built

- Pass 1 (`a12082957b`, hover fix `0552209e91`): Ghostty delivers OSC 8 clicks and ⌘-hover feedback. Desktop opens web, mail and safe local documents and confirms external schemes.
- Pass 2 (`e50723744`): Work links resolve in the repository window, checkout links reveal Files at the line, Dashboard tiles show the Files pane, and link failures appear in the originating terminal.
- Pass 3 (`04c2f98af7`): New held terminals report `TERM_PROGRAM=ghostty` and remove the caller's `TERM_PROGRAM_VERSION`.

## Acceptance check

Pass 1 passed the native paused-Session OSC 8 fixture. Pass 2 passed
`swift test --package-path swift --filter 'TerminalLinkTests|TerminalFeedbackTests'`
with the two display tests skipped, and both
`cargo test -p loopflow-execution --lib ops::task::directory::tests::` tests passed.
The signed Xcode build and live local and remote agent demos remain for CI and QA.

Pass 3 passed `cargo test -p loopflow-execution --lib held_children_get_ghostty_identity_and_default_terminal_colors`.
The PTY child saw `ghostty` and no version with server, Rex, and Apple
Terminal inputs. QA still needs to check what a live Codex Session prints.

Demo (Dev app): start a Task from Desktop so the server launches its Session.
Ask the agent for `[the PR](https://github.com/…)`, a checkout file at a line,
and `lf open --task <ID> --link`. ⌘-hover shows the hand and the target.
⌘-click opens the browser, the Files pane at the line, and the Task.

## Absent and error states

- Refused target: nothing opens, the overlay says why, and the callback still
  returns true.
- Missing local file: refused as missing.
- Remote file outside the checkout, or any file from a moved Task's old
  remote shell: refused as on another Machine.
- Unknown Task in a Work link: the overlay says so; no open-work window.
- Running Sessions keep their environment. Links appear from the next launch.

## Security and privacy

Link targets are untrusted: any program output, including web text an agent
passes on, can carry one. These must hold:

- No OSC 8 target reaches Launch Services without the classification.
- No executable local file opens from a click.
- Terminal output cannot reach app-control `loopflow://` routes.

`lf security-review` runs before delivery.

## Exclusions

- Changing Ghostty's link modifiers or regexes.
- No GhosttyKit patch or rebuild: every action used is in the pinned header.

## Deferred findings

- **One Desktop link router for every surface.** `TaskBriefView` hands
  `loopflow://` to Launch Services; `TaskHTMLView.TaskHTMLLinks` lets
  non-web schemes fall into WKWebView navigation. Both can use the Work resolver and Desktop window opener. File during ship-review.
- **Files pane for Repository and Wave workspaces.** `TaskFilesStore`,
  `MultiplexerStore` and server `task.file` are Task-only. Needs repository
  file reads on the server and a workspace layout change. Its own Task; until
  then those panes open safe local files in the default app (Q1 → 1a).
- **iPhone Sessions.** `swift/LoopflowiOS/GhosttySurface.swift` ignores
  `OPEN_URL`, Ghostty's iOS opener is unimplemented, and a tap carries no ⌘.
  Needs a touch gesture designed, then `TerminalLink` shared. Its own Task.

## Second concept review: where each finding went

- Route files by the Task's active Files place, with fallbacks → Pass 2.
- Remote `file://` hostnames inside the remote checkout → Pass 2 (Q3 → 3a).
- `loopflow://` grammar matching `WorkUrl`, in-app Work links → Pass 2.
- One feedback path for link errors → Pass 2. Hover matching click
  validation → fixed in realign (`TerminalLink.preview`).
- `TERM_PROGRAM=ghostty` on held terminals → Pass 3.
- Shared router, Repository/Wave Files, iPhone → Deferred findings.

## Done when

⌘-click on an agent's link in a Desktop Session opens it in the right place,
the cursor and preview show it is a link, and a Task started from Desktop or
from any terminal prints clickable links.

## Docs

- `docs/surfaces.md` (`### Links in terminal panes`) describes the end state;
  recheck it after Pass 2 for the Repository/Wave and moved-shell fallbacks.
- `swift/README.md` points to it.
