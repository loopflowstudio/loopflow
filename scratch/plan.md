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

Four headless concept reviews asked these questions; nobody answered them.
Each recommended direction is adopted and marked for `ship-review` to raise.
Numbers refer to the second review (after Pass 1); the fourth review
(`scratch/concept-review.html`) re-asks them as Q2–Q4 and adds Q1 (zoom)
and Q5 (scope).

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
- **Zoomed terminals and Dashboard tiles (third and fourth review Q1 → 1a).**
  A checkout file link shows Files beside its Session, whether clicked inside
  a zoomed Task view or from a Dashboard tile: it clears terminal zoom and
  leaves keyboard focus on the terminal. Passed over: full-screen Files
  inside a zoomed Task (built in Pass 4, hides the Session), and flipping
  only the tile to Files.
- **Ship terminals first (fourth review Q5 → 5a).** Brief, Markdown and
  review-page links move to the shared router in a follow-up Task (Deferred
  findings).
- **FORCE_HYPERLINK** stays rejected: children inherit it, and
  `supports-hyperlinks` honors it with no TTY, so piped output fills with
  escapes.

## Built

- Pass 1 (`a12082957b`, hover fix `0552209e91`): Ghostty delivers OSC 8 clicks and ⌘-hover feedback. Desktop opens web, mail and safe local documents and confirms external schemes.
- Pass 2 (`e50723744`): Work links resolve in the repository window, checkout links reveal Files at the line, Dashboard tiles show the Files pane, and link failures appear in the originating terminal.
- Pass 3 (`04c2f98af7`): New held terminals report `TERM_PROGRAM=ghostty` and remove the caller's `TERM_PROGRAM_VERSION`.
- Pass 4 (`98c105742`): Remote links use `task.file`, hover and click share the `TerminalHyperlink` classifier, and tile clicks enter the Task with Files beside its Session.

## Acceptance check

Pass 1 passed the native paused-Session OSC 8 fixture. Pass 2 passed
`swift test --package-path swift --filter 'TerminalLinkTests|TerminalFeedbackTests'`
with the two display tests skipped, and both
`cargo test -p loopflow-execution --lib ops::task::directory::tests::` tests passed.
The signed Xcode build and live local and remote agent demos remain for CI and QA.

Pass 3 passed `cargo test -p loopflow-execution --lib held_children_get_ghostty_identity_and_default_terminal_colors`.
The PTY child saw `ghostty` and no version with server, Rex, and Apple
Terminal inputs. QA still needs to check what a live Codex Session prints.

Pass 4 passed `TerminalHyperlinkTests|TerminalFeedbackTests` with two display
fixtures skipped. The focused Rust snapshot, directory and documented-command
tests passed. Files performs its normal read on mount after the remote link
lookup.

Demo (Dev app): start a Task from Desktop so the server launches its Session.
Ask the agent for `[the PR](https://github.com/…)`, a checkout file at a line,
and `lf open --task <ID> --link`. ⌘-hover shows the hand and the target.
⌘-click opens the browser, the Files pane at the line, and the Task.

## Absent and error states

- Refused target: nothing opens, the overlay says why, and the callback still
  returns true.
- Missing file on this Mac or a remote Machine: refused as missing.
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
  non-web schemes fall into WKWebView navigation. Both can use the Work
  resolver and Desktop window opener, and the terminal's Swift
  `isWorkAddress` copy of `WorkUrl::parse` may shrink to a scheme check.
  File during ship-review.
- **Files pane for Repository and Wave workspaces.** `TaskFilesStore`,
  `MultiplexerStore` and server `task.file` are Task-only. Needs repository
  file reads on the server and a workspace layout change. Its own Task; until
  then those panes open safe local files in the default app (Q1 → 1a).
- **iPhone Sessions.** `swift/LoopflowiOS/GhosttySurface.swift` ignores
  `OPEN_URL`, Ghostty's iOS opener is unimplemented, and a tap carries no ⌘.
  Needs a touch gesture designed, then `TerminalLink` shared. Its own Task.

## Remaining: Pass 5 (fourth concept review, `9a7c6e5442`)

`scratch/concept-review.html` reviewed `bba5ff2138`. Pass 4's four findings
hold. Four seams in built code remain; all are Pass 5:

1. **One remote read per click.** `TerminalHyperlink.File.destination`
   fetches a `TaskFileSnapshot` through `query.taskFile` and discards it;
   `TaskFilesView` then reads the file again over SSH. Carry the snapshot on
   `.checkout` and seed it with `files.document(path).reconcile(snapshot)`
   before `openPresentation`, so mount does not refetch it. `reconcile`
   already merges dirty drafts and defers during saves and IME composition,
   which answers Pass 4's reason for the second read.
2. **One zoom behavior.** `openTerminalFile`
   (`WorkspacePanesView.swift:1055`) passes `focus: onEnter == nil`, and
   `MultiplexerStore.showBrowser` (`MultiplexerStore.swift:244`) unzooms only
   when `focus` is false. Inside a zoomed Task the click zooms Files and moves
   focus. Split "show beside, unzoomed" from "focus" in `showBrowser` and use
   the 1a behavior for both entry points.
3. **⌘ over an unfocused pane.** The tracking area uses
   `.activeInKeyWindow`, but `flagsChanged` reaches only the first responder.
   Pressing ⌘ over a link in an unfocused pane or tile shows nothing until
   the pointer moves, and releasing ⌘ leaves the pill and hand cursor stuck.
   Deliver modifier changes to the hovered `GhosttyMetalView` with a local
   event monitor, as `WorkPalette` does.
4. **Drop the Swift `WorkUrl::parse` copy.** `isWorkAddress`
   (`TerminalHyperlink.swift:97`) is not a security check: `.work` clicks go
   only to `query.workLink`. It mislabels `#frag` and `LOOPFLOW://` as
   "only Work" refusals and lets invalid IDs pass hover. Classify any
   `loopflow://` as `.work`; the click shows Rust's own error in the pane
   notice.

Pass 5 should teach whether one round trip removes the remote loading flash,
and leave the link code with one Work grammar. Its checks are
`TerminalHyperlinkTests|TerminalFeedbackTests`, plus a `MultiplexerStore`
test for clicks into a zoomed layout.

Questions Q2–Q4 re-ask the adopted 2a / 4a / 3a choices for `ship-review`.

## Done when

⌘-click on an agent's link in a Desktop Session opens it in the right place,
the cursor and preview show it is a link, and a Task started from Desktop or
from any terminal prints clickable links.

## Docs

- `docs/surfaces.md` (`### Links in terminal panes`) describes link behavior,
  including opening a checkout file from a Dashboard tile.
- `swift/README.md` points to it.
