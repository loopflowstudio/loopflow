# Mobile demo feedback · November 10

Jack Heart: “the demo i tried didnt have ar eal terminal”

The retained November 9 demo record says the native app launched with `--fixture`, backed by an in-memory editor, with no provider or real Session. `SessionTerminalView` uses SwiftTerm, but `FixtureTerminal` never executes commands. That demo did not demonstrate the required real Session experience.

Remove `--fixture` from the normal Xcode launch scheme; UI tests continue to opt in individually. Generated scheme inspected: no enabled fixture launch argument. Real connection demonstration still requires a matching installed Machine server and distinct Tailscale device. Do not describe fixture editing or automated acceptance as a hands-on live Session demo.
