# 2ac1ee: a Session follows the view someone is at

## Outcome

Opening Loopflow Dev, a second Desktop window or any background view against
the live Machine leaves running Sessions alone: no resize, no `SIGWINCH`, no
theme change, no focus report to the agent. Size, colors and the agent's
sense of focus move when a person focuses a view or types in it, and at no
other time.

Done when:

- A view that attaches without a person at it never changes the PTY size or
  the owner of a Session another live view holds, and the agent never hears
  that view lose focus.
- That view takes the size the moment it gains focus or types, and inherits it
  when the holder leaves.
- `lf resume` typed into a terminal still takes the size at once, as today.
- A view that reconnects by itself attaches with the focus it last had.
- Loopflow Dev launched on the live Machine starts no terminal and resumes no
  Session nobody opened in Dev.

## The model

A Session has one **view in use**: the live view a person most recently
focused or typed in. Four things follow it and nothing else:

| Follows the view in use | Today | After |
| --- | --- | --- |
| PTY size | most recent attach, type or focus | most recent type or focus; an attach only when it says a person is there |
| colors | same | same |
| answers to the agent's terminal queries | same | same |
| focus reports the agent hears | any view's | focus-in from any view (it becomes the view in use); focus-out only from the view in use |

A view's **focus** is one fact with one name on the wire: stated at attach
(`focused`), updated by `focus {focused}`, implied by typing. `Sizing`
(`rust/loopflow-execution/src/transport.rs:853`) already keeps the view in use
by recency. The bug is that attaching counts as use
(`transport/hosted.rs:414`, unconditional `sizing.touch`).

## Server

`Request::Attach` gains `focused: Option<bool>` (`transport/wire.rs:167`).

| `focused` | Meaning | Server |
| --- | --- | --- |
| absent | a relay from before this change | takes the size, as today |
| `true` | a person is at this view | takes the size |
| `false` | nobody is known to be at it | no `touch`; holds the size only while no live view is more recent |

`Request` has no `deny_unknown_fields` and `from: Option<u64>` already sets the
precedent for a field older relays omit, so both skews decode: an old server
ignores the field and behaves as today.

Three details:

- **Ties keep the owner.** `Sizing::settle` uses `max_by_key`, which returns
  the last of equal keys. Two unfocused views both have `active == 0`, so the
  second would take the size from the first. `settle` keeps the current owner
  while it is live and nobody is more recent.
- **The `attached` reply names the real owner.** `hosted.rs:415` answers
  `owner: Some(self)` before `settle` runs. An unfocused attach with a live
  holder answers with the holder. Swift's `OwnerStream` reads this field
  (`OwnerProtocol.swift:126`).
- **Focus-out reaches the agent only from the view in use.** The `Focus` arm
  (`hosted.rs:343`) forwards `CSI I`/`CSI O` from any view. A warm pane
  reports focus-out the moment it attaches (see below), and an agent that
  asked for focus reports would hear it while a person types in another view.
  `focused: true` is forwarded as today (that view is now the one in use);
  `focused: false` is forwarded only when `sizing.held_by(viewer)`. This is
  the rule query answers already follow (`input::without_reports`).

Everything else already handles a view that doesn't own: its first screen is
painted at the PTY's size with a `resize` event (`hosted.rs:484-494`), its
terminal answers are dropped, and its first typed byte or
`focus {focused:true}` calls `touch`.

## Relay

The relay keeps its view's focus as one value and sends it with **every**
attach, first or reconnect (`transport.rs:1535 attachment`, used by
`continuation` too):

- starts `true` when stdin is a terminal and `--unfocused` was not given. A
  person who just pressed Enter on `lf resume` is at that terminal, and no
  terminal can be asked;
- becomes what the terminal last reported (`focus_events`,
  `transport.rs:2092`);
- becomes `true` when the person types.

Without this a relay that reconnects by itself (holder replaced, SSH blip)
would attach as `true` from a terminal the person left, and take the size
from the view they moved to.

`lf session resume --unfocused` is the flag. Desktop passes it for every pane,
selected or warm, appended to `record.openArgv` beside `--agent`
(`SessionsView.swift:1028 launchArguments`). The argv is built by the server
(`human_session_argv`) and wrapped in `ssh` for a remote Machine
(`ssh.rs:345`); a trailing flag reaches the remote relay the way `--agent`
does. A remote `lf` from before the flag refuses it and the pane shows that
error; the fix is upgrading the remote.

A selected pane still gets the size at once with no signal from Desktop:
Ghostty reports the current focus whenever mode 1004 is turned on, and the
relay turns it on at each acknowledged attach (`transport.rs:1246
report_focus`). Confirmed in the pinned source (`a60e9e2`,
`src/termio/stream_handler.zig:766`: `.focus_event => if (enabled)
self.messageWriter(.{ .focused = self.terminal.flags.focused })`;
`Terminal.zig:132 focused: bool = true`). Desktop corrects the default for a
pane nobody looks at: `createSurface` ends in `syncSurfaceFocus()`
(`GhosttyTerminalView.swift:752`), focus false without first responder in the
key window.

`OwnerStream.attachRequest` (`swift/Loopflow/Services/OwnerProtocol.swift:180`,
the native client used by `OwnerStreamTests`) sends `focused: true`: a view a
person just opened on a device. Its doc comments drop "attaching makes this
view size the terminal".

### Passed over (made for review)

- **A second-class viewer that never takes the size while a first-class one
  lives.** A person typing into a Dev pane laid out for another window's
  width sees a broken screen. Focus and input already say who is looking; a
  class of viewer would be a second answer to the same question.
- **Attach never takes the size, with no wire field.** Terminal.app and some
  tmux setups don't report focus when 1004 turns on, so `lf resume` there
  would show the agent at Desktop's width until the first keypress.
- **A `focus` request right after `attach` instead of a field.** Two frames
  can land in two server passes: one paint at the old size, then a resize.
- **An env var instead of `--unfocused`.** It doesn't cross `ssh`, and it
  reaches every child of a pane.
- **Removing warm conversations from release Desktop.** Warmth restores saved
  tabs and holds worktrees ("Keep a Task Session view open to retain its
  worktree", transport README); with unfocused attaches it no longer disturbs
  anyone.

## Dev opens what a person opens

Opening a conversation is a write: `prepareSession` runs `lf session resume`
(`RegistryQuery.swift:1224`), which views a held terminal, resumes an ended
Session or takes one over from a native client, and each open view holds its
Task's worktree. Release Desktop does this for up to sixteen Tasks plus
primaries at launch because it is the Machine's own window. Dev is a build
someone is trying against that Machine, so it acts on the Work a person
clicks and nothing else. That also covers holders from before this change,
which ignore `focused`.

`SessionsWorkspaceRegistry` takes `opensUnvisitedWork: Bool` at `init`, beside
`localMachineId` and `runtime`; the app passes
`Bundle.main.bundleIdentifier?.hasSuffix(".dev") != true`, as
`Design/RenderCost.swift:47` reads it. When false, `openWarmConversations`
(`SessionsView.swift:604`) skips Work not deliberately visited in this run
(`focus(key, deliberate: true)`); the registry remembers those keys. A
selection restored at launch (`openSelectedWorkspace(deliberate: false)`)
waits behind the existing placeholder and its action
(`openConversation(in:store:)`).

`prefetch`, `prefetchPrimary` and saved warm intent keep running in Dev: they
build workspaces and placeholders and start no process.

Release Desktop keeps warming. Its warm panes attach unfocused, so a second
window or a relaunch over new-enough holders resizes nothing.

## Surfaces and consumers

| Surface | Change |
| --- | --- |
| `transport/wire.rs` `Request::Attach` | `focused: Option<bool>` |
| `transport/hosted.rs` attach arm | `touch` unless `Some(false)`; reply names the real owner |
| `transport/hosted.rs` focus arm | focus-out forwarded only from the view in use |
| `transport.rs` `Sizing::settle` | ties keep the live owner |
| `transport.rs` relay | keeps focus; `attachment` and `continuation` send it |
| `lf session resume` | `--unfocused` |
| `SessionItem.launchArguments` | appends `--unfocused`; assertions in `WorkCacheTests`, `SessionsStoreTests` follow |
| `OwnerProtocol.swift` | `attachRequest` sends `focused: true`; doc comments; `OwnerStreamTests` |
| `SessionsWorkspaceRegistry` | `opensUnvisitedWork` |
| `transport/README.md` §ownership (l.475) | "the view a person most recently focused or typed in"; what an attach says; focus-out rule; `--unfocused` |
| `terminal_holder`, `held.rs` | untouched: they send `Resize`, never `Attach` |

Raw-protocol tests in `rust/loopflow/tests/transport_tests.rs` attach without
the field and keep today's behavior, which is the old-relay case.

`scripts/loopflow-dev.py run` opens a copy of the Machine only when
`lf server describe` differs. `transport::wire::Request` derives no
`JsonSchema` and `SessionRecord` doesn't change, so Dev still opens the real
Machine.

## Absent and error states

- No live view: an unfocused attach owns the size (nobody else to size it).
- The holder leaves: the next most recent live view owns; among views never
  used, the one already owning or the first found.
- A host that reports no focus and attached unfocused: it takes the size on
  the first typed byte. The README's "attach-and-type rule" sentence becomes
  "type rule" for such hosts.
- A holder restarts and every view reattaches: each sends its own focus, so
  the focused one owns whatever the order.
- Old server, new relay: the attach takes the size as today. Dev's lazy
  opening covers this.
- Old remote `lf`, new Desktop: the pane fails on the unknown flag and says
  so.

## Exclusions

- No change to which view may type, to retention (`viewer_hold`), or to
  catch-up and snapshot painting.
- No protection for old holders from release Desktop's warm attaches; they
  clear as Sessions restart.
- `owner` means two things in this area: the Session's owner process
  (`OwnerProtocol`, README l.282) and the view that sizes the terminal
  (`Reply::Attached.owner`, `Event::Owner`). Renaming the second changes
  events live holders already send; it is its own Task.
- PR #2007 (`9a2459`) edits `SessionsView.swift` and `scripts/loopflow-dev.py`;
  the Swift change here stays inside `openWarmConversations`, the registry's
  `init` and the launch argv, and `loopflow-dev.py` isn't touched.

## What goes

- The unconditional `sizing.touch(viewer)` in the attach arm.
- `Some(viewer.id)` as the unconditional `owner` of the `attached` reply.
- Unconditional focus-out forwarding in the focus arm.
- "most recently attached" as an ownership rule in `wire.rs:165`,
  `transport.rs:667,850,882`, `OwnerProtocol.swift:14,179`, the README and the
  doc comment at `transport_tests.rs:1686`.
- Unit test `the_latest_view_sizes_the_terminal_and_hands_back_when_it_leaves`
  is rewritten around the new rule, not kept beside it.

Must survive: `lf resume` in a terminal takes the size at once; a clicked
Desktop pane takes it within one focus report; saved warm tabs restore in
release Desktop; an agent that asked for focus reports still hears the view
in use gain and lose focus.

## Acceptance check at gate

```bash
uv run python scripts/test.py --reuse-passing \
  --rust-filter 'test(an_unfocused_view_leaves_the_session_with_the_view_in_use)'
```

One held terminal whose child turns on mode 1004 and records what it reads,
three raw-protocol views in `transport_tests.rs`: A attaches at 40×120 (no
field); B attaches `focused:false` at 24×80 → B's `attached` reply and `owner`
event name A, both views' `resize` says 40×120, the child sees no `SIGWINCH`;
B sends `focus {focused:false}` → the child reads nothing; B sends
`focus {focused:true}` → `owner` names B, size 24×80, the child reads `CSI I`;
A sends `focus {focused:false}` → the child reads nothing; C attaches
`focused:false` → still B; B leaves → A; A leaves → C.

A unit test beside the relay: after a focus-out report, `continuation()`
sends `focused: false`; after a typed byte, `true`.

```bash
scripts/test_desktop.sh --filter 'WorkspaceRetentionTests|SessionsStoreTests|WorkCacheTests|OwnerStreamTests'
```

A registry with `opensUnvisitedWork: false`, sixteen working Tasks and both
primaries prefetched prepares no Session until one is deliberately focused; a
default registry prepares them as before, each with `--unfocused` in its argv.

Live, by a person or at qa: Desktop open on a disposable Session, then
`lf session resume <id> --unfocused` in a narrower terminal → Desktop's pane
does not reflow; press a key there → it does. Then
`uv run python scripts/loopflow-dev.py run` → `pgrep -lf 'session resume'`
shows no new relay until a Task is clicked in Dev.

## Slices

1. **The server rule, end to end over the wire.** `focused` on `Attach`, the
   attach arm, `settle` ties, the reply's owner, the focus-out rule, and the
   acceptance test above. This is where the model holds or doesn't: three raw
   views and a child that listens.
2. **The relay and the flag.** The relay's focus value on first attach and
   reconnect, `--unfocused`, Desktop's launch argv, `OwnerStream`, README and
   doc comments, the rewritten unit test. One live look: a warm pane's relay
   sends `focus {focused:false}` after attach and a selected pane sends
   `true`.
3. **Dev opens lazily.** `opensUnvisitedWork`, the Swift tests, the live
   check with release Desktop and Dev side by side.

## Checks

| Command | Result | Who runs it |
| --- | --- | --- |
| acceptance Rust filter above | not yet written | implement, then CI |
| relay focus unit test | not yet written | implement, then CI |
| `scripts/test_desktop.sh --filter …` | not yet written | implement, then CI |
| two views at different widths, live | not run | qa |
| Dev beside release Desktop, live | not run | qa |
