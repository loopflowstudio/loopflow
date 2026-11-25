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
to show — **Working** (Desktop's sidebar, grouped into **Active**, **Shipping**,
and **Paused**), **Ready for you**, **Working now**, **Backlog** or **Done** —
in the Machine's order, within one Wave when you pick it there. Tasks
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
**Idle**, **Active**, **Moving**, **Paused**, **Done**) while nothing reports. **Auto**
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

Opening a Task fits its conversation to the phone. The workflow bar above it
shows the Graph and a split button. Tap ⏸ beside **Running: implement** to
pause the worker while keeping the conversation open. Tap ▶ to resume the
interrupted step. At a human step such as ship-review, ▶ lifts the hold and
waits for you. The next tap runs the step named on the button.

Tap the step label to pick a starting point, pause at review, or interrupt the worker.
When ▶ would start work, the menu also offers **Unpause without running**.
Cleanup disables unpause until the worker stops.
The **Auto** switch stays armed during a hold and can be switched off.
Tap ✓ to complete the Task. **…** opens its PR, Sessions, Wave moves and copy actions.

Under it, **Session · Files · Changes · Shell** switch what the screen shows.
The paired Machine reaches the Task wherever it runs. When another Machine
runs the Task, the workflow bar names it (**On mini**).

- **Session** opens the Task's primary conversation. Pick a recorded Session
  from **…** to open it in the same pane.
- **Files** is the checkout's tree, changed files or not; a folder opens the
  next level and a file opens as it stands. It re-reads itself when the
  checkout changes or the Task moves.
- **Changes** lists the files the Task changed; each opens its diff, and the
  diff opens the whole file. It re-reads when the Task moves.
- **Shell** is a shell in the Task's checkout, started the first time you open
  it and kept running when you leave. After the Task moves, it says the shell
  has ended and offers **New shell**, which starts one on the new Machine.

A Task with no conversation opens on its Changes. One with no checkout yet
says **Not started** on Files, Changes and Shell, with **Try again** on
Changes and Shell; Files fills in by itself once the Task starts. When the Machine running the Task
doesn’t answer, each view names it and offers **Try again** (**Reconnect** in a terminal). Once a
conversation starts, it opens in place, without going back to the list. Task
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
| `TerminalLink.swift` | The phone's hold on one terminal: a Session or the Task's shell |
| `GhosttySurface.swift` | Ghostty's app and one terminal surface fed with the terminal's bytes |
| `SessionTerminal.swift`, `TerminalView.swift` | Terminal scrolling, reply field and keys |
| `TaskListView.swift` | Repository, filter, search and the Task list |
| `TaskWorkspaceView.swift` | A Task's Session, Changes, diffs and files |
| `TaskFilesTree.swift` | Files: the checkout's tree, followed through `task.files.watch` |
| `MobileTaskCreation.swift`, `MobileNewTaskView.swift` | New Task |

```sh
uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests
```
