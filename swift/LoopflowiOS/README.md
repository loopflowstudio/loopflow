# Loopflow for iOS

```sh
uv run python scripts/loopflow-dev.py ghostty-build   # once: the terminal's framework
xcodegen generate --spec swift/ios.yml
open swift/LoopflowMobile.xcodeproj
```

Run **LoopflowiOS** on your iPhone, connect to your Machine, open a Task and
reply to its Session.

## Connect

Sign in to the same Tailscale account on both devices. In Desktop, open
Settings → Mobile → **Enable mobile access**, then scan its code on the phone.
**Enter address** accepts the same HTTPS address typed or pasted. The code holds
only the address: Tailscale identifies the caller and the Machine admits its
owner. The phone keeps the address and Machine identity in Keychain.

A Simulator on the server's Mac shares its Tailscale identity and is refused
until that Mac opts in with `touch ~/.lf/mobile-same-device`.

## Theme

The phone draws your Machine's theme: whatever Desktop's Choose Theme applied
(`theme:` in the Machine's `~/.lf/config.yaml`). It reads it on connect and
whenever the app comes back, and remembers the last answer so launch opens in
it. `light:Fig Leaf,dark:Fig` follows the phone's appearance; one name is that
theme in either. A name the phone doesn't ship, such as your own theme file or
a Ghostty theme, draws Loopflow Light or Dark. Before you connect, Loopflow's
own pair.

## Tasks

The list shows one repository at a time: tap the title to switch, or pick
**All repositories** to see each under its name. The filter button chooses what
to show — **Started** (Desktop's sidebar: Working Tasks), **Ready for
you**, **Working now**, **Paused**, **Backlog** or **Done** — in the Machine's order, within one Wave when you pick it there. Tasks
with a conversation come first; started Tasks nobody has opened a conversation
for fold under "N without a conversation". Search covers title and checkout
path. Pull to refresh.

Each row shows the Task’s full title, then its ID, status and current Graph
step when it has one.

```text
Loopflow for iPhone uses the shared design
tokens and components
LOO-5  Busy  Auto  implement
```

The word is the one the Mac shows for that Task: what its conversation reports
(**Blocked**, **Error**, **Busy**, **Available**), or its lifecycle word (**Unstarted**,
**Idle**, **Active**, **Moving**, **Done**) while nothing reports. **Auto**
follows while Auto-Ship is on. A Task whose reading carries
neither says **Unavailable**. The row carries no stripe: colour is on the word.

**+** opens New Task. Under "What do you want?", describe the work, pick a
repository, **Create & start**. The Task's conversation opens as soon as the
Machine answers and prints its setup (checkout, then Graph) before the agent's
prompt.

Swipe a Task left and tap **Complete**: the row leaves the list at once. If the
Machine refuses, the row comes back and a line at the bottom of the screen says
why. A list that fails to load keeps what it last showed and says so there too.
The list opens on the last reading this phone saved for that Machine and
filter, then updates.

A list with nothing in it says so and offers the next step: **New Task** when
the filter is empty, **Clear search** when the search matches nothing.

## Task

Opening a Task with a conversation fits its terminal to the phone, even while
the Graph is paused. Above it, one sentence says where the Task stands:

```text
build is running implement. You are next needed at ship-review.
```

It is the sentence `lf task status` prints and the Mac's workflow bar shows,
in full: it wraps rather than cuts, and it changes as the Task moves. Turned
sideways, or on an iPad, the Graph's steps stand before it as they do on the
Mac. A Task that has not taken up a Graph shows no sentence.

**Task actions** in the toolbar holds what the Mac's workflow bar offers:

- **Start** before the Task has run, **Run** once its Graph has stopped or is
  waiting at a step. While a run carries the Task, neither is there.
- **Pause** stops a started Task and keeps your ownership. **Unpause** returns
  it to the list without running its Graph. While cleanup finishes, Unpause is disabled.
- **Auto-Ship** turns the Task's auto setting on and runs it, as
  `lf task auto on` does; **Stop Auto-Ship** turns it off. Every Task that is
  not completed has one of them.

Run and auto requests show a line like "LOO-5 Auto-Ship requested" at the bottom of the
screen. An action the Machine already knows it would refuse is greyed, and
the reason stands in full under the workflow sentence; any other refusal shows
in that line.

**Changes** lists the files the Task changed; each opens its diff, and the diff
opens the whole file. A Task with no conversation opens on its Changes; one
that has not started says "No changes yet" and offers **Start**. Once it
starts, its conversation opens in place, without going back to the list. Task
actions follow the Task itself, so they stay when the list's filter no longer
shows it.

The device touched last has the terminal. When another device takes it, the
phone stops drawing and offers **Continue here**.

Type or dictate in **Reply** and send: the text arrives as one paste followed by
Return. The software keyboard never types into the terminal itself, so touching
the screen sends nothing. The arrows button shows Escape, Tab, the arrow keys and
Return for provider menus. A hardware keyboard types directly once you tap the
terminal.

Scrolling up holds your place while output arrives; **Latest** returns to the
end. Leaving the app drops the connection and the Session keeps running;
returning reconnects. A stopped Session offers **Start it**. Nothing typed is
ever sent twice: after an uncertain send the app says so and waits.

When there is no terminal to draw (the Session ended, is saved but not
running, is open on another device, or the connection dropped), the screen
says which in a line of serif and offers the one button that fits. A Task
whose Session cannot be reached from here offers **Changes**. Changes,
a diff and a file do the same when they are empty or fail to load; a failure
keeps **Try again**.

## Terminal

Ghostty draws the Session, the same renderer as Desktop, in the theme's terminal
colours and Spline Sans Mono 12 scaled by Dynamic Type, with ligatures off and
10% more cell height, as on the Mac. No process runs on the
phone: the bytes the Machine sends are fed to the surface, and what Ghostty
would write back (keys, and its answers to a program's queries such as
`OSC 7501;?`) goes to the Machine. Ghostty encodes the key row and hardware keys
for the modes the program set, so arrows follow application-cursor mode.

The framework is GhosttyKit with the iOS slices from
[`0006-ios-external-io.patch`](../GhosttyKitPatches/README.md), the archive
`Package.swift` pins for Desktop too; `scripts/test_ios.py` resolves it before
generating the project. The Simulator slice is arm64.

## Look

The phone draws with the Mac's parts. The Task row, status word, workflow
sentence, button and empty or failed state are the shared components in
[`swift/Loopflow/Design/`](../Loopflow/Design/), in the same three faces, on
the iPhone column of the type ramp in [DESIGN.md](../../DESIGN.md#type). Text
scales with Dynamic Type, nothing meant to be read is under 12 pt, and every
control is 44 pt tall. Each screen has one accent button.

A view names a step (`Typography.text`, `Spacing.md`, `CornerRadius.pane`) and
never passes a number; `DesignDisciplineTests` reads this directory and
fails on a literal size, padding or radius, a `.borderedProminent` button or a
`ContentUnavailableView`.

```sh
cd swift && swift test --filter DesignDisciplineTests
```

## Files

| File | What it is |
|---|---|
| `MachineConnection.swift`, `SavedMachine.swift` | The WebSocket that carries `Loopflow`'s shared `ServerLink`; saved address |
| `RemoteMobileClient.swift`, `RemoteTaskObservation.swift` | Requests, the live Task reading (resumed after a lost connection), a Session attachment |
| `SessionLink.swift` | The phone's hold on one Session |
| `GhosttySurface.swift` | Ghostty's app and one terminal surface fed with the Session's bytes |
| `SessionTerminal.swift`, `SessionView.swift` | Terminal scrolling, reply field and keys |
| `TaskListView.swift` | Repository, filter, search and the Task list |
| `TaskWorkspaceView.swift` | A Task's conversation and its Changes |
| `MobileTaskCreation.swift`, `MobileNewTaskView.swift` | New Task |

```sh
uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests
```
