# 2ac1ee: a view takes the size only when someone is at it

## Outcome

Opening Loopflow Dev, a second Desktop window or any background view against
the live Machine leaves running Sessions alone: no resize, no `SIGWINCH`, no
theme change. The size and colors move when a person focuses a view or types
in it, and at no other time.

Done when:

- A view that attaches without a person at it never changes the PTY size or
  the owner of a Session another live view holds.
- That view takes the size the moment it gains focus or types, and inherits it
  when the holder leaves.
- `lf resume` typed into a terminal still takes the size at once, as today.
- Loopflow Dev launched on the live Machine starts no terminal for a Session
  nobody opened in Dev.

## One rule, in the server

`Sizing` (`rust/loopflow-execution/src/transport.rs:853`) already follows the
view most recently "used". The bug is that attaching counts as use
(`transport/hosted.rs:414`, unconditional `sizing.touch`). The change: an
attach counts as use only when the view says a person is at it.

`Request::Attach` gains `focused: Option<bool>` (`transport/wire.rs:167`).

| `focused` | Meaning | Server |
| --- | --- | --- |
| absent | a relay from before this change | takes the size, as today |
| `true` | a person launched this view from their terminal | takes the size |
| `false` | launched by a host, or not on a terminal | no `touch`; holds the size only while no live view is more recent |

`Request` has no `deny_unknown_fields` and `from: Option<u64>` already sets the
precedent for a field older relays omit, so both skews decode: an old server
ignores the field and behaves as today.

Two details the rule needs:

- **Ties keep the owner.** `Sizing::settle` uses `max_by_key`, which returns
  the last of equal keys. Two unfocused views both have `active == 0`, so the
  second would take the size from the first. `settle` keeps the current owner
  while it is live and nobody is more recent.
- **The `attached` reply names the real owner.** `hosted.rs:415` answers
  `owner: Some(self)` before `settle` runs. An unfocused attach with a live
  holder answers with the holder.

Everything after that already handles a view that doesn't own: its first
screen is painted at the PTY's size with a `resize` event
(`hosted.rs:484-494`), its terminal answers are dropped
(`input::without_reports`), and its first typed byte or `focus {focused:true}`
calls `touch`.

### Who says `false`

The relay (`transport.rs:1535 attachment`) sends `focused: true` only when its
stdin is a terminal and it wasn't told otherwise. A person who just pressed
Enter on `lf resume` is at that terminal; no terminal can be asked.

Desktop panes launch the same relay (`lf session resume <id>`,
`SessionsView.swift:1217 startTerminal`) and pass `--unfocused`, for every
pane, selected or warm. Desktop runs its own bundled `lf` (`LF_BIN`,
`ProcessEnvironment.swift:39`), so the flag and the relay are always one
build.

A selected pane still gets the size at once, with no signal from Desktop,
because Ghostty reports the current focus when mode 1004 is turned on:
`src/termio/stream_handler.zig:755` (`.focus_event => if (enabled)
messageWriter(.{ .focused = terminal.flags.focused })`) and
`termio/Thread.zig:335` → `focusGained`, which writes `CSI I` or `CSI O`. The
relay turns 1004 on when it is acknowledged (`transport.rs:1946
report_focus`) and forwards the report as `Request::Focus`. Ghostty assumes
focus by default (`Terminal.zig:105 focused: bool = true`), and Desktop
corrects that for a pane nobody looks at: `createSurface` ends in
`syncSurfaceFocus()` (`GhosttyTerminalView.swift:752`), which sets focus false
for a view with no key window before the child has started.

Read from `~/src/loopflow/vendor/ghostty` at `0149fd7`; the app's pin is
`a60e9e2` (`rust/loopflow-terminal/ghostty-vt.json`). Slice 1 confirms it on
the pinned build.

### Passed over (made for review)

- **A second-class viewer that never takes the size while a first-class one
  lives.** A person typing into a Dev pane laid out for another window's
  width sees a broken screen. Focus and input already say who is looking; a
  class of viewer would be a second answer to the same question.
- **Attach never takes the size, with no wire field.** Terminal.app and some
  tmux setups don't report focus when 1004 turns on, so `lf resume` there
  would show the agent at Desktop's width until the first keypress.
- **An env var instead of `--unfocused`.** `LF_DESKTOP` reaches every child of
  a pane, including a tmux a person starts there; the flag reaches only the
  relay Desktop launches.
- **Removing warm conversations from release Desktop.** Warmth restores saved
  tabs and holds worktrees ("Keep a Task Session view open to retain its
  worktree", transport README); with unfocused attaches it no longer disturbs
  anyone.

## Dev opens what a person opens

Unfocused attaches don't protect Sessions whose holder predates this change:
an old server ignores the field. Dev always meets those (the live Machine's
Sessions were started by the installed `lf`), so Dev starts no terminal for
Work nobody visited.

`SessionsWorkspaceRegistry.openWarmConversations`
(`SessionsView.swift:604`) walks `warmWork`: selected, visible tiles,
primaries, recent. In Dev it opens only Work a person deliberately visited in
this run (`focus(key, deliberate: true)`, `openConversation`). A selection
restored at launch waits behind the existing placeholder and its open action.
Dev is `Bundle.main.bundleIdentifier?.hasSuffix(".dev")`, as
`Design/RenderCost.swift:47` already reads it; the registry takes it as a
value at `init`, beside `localMachineId` and `runtime`.

`prefetch`, `prefetchPrimary` and saved warm intent keep running in Dev: they
build workspaces and placeholders and start no process.

Release Desktop keeps warming. Its warm panes attach unfocused, so a second
window or a relaunch over new-enough holders resizes nothing.

## Surfaces and consumers

| Surface | Change |
| --- | --- |
| `transport/wire.rs` `Request::Attach` | `focused: Option<bool>` |
| `transport/hosted.rs` attach arm | `touch` only unless `Some(false)`; reply names the real owner |
| `transport.rs` `Sizing::settle` | ties keep the live owner |
| `transport.rs` relay `attachment` | sends `focused` |
| `lf session resume` / attach path | `--unfocused` |
| `SessionsStore` launch argv | adds `--unfocused`; `launchArguments` assertions in `WorkCacheTests`, `SessionsStoreTests` follow |
| `SessionsWorkspaceRegistry` | Dev opens only deliberately visited Work |
| `transport/README.md` §ownership (l.475) | "attached with a person at it, typed or gained focus"; the `--unfocused` rule |
| `terminal_holder`, `held.rs` | untouched: they send `Resize`, never `Attach` |

Raw-protocol tests in `rust/loopflow/tests/transport_tests.rs` attach without
the field and keep today's behavior, which is the old-relay case.

## Absent and error states

- No live view: an unfocused attach owns the size (nobody else to size it).
- The holder leaves: the next most recent live view owns; among views never
  used, the one already owning or the first found.
- A host that reports no focus and attached unfocused: it takes the size on
  the first typed byte. The README's "attach-and-type rule" sentence becomes
  "type rule" for such hosts.
- Old server, new relay: the attach takes the size as today. Dev's lazy
  opening is what covers this.
- `scripts/loopflow-dev.py run` opens a copy of the Machine when a branch
  "changes what the server sends". `Request::Attach` travels the other way on
  the Session socket; slice 1 checks that the detector agrees, since the live
  acceptance needs Dev on the real Machine.

## Exclusions

- No change to which view may type, to retention (`viewer_hold`), or to
  catch-up and snapshot painting.
- No protection for old holders from release Desktop's warm attaches; they
  clear as Sessions restart.
- PR #2007 (`9a2459`) edits `SessionsView.swift` and `scripts/loopflow-dev.py`;
  the Swift change here stays inside `openWarmConversations` and the launch
  argv, and `loopflow-dev.py` isn't touched.

## What goes

- The unconditional `sizing.touch(viewer)` in the attach arm.
- `Some(viewer.id)` as the unconditional `owner` of the `attached` reply.
- "most recently attached" as an ownership rule in `wire.rs:165`,
  `transport.rs:667,850,882`, the README and the doc comment at
  `transport_tests.rs:1686`.
- Unit test `the_latest_view_sizes_the_terminal_and_hands_back_when_it_leaves`
  is rewritten around the new rule, not kept beside it.

Must survive: `lf resume` in a terminal takes the size at once; a clicked
Desktop pane takes it within one focus report; saved warm tabs restore in
release Desktop.

## Acceptance check at gate

```bash
uv run python scripts/test.py --reuse-passing \
  --rust-filter 'test(an_unfocused_view_leaves_the_size_with_the_view_in_use)'
```

One held terminal, three raw-protocol views in `transport_tests.rs`: A attaches
at 40×120 (no field); B attaches `focused:false` at 24×80 → B's `attached`
reply and `owner` event name A, both views' `resize` says 40×120, the child
sees no `SIGWINCH`; B sends `focus {focused:true}` → `owner` names B, size
24×80; C attaches `focused:false` → still B; B leaves → A; A leaves → C.

```bash
scripts/test_desktop.sh --filter 'WorkspaceRetentionTests|SessionsStoreTests|WorkCacheTests'
```

A Dev registry with sixteen working Tasks and both primaries prefetched
prepares no Session until one is deliberately focused; a release registry
prepares them as before, each with `--unfocused` in its argv.

Live, by a person or at qa: Desktop open on a disposable Session, then
`lf session resume <id>` in a narrower terminal with `--unfocused` → Desktop's
pane does not reflow; press a key there → it does. Then
`uv run python scripts/loopflow-dev.py run` → `pgrep -lf 'session resume'`
shows no new relay until a Task is clicked in Dev.

## Slices

1. **The server rule, end to end over the wire.** `focused` on `Attach`,
   the attach arm, `settle` ties, the reply's owner, and the acceptance test
   above. Then the probe that could sink the Desktop half: on the pinned
   Ghostty build, start a pane unmounted and confirm its relay sends
   `focus {focused:false}` on attach, and a selected pane sends `true`. If the
   pinned build doesn't report on enable, Desktop passes focus at launch
   instead and the pane re-sends it from `syncSurfaceFocus`. Also run
   `loopflow-dev.py run` once to see that it still opens the Machine, not a
   copy.
2. **The relay and the flag.** `focused` from the relay (tty and
   `--unfocused`), Desktop's launch argv, README and doc comments, the
   rewritten unit test.
3. **Dev opens lazily.** The registry's Dev value, the Swift tests, the
   live check with release Desktop and Dev side by side.

## Checks

| Command | Result | Who runs it |
| --- | --- | --- |
| acceptance Rust filter above | not yet written | implement, then CI |
| `scripts/test_desktop.sh --filter …` | not yet written | implement, then CI |
| two views at different widths, live | not run | qa |
| Dev beside release Desktop, live | not run | qa |
