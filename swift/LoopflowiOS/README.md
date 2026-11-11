# Loopflow for iOS

```sh
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

The list shows the Machine's open Tasks in the Machine's order and updates while
it is on screen. Pull to refresh; search by title or checkout path. **+** opens
New Task: describe the work, pick a repository, **Create & start**.

## Session

Opening a Task opens its primary Session and fits the terminal to the phone.
The device touched last has the terminal. When another device takes it, the
phone stops drawing and offers **Continue here**.

Type or dictate in **Reply** and send: the text arrives as one paste followed by
Return. The software keyboard never types into the terminal itself, so touching
the screen sends nothing. The arrows button shows Escape, Tab, the arrow keys and
Return for provider menus. A hardware keyboard types directly.

Scrolling up holds your place while output arrives; **Latest** returns to the
end. Leaving the app drops the connection and the Session keeps running;
returning reconnects. A stopped Session offers **Start it**. Nothing typed is
ever sent twice: after an uncertain send the app says so and waits.

## Files

| File | What it is |
|---|---|
| `MachineConnection.swift`, `SavedMachine.swift` | WebSocket to the Machine's server; saved address |
| `RemoteMobileClient.swift`, `RemoteTaskObservation.swift` | Requests, the live Task reading, a Session attachment |
| `OwnerStream.swift` | Decoder for the owner protocol in `rust/loopflow/src/transport/wire.rs` |
| `SessionLink.swift` | The phone's hold on one Session |
| `SessionTerminal.swift`, `SessionView.swift` | Terminal, reply field and keys |
| `TaskListView.swift` | Task list |
| `MobileTaskCreation.swift`, `MobileNewTaskView.swift` | New Task |

```sh
uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests
```
