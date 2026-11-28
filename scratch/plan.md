# LOO-1126: links in agent Sessions open from Loopflow Desktop

Brief, word for word: "links in agent sessions that work in terminals like rex
dont work in loopflow desktop".

Nothing is built yet. Base 6deb3b0450.

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
the Session's checkout opens in the Files pane at its line, on this Mac or a
remote Machine. Any Session prints clickable links, whichever terminal started
it.

## Design decisions made for review

The concept review (`scratch/concept-review.html`) asked four questions. The
run was headless and nobody answered them, so each recommended direction is
adopted here and marked for `ship-review` to raise:

- **File links (review Q1 → 1a).** A file inside the pane's checkout opens in
  Desktop's Files pane at `#L<n>`. Only local files outside the checkout go
  to the default app. Passed over: the default app for every file, as Rex
  does. That drops the line, has to refuse repo scripts like `scripts/test.py`,
  and cannot open a file on a remote Machine.
- **Held terminal identity (Q2 → 2a).** Every held terminal sets
  `TERM_PROGRAM=ghostty` and drops `TERM_PROGRAM_VERSION`, whatever the
  starter had. The program always talks to libghostty-vt. Passed over: setting
  it only when unset, which was this plan's first form. That leaves Apple
  Terminal and tmux starters without links, and iTerm/Warp starters invite
  escapes libghostty-vt doesn't implement. Sessions started from Rex lose
  `rex`; lf's host detection (`terminal_host.rs`) reads `REX_SESSION` and
  `REX_BLOCK`, not `TERM_PROGRAM`. `TERM` and `COLORTERM` can still be
  overridden by the caller.
- **Work links (Q3 → 3a).** A `loopflow://` URL that parses as a read-only
  Work address (`loopflow://<repo>[/task/<id>|/wave/<name>]`, same shape as
  `WorkUrl` in `work/link.rs`) opens through `WorkLinkRouter` in this app, with
  no alert. Control hosts (`open`, `update`, `portfolio`, `sessions`) are
  refused, because terminal output must not be able to drive the app. Passed
  over: treating `loopflow://` as an unknown scheme. That shows a warning on
  lf's own links, and Launch Services can wake the installed release app
  instead of the Dev build.
- **FORCE_HYPERLINK** stays rejected: children inherit it, and
  `supports-hyperlinks` honors it with no TTY, so piped tool output would fill
  with escape sequences.

## Design

Three layers, each with one job:

1. **`TerminalLink`** (`Services/Ghostty/TerminalLink.swift`) parses the
   action's bytes and `kind` and classifies them as `.web(URL)`, `.mail(URL)`,
   `.work(URL)`, `.file(path:, host:, line:)`, `.externalScheme(URL)` or
   `.refused(reason)`. It is a pure value with no workspace knowledge. It
   refuses malformed or relative targets, control and bidi characters, and
   fragments other than `#L<n>`. A `file://` host is local if it is empty,
   `localhost`, or `ProcessInfo.processInfo.hostName` (case-insensitive).
   `ls --hyperlink` and `rg` print the hostname. A scheme-less regex match
   becomes `.file`.
2. **`action_cb`** handles `OPEN_URL` by copying the bytes inside the
   callback and passing them to the view's `onOpenLink` closure on the main
   actor. It always returns true for `.osc8`, so Ghostty's fallback can never
   run. `MOUSE_SHAPE` sets the view's `NSCursor`. `MOUSE_OVER_LINK` passes the
   target to the view for the preview and clears it on an empty URL and on
   `mouseExited`.
3. **The pane** (`WorkspacePanesView`, which knows the Machine, the checkout,
   `WorkLinkRouter` and `showBrowser(.files)`) routes the classification:
   - `.web`, `.mail`: `NSWorkspace.shared.open`.
   - `.work`: `WorkLinkRouter`, the same path as `handleDeepLink`'s default
     case.
   - `.file` inside the pane's checkout, local or remote: Files pane through
     `TaskFilesStore.openPresentation` with the line, the way
     `DesktopActions.swift:552` does. The server's read-only checkout reader
     enforces containment and never runs anything.
   - `.file` outside the checkout on this Mac: resolve symlinks, require an
     existing regular file or folder, refuse app bundles, scripts and
     executables (extension, content type, executable bit), then
     `NSWorkspace.shared.open`.
   - `.file` outside the checkout in a remote Session: refused, "File is on
     another Machine". Never look up the same path on this Mac.
   - `.externalScheme`: an alert names the target and asks before opening.
   - `.refused`: notice only.

**Feedback slot.** The hover target and refusal notices share one
bottom-corner overlay on `GhosttyTerminalMount`, visible whether or not the
top strip shows. `copyConfirmation` lives in the strip, which is hidden on a
solo shell pane (`showsStrip`, `WorkspacePanesView.swift:358`), so it can't
carry these.

**Paused input.** `GhosttyMetalView.flagsChanged`
(`GhosttyTerminalView.swift:1351`) returns early when
`!connectionAcceptsInput`. A modifier-only event must still update Ghostty's
local state while nothing is written to the PTY. Without that, ⌘-hover does
nothing on a reconnecting or ended screen.

## Remaining passes

### Pass 1: Desktop opens web links, with hand cursor and preview

`TerminalLink` with the full classification, the three `action_cb` cases,
`onOpenLink` routing for `.web`, `.mail`, `.externalScheme`, `.refused` and
local outside-checkout `.file`, the overlay, and the `flagsChanged` fix.

Shows: in the Dev app, ⌘ over an agent's Markdown link turns the cursor to a
hand and shows the target; a click opens the browser. Teaches whether the
action arrives for a relayed Session. If it doesn't, the fault is in the relay
and the later passes change.

Tests: `TerminalLinkTests` (headless) over the classification, including
`file:///tmp/x.command`, a symlink to an app, `https:relative`, a bidi
override, `#L12`, `file://<hostName>/…`, `loopflow://update`,
`loopflow://open?repo=/tmp`. One `.requiresDisplay` case in
`TerminalFeedbackTests` feeds `ESC ] 8 ;; https://example.com ESC \ text`, then
⌘-clicks the cell and asserts `onOpenLink` received the URL.

### Pass 2: Work links and checkout files open in Desktop

Route `.work` through `WorkLinkRouter`. Route checkout `.file` to the Files
pane at its line, choosing by the pane's Machine and checkout path. Refuse
remote paths outside the checkout.

Shows: ⌘-click on `lf open --link` output inside a Session selects that Task in
this window. ⌘-click on `scripts/test.py#L42` opens the Files pane at line 42,
including in a remote Session.

Tests: routing decisions over (local/remote) × (inside/outside the checkout)
in a headless pane-router test with the opener closures stubbed.

### Pass 3: every held terminal names itself ghostty

`registry.rs`: set `TERM_PROGRAM=ghostty` and drop `TERM_PROGRAM_VERSION` on
every held terminal, next to the `TERM`/`COLORTERM` defaults. Extend the env
case in `transport/registry_tests.rs:157` with cases where the caller's value
is unset, `rex` and `Apple_Terminal`, with a stale `TERM_PROGRAM_VERSION`.

Codex: its link output was not probed. QA records what a Codex Session prints.

## Acceptance check

Headless: `cargo test -p loopflow-execution --lib transport::registry_tests::`
and the `TerminalLinkTests` suite pass.

Demo (Dev app): start a Task from Desktop so the server launches its Session.
Ask the agent for `[the PR](https://github.com/…)`, a checkout file at a line,
and `lf open --task <ID> --link`. ⌘-hover shows the hand and the target.
⌘-click opens the browser, the Files pane at the line, and the Task.

## Absent and error states

- Refused target: nothing opens, the overlay says why, and the callback still
  returns true.
- Missing local file: refused as missing.
- Remote file outside the checkout: refused as on another Machine.
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

- **One Desktop link router for every surface.** `TaskHTMLView.TaskHTMLLinks`
  opens only http/https/mailto externally and lets `loopflow://` fall into
  WKWebView navigation. `TaskBriefView` hands `loopflow://` to Launch Services.
  Moving both onto Pass 2's router is a small Task (~100 lines of Swift plus
  tests). File it after Pass 2 so the router exists to share.
- **iPhone Sessions.** `swift/LoopflowiOS/GhosttySurface.swift` ignores
  `OPEN_URL`, Ghostty's iOS opener is unimplemented, and a tap carries no ⌘.
  It needs a touch gesture designed first, then `TerminalLink` shared across
  both platforms. Its own Task.

## Done when

⌘-click on an agent's link in a Desktop Session opens it in the right place,
the cursor and preview show it is a link, and a Task started from Desktop or
from any terminal prints clickable links.
