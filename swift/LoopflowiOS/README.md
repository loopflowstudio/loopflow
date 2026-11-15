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

## Tasks

The list shows one repository at a time: tap the title to switch, or pick
**All repositories** to see each under its name. The filter button chooses what
to show — **Started** (Desktop's sidebar: Working, then Autonomous), **Ready for
you**, **Working now**, **Backlog** or **Done** — in the Machine's order, within one Wave when you pick it there. Tasks
with a conversation come first; started Tasks nobody has opened a conversation
for fold under "N without a conversation". Search covers title and checkout
path. Pull to refresh. **+** opens New Task: describe the work, pick a
repository, **Create & start**.

## Task

Opening a Task opens its conversation and fits the terminal to the phone.
**Changes** lists the files the Task changed; each opens its diff, and the diff
opens the whole file. A Task with no conversation opens on its Changes.

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

## Terminal

Ghostty draws the Session, the same renderer as Desktop, in the theme's terminal
colours and JetBrains Mono 12 scaled by Dynamic Type. No process runs on the
phone: the bytes the Machine sends are fed to the surface, and what Ghostty
would write back (keys, and its answers to a program's queries such as
`OSC 7501;?`) goes to the Machine. Ghostty encodes the key row and hardware keys
for the modes the program set, so arrows follow application-cursor mode.

The framework is GhosttyKit with the iOS slices from
[`0006-ios-external-io.patch`](../GhosttyKitPatches/README.md), the archive
`Package.swift` pins for Desktop too; `scripts/test_ios.py` resolves it before
generating the project. The Simulator slice is arm64.

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
