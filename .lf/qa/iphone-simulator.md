Read when the change reaches the iOS app's screens and a Simulator on the Machine's Mac will do.

# iPhone app in a Simulator, against the live Machine

Needs `lf server mobile --status` to say `"ready":true` and
`~/.lf/mobile-same-device` to exist; both are the person's settings, so read
them and stop if they are off. Paired that way, the phone talks to the
installed server and the real store.

To put a branch server behind the phone, give a private Machine its own
listener: start the branch `lf server run` with that `LF_HOME` (and `HOME`),
`touch $LF_HOME/mobile-same-device`, then run the same binary's `lf server
mobile`. Its server already answers with the same executable, so no launchd
job is registered; the listener takes another port on the same tailnet name,
and `lf server mobile --status` in that home prints the endpoint to type.
Machine settings for the phone to read (`theme:`) go in `$LF_HOME/config.yaml`;
the default portfolio discovers repositories under `$HOME/src`. A private
repository outside it appears in the feed after `lf desktop portfolio manual`
and `lf desktop portfolio add <repo>` on the private Machine. "Forget Machine"
in the Machine menu unpairs; the pairing survives `simctl uninstall`.

```sh
brew install cameroncooke/axe/axe          # taps, typing, swipes and the accessibility tree, headless
U=$(xcrun simctl create "QA SE" com.apple.CoreSimulator.SimDeviceType.iPhone-SE-3rd-generation com.apple.CoreSimulator.SimRuntime.iOS-26-2)
xcrun simctl boot $U
xcodebuild build -project swift/LoopflowMobile.xcodeproj -scheme LoopflowiOS \
  -destination "platform=iOS Simulator,id=$U" -derivedDataPath swift/.build/ios \
  -skipPackagePluginValidation CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- < /dev/null
xcrun simctl install $U swift/.build/ios/Build/Products/Debug-iphonesimulator/LoopflowiOS.app
xcrun simctl launch $U com.loopflow.ios.dev
```

Run `xcodebuild` with `LF_*` unset. Run
`xcodegen generate --spec swift/ios.yml` before building. A project generated
before a sync can miss Swift files added on main. No Simulator window is needed.

Drive it with `axe … --udid $U`: `describe-ui` (JSON tree with labels,
identifiers and frames in points), `tap --label` / `--id` / `-x -y`, `type`,
`swipe`, `button home`. `xcrun simctl io $U screenshot f.png`,
`xcrun simctl ui $U appearance dark`, `xcrun simctl ui $U content_size
accessibility-large`. `axe type` types through the hardware keyboard, so the
software keyboard stays down.

- Pair: tap "Enter address", type the endpoint from `lf server mobile
  --status` (`configuration.endpoint`; read it into a file, don't print it),
  tap `machine-address-continue`, then "Connect Machine". Relaunching goes
  straight to the list.
- Menus (filter, Task actions, Machine) appear in the tree as Buttons once
  open. The New Task sheet and its repository picker do not appear in the
  tree at all: tap by coordinates from a screenshot.
- A swipe from x=330 to x=120 on a row shows "Complete".

## Disposable Tasks on the live Machine

New Task lists the Machine's portfolio (`lf desktop portfolio --json`). In
`folder` mode a clone placed in that folder joins it and leaves when deleted;
a repository under `/tmp` never appears. Give the clone a local bare origin, a
local Git identity, `graph: qa` and a Graph whose agent steps cannot do
anything:

```yaml
# .lf/skills/pause.md: "Reply with the single line "pause reached" and stop."
# .lf/graphs/qa.yaml
- cmd: help task
- break: first-stop
- pause
- break: second-stop
```

Do not put a single terminal breakpoint with nothing after it there: continuing
past the last breakpoint ends the Graph and requests completion. `lf task
create '<words>'` in the clone (stdin closed) seeds an unstarted Task for
Backlog; Create & start on the phone makes one with a conversation, on the
repository's default agent.

Never open a personal Task's conversation from the phone: the view touched
last sizes the terminal. Changes and the workflow sentence of a Task with no
conversation are safe to read.

To see "This Session is open on another device.", run `lf session resume
<id>` in a PTY on the Mac (Python `pty.fork`, then `TIOCSWINSZ`) while the
phone shows the Session; the phone resumes drawing when that view leaves.

To see a refused action, `mv` a started Task's checkout out of the folder
while its screen shows: Run and Auto-Ship grey and the reason appears under
the sentence within seconds; `mv` it back to clear it. Auto-Ship on an
unstarted Task starts it and runs this Graph to its end in under a minute.
The provider stops at its trust prompt for the new clone; Return from the
key row answers "No, exit" and ends the conversation.

iPad: the same build installs on any iPad Simulator (`simctl list devices`),
which pairs the same way and shows the full workflow line. `simctl uninstall`
and `shutdown` it afterwards; it is not yours to delete. Nothing headless
rotates a Simulator, so landscape stays unseen.

Do not list processes with `pgrep -fl`: it prints every agent's whole prompt.
Use `ps -axo pid,command | grep <clone name> | cut -c1-110`.

Clean up: type `/exit` and Return into each conversation through a PTY
`lf session resume <id>`, `lf task abandon <id>` each Task that is not done,
delete the clone, its `.<task>` checkouts and `.ready`, then `xcrun simctl
shutdown` and `delete` the Simulator by UDID. Done Tasks cannot be abandoned
and stay in the store as Done.

## A branch server on a private Machine, step by step

`lf server mobile` needs `tailscale` on `PATH` (`~/.local/bin` here); with it
the private home gets its own port on the Mac's tailnet name. The phone lists
only repositories in the portfolio, `$HOME/src` of the private `HOME`: clone
the fake origin there (`git clone …/origin.git $HOME/src/<name>`, then the
`insteadOf` rewrite) and create Tasks in that clone, or the list stays empty.
Backlog is under the filter button (tap 38,42, then `--label Backlog`); Tasks
without a conversation sit under "N without a conversation", collapsed. The
expanded Notice's "Show" opens the Task; `--label Show` also matches the
filter button, so tap the Notice's action by coordinates. Restarting the
private server leaves the app "away" until `simctl terminate` and `launch`.
`lf server mobile --off` in the private home when done.
