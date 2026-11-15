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
repository, **Create & start**. The editor closes immediately into the submitted
conversation. Its words appear once beside **Starting conversation…** while the
Machine prepares the Task and Session; native attachment controls input. Leaving for another Task keeps that
selection when a receipt arrives. The recorded Session opens without waiting for
its Task to appear in the feed. Opening the result, including from an ordinary
Task row, clears the redundant request row; unresolved recovery stays visible.
After a local save failure, **Try again** retries the same ID and **Edit request**
returns the words to the editor. Once saved, **Check request** recovers the original
result and **Inspect** shows its receipt when recovery needs attention.
**Reconnect** reopens the same Session after attachment fails. Complete and Abandon
show no extra request row while delivering. A disconnected request shows
**Waiting for connection** without an action and resumes automatically on its
original Machine, including after app restart. The Task feed shows **Waiting for
connection** once above saved Tasks or an empty list. Pull to refresh ends
immediately while waiting; Tasks refresh automatically when the Machine returns.
A request needing attention shows recovery controls; a creation also keeps its row
until its result has been shown.

The shared SQLite outbox saves the original request
before `operation.submit` admits it. Receipt subscriptions observe preparation;
a lost answer is recovered by its original request ID. A replacement Machine
database never authorizes another submission of an unanswered request.
Released phone submission files import to each request's original Machine store,
including exact words, request IDs and saved outcomes. Matching outbox records keep
their receipt and send history. Conflicting or unreadable files remain intact;
missing historical receipts require inspection rather than another submission.
Unsent editor text remains a draft. Retired requests keep only a command digest
and source identity, so a surviving editor file cannot submit again. They retain
no authored words or receipts.

The Task feed restores confirmed query bases from that same SQLite store. Older
replies cannot replace newer saved bases; a retired command leaves a required
reload fence so an older saved list cannot be shown as current. Returning to a
cached filter applies pending completion before showing its rows. Separate feeds
keep their own query leases; closing one leaves the other feed current. A failed
refresh keeps usable cached content beside **Try again**.

Recent feeds share a bounded cache. Returning after eviction or invalidation loads
in the selected feed; a retained answer is checked against saved pending changes
after observation resumes. Closing a feed never cancels its submitted commands.
A feed known to predate settled changes waits for a covering reading.

## Task

Opening a Task opens its conversation and fits the terminal to the phone.
**Changes** lists the files the Task changed; each opens its diff, and the diff
opens the whole file. A Task with no conversation opens on its Changes.

**Complete** removes the Task from affected feeds and counts immediately, then
replaces this route with the next visible Task or returns to the list. Other feeds
keep their selections. The same command saves before sending; its request row's
**Check request** recovers its original identity after an unanswered send. Save failure restores membership
without changing your newer navigation. **Abandon** in Task actions asks for
confirmation before following the same removal and next-Task rule.
A retained Task shows **Completing…** while
its command remains pending. These local changes work while the network is waiting;
provider readiness still comes from the Session connection.

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
