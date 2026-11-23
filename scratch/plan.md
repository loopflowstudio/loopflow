# LOO-1069 · Modifier keys in Desktop Sessions

> “Scope is Loopflow Desktop: modifier keys are not working in desktop sessions.” — Jack Heart

> “Observed live in Desktop: Shift does not work at all in agy sessions specifically (e.g. Shift+/ produces / instead of ?, Shift+a produces a), while working in other sessions in the same Desktop app.” — Jack Heart

> “Also include in this fix: sessions launched from Desktop / lf server (not from Rex or an existing terminal) have no color in agy and some claude sessions, whereas sessions started in Rex first (e.g. LOO-996) keep color.” — Jack Heart

## What ships

A focused Desktop Session or Shell sends shifted text and terminal-owned chords once, in the program’s active keyboard protocol. Reopening a held Session restores that protocol. Held children get `TERM=xterm-256color` and `COLORTERM=truecolor` for missing or empty values. The registry keeps each nonempty caller setting.

- **Composition.** The Desktop and palette shortcut monitors yield to the focused `NSTextInputClient` while it has marked text. The terminal collects text from `interpretKeyEvents`, keeps preedit local and sends committed text without the composing key’s modifiers. Canceling emits no text.
- **Shortcuts.** Desktop window and pane monitors run first. The terminal responder handles Copy, Paste and Find, and returns window cycling to AppKit. Ghostty bindings, Control-Return, Control-/ and chords that only a shifted menu item would claim go through `keyDown`. Disabled and hidden menu items count too: AppKit validates at dispatch and they still consume their chord. When AppKit redirects Command-period to a responder command, the timestamp identifies the event to redispatch. Disconnected panes still offer local selection and menus. An exited child can still receive its dismiss key.
- **Translation.** `GhosttyKeyEvent.swift` maps AppKit modifiers to Ghostty, including the right-hand device bits, and applies Ghostty’s text modifiers for Option-as-Alt. Input methods get the original event whenever the flags agree. A modifier release follows its physical key even with the opposite side held. Input-source changes refresh Ghostty’s keyboard map.
- **Bindings.** The embedded defaults clear Ghostty’s standalone bindings and declare the supported editing, selection, scrollback, prompt, font and search shortcuts. Personal Ghostty configuration loads next. Desktop then pins Command-Up/Down prompt jumps and reserves Quit, Settings and Close Window. Clipboard paste and the held-terminal relay keep their existing paths.

## Checks

- The installed Antigravity 1.5.14 requested modifyOtherKeys level 2 and Kitty flags 5. Its unsent draft read `q?Ax`, then `q?Axq?Ax` after reattachment. The provider received `xterm-256color:truecolor`. The [QA report](qa.html) includes the byte traces, screenshots and locked-screen limits.
- The headless Desktop tests cover Option-B on either side, shifted printables in both protocols, modified Return and Tab, release and repeat, composition, shortcut ownership, personal remaps, config diagnostics and exact bytes before and after reattachment. The prior focused run with the live probe passed 17 tests and skipped one display diagnostic.
- Mounted tests passed for composition, paste focus, typing after scrollback restore and Session recovery. QA on a locked Mac verified Settings and the unshifted Command-Return and Command-E handoff. The held child retained its keyboard mode after quitting and relaunching Desktop.
- After extracting key translation, `scripts/test_desktop.sh --filter 'GhosttyModifierInputTests|SessionModifierInputTests|shortcutOwnership|windowCycleBelongsToAppKit|embeddedConfigBindsPromptJumps'` passed 18 tests. The skipped composition check passed in the native run. The window-isolation test observed its first surface before release finished. After awaiting release and the surviving relay’s readiness, `LOOPFLOW_NATIVE_TESTS=1 swift test --package-path swift --skip-build --no-parallel --filter 'releaseSurfaceIsWindowLocal|pasteShortcutFollowsFirstResponder'` passed both tests.
- `cargo test -p loopflow-execution --lib held_children_get_terminal_colors_when_the_caller_leaves_them_unset` passed for missing, empty, custom and partial environments. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and the four `documented_commands::` checks passed.

- QA after compression, on a locked Mac: legacy and Kitty bytes for Return, Shift/Control/Option-Return, Shift-Tab, Option-B on either side, Shift-/ and Shift-A matched the earlier run. `claims` showed the View menu taking ⌘↩ again: Zoom Pane read `isEnabled=false` and the enabled filter skipped it. A standalone `NSMenu` probe showed disabled and hidden Shift items still consume the unshifted chord. With the filter gone, ⌘↩ reached the provider as `CSI 13;9u`, and the focused Desktop filter passed 19 tests.

## Remaining

Gate and unlocked-Mac QA for physical keys, key-window menu dispatch, palette focus, input-source switching and IME composition. App-wide Shift-menu matching outside terminals and shortcut monitor consolidation belong to separate work.

For the demo, open the isolated Session and press Return, Shift-Return and Control-Return. The raw provider shows `0d`, `1b 5b 31 33 3b 32 75` and `1b 5b 31 33 3b 35 75`. Reopen the pane and repeat. With a US layout, compare Option-B as Alt (`1b 62`) and as text (`∫`). Check Quit, Settings and Close Window over the focused terminal, then type Command-Return, Command-E, Shift-Command-J and Command-period into a program that requests modified keys.

References: [Desktop keyboard docs](../swift/README.md), [Concept review](concept-review.html), [Pinned Ghostty surface API](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/src/apprt/embedded.zig#L2015-L2076), [Ghostty AppKit responder](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/macos/Sources/Ghostty/Surface%20View/SurfaceView_AppKit.swift#L1102-L1426), [Ghostty event fields](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/macos/Sources/Ghostty/NSEvent%2BExtension.swift).
