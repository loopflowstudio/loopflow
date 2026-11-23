# LOO-1069 · Modifier keys in Desktop Sessions

> “Scope is Loopflow Desktop: modifier keys are not working in desktop sessions.” — Jack Heart

> “Observed live in Desktop: Shift does not work at all in agy sessions specifically (e.g. Shift+/ produces / instead of ?, Shift+a produces a), while working in other sessions in the same Desktop app.” — Jack Heart

> “Also include in this fix: sessions launched from Desktop / lf server (not from Rex or an existing terminal) have no color in agy and some claude sessions, whereas sessions started in Rex first (e.g. LOO-996) keep color.” — Jack Heart

The held-terminal registry fills missing or empty `TERM` and `COLORTERM` at launch. Desktop, server and Rex callers share that path.

Done when a focused Desktop Session delivers terminal-owned chords to the running program with their modifiers intact, including after reopening the Session. Desktop shortcuts and macOS app menus still work over a focused terminal, Ghostty terminal bindings run once without standalone-app defaults swallowing chords, Option text / dead keys / IME composition type and cancel cleanly, and the same keyboard behavior applies in a Desktop Shell.

## Model and design

One keyboard ownership hierarchy governs every Desktop terminal pane (Session and Shell):

1. **Active composition (`NSTextInputClient.hasMarkedText()`):** While any focused text input client — including `GhosttyMetalView` — has marked text, Desktop shortcut monitors (`DesktopActionMonitor`, `WorkPaletteShortcut`) and `GhosttyMetalView.flagsChanged` leave composition alone.
2. **Desktop window/pane shortcuts & macOS app menus:** `DesktopActionMonitor` (`DesktopActions.swift`, including `Ctrl-Tab`, `Ctrl-Shift-Tab`, `Ctrl-\`` when available) and `WorkPaletteShortcut` (`⌘K`, `⌘\`) act before the terminal. Inside `GhosttyMetalView.performKeyEquivalent`, macOS window cycling (`⌘\``, `⌘⇧~`) falls through to AppKit and exact `⌘C`/`⌘V`/`⌘F` stay with Desktop Copy/Paste/Find. Enabled exact app-menu equivalents (`⌘Q`, `⌘,`, `⌘H`, `⌘M`, `⌘⇧W`, `⌘⇧O`, `⌘/`) return `false` to AppKit.
3. **Embedded Ghostty terminal bindings:** `GhosttyManager.loopflowConfig` starts with `keybind = clear` and declares only the surface-owned terminal bindings Loopflow Desktop actually supports, before loading `~/.config/ghostty/config`. `embeddedConfig` loads after user files to pin `⌘↑`/`⌘↓` prompt jumping and keep Desktop-owned `⌘Q`, `⌘,`, and `⌘⇧W` unbound. Matching bindings, `⌃↩`, `⌃/`, and chords shadowed only by an enabled, visible Shift-required menu item route from `performKeyEquivalent` into `keyDown` and execute once.
4. **The running program:** Every remaining keystroke — including `⌘.` redirected by AppKit through `doCommand(by:)` — flows through a single AppKit-to-Ghostty key path into `ghostty_surface_key`, then through the PTY and held-terminal relay (`transport.rs`, `transport/hosted.rs`, `transport/input.rs`, `rust/loopflow-terminal/src/repaint.rs`) with keyboard modes restored on attach.

### What changes across layers

- **Single AppKit-to-Ghostty key path (`GhosttyTerminalView.swift`).** Follow the pinned Ghostty AppKit responder contract:
  - Delete split direct-bypass and paste-on-insert paths (done).
  - Translate modifiers with left/right side masks (`NX_DEVICEL*`/`NX_DEVICER*` -> `GHOSTTY_MODS_*_RIGHT` / base flags), carry press vs. `event.isARepeat` -> `GHOSTTY_ACTION_REPEAT` vs. release, and compute `unshifted_codepoint` from `event.characters(byApplyingModifiers: [])` (done).
  - Query `ghostty_surface_key_translation_mods(surface, mods)` on `keyDown` and update all four modifier flags (`[.shift, .control, .option, .command]`) from the result, rather than short-circuiting on `.option` alone, so AppKit interprets the layout modifiers Ghostty requests. The pinned translation API applies Option-as-Alt. Ghostty applies `key-remap` during binding lookup and key encoding. Construct a translated `NSEvent` only when the translated flags differ from `event.modifierFlags`; reuse the original `NSEvent` unchanged when equal (required by input methods such as Korean). Record `consumed_mods` from the translation modifiers (excluding Control and Super).
  - Run `interpretKeyEvents([translationEvent])` with `keyText` open; deliver collected text on the single `ghostty_surface_key` call. Clear `_markedRange` and preedit when `setMarkedText` receives an empty string; commit preedit text without triggering modifiers; emit nothing on cancel (done).
  - Fix `flagsChanged` to skip while `hasMarkedText()` is true and determine press vs. release from each keycode's side-specific `NX_DEVICE*` bit (done).
  - In `performKeyEquivalent`, keep first-responder, `surface`, `connectionAcceptsInput`, window-cycling (`⌘\``, `⌘⇧~`), and exact `GhosttyTerminalShortcut` (`⌘C`/`⌘V`/`⌘F`) handling. Route into `keyDown` and return `true` for:
    1. `ghostty_surface_key_is_binding` matches (so each binding executes once, including `unconsumed:` bindings);
    2. `⌃↩` (without Command, beating macOS's context-menu shortcut) and `⌃/` (Control only, preventing macOS's beep and sending `C-_` / Kitty `CSI 47;5u`);
    3. Unshifted chords that no enabled, visible exact menu item claims when an enabled, visible Shift-required menu item would misfire on them (`GhosttyTerminalShortcut.shadowedByShiftedMenuItem`, checking `!item.isHidden` and `item.isEnabled`);
    4. Command or Control chords re-dispatched from `doCommand(by:)` via Ghostty's `lastPerformKeyEvent` timestamp round-trip (ignoring synthetic `event.timestamp == 0` events, clearing `lastPerformKeyEvent = nil` in `keyDown`, and re-sending `NSApp.currentEvent` from `doCommand(by:)` when its timestamp matches `lastPerformKeyEvent`), so chords like `⌘.` reach the terminal once AppKit menus pass on them.
  - Return `false` for all other equivalents so AppKit menus (`⌘Q`, `⌘,`, `⌘H`, `⌘M`, `⌘⇧W`, `⌘⇧O`, `⌘/`) and non-Command `keyDown` dispatch run normally. Implement `@objc func selectAll(_ sender: Any?)` via `performBinding("select_all")` (done).
  - Keep existing connection/exit gating: awaiting-relay or disconnected panes send no PTY input (while Copy, Find, window cycling, and Desktop menus still work), a missing surface falls through to `super`, and an exited child still accepts its local dismiss key (done).
- **Ghostty config & keyboard-layout sync (`GhosttyManager.swift`).**
  - Observe `NSTextInputContext.keyboardSelectionDidChangeNotification` and call `ghostty_app_keyboard_changed(app)` so Ghostty's layout-dependent `macos-option-as-alt` default and physical-key mapping stay accurate when input sources change (done).
  - Replace the negative 49-entry `standaloneShortcuts` blacklist in `loopflowConfig` with `keybind = clear` followed by the explicit set of surface-owned macOS terminal bindings Loopflow Desktop supports (loaded before `~/.config/ghostty/config` so personal overrides win):
    - Natural macOS text editing: `alt+arrow_left=esc:b`, `alt+arrow_right=esc:f`, `super+arrow_left=text:\x01`, `super+arrow_right=text:\x05`, `super+backspace=text:\x15`
    - Additional prompt navigation: `super+shift+arrow_up=jump_to_prompt:-1`, `super+shift+arrow_down=jump_to_prompt:1`
    - Selection: `super+a=select_all`, `shift+arrow_left/right/up/down/page_up/page_down/home/end=performable:adjust_selection:...`
    - Viewport scrolling: `super+home=scroll_to_top`, `super+end=scroll_to_bottom`, `super+page_up=scroll_page_up`, `super+page_down=scroll_page_down`, `super+j=performable:scroll_to_selection`
    - Font size: `super+equal=increase_font_size:1`, `super+plus=increase_font_size:1`, `super+minus=decrease_font_size:1`, `super+0=reset_font_size` (verify exact Ghostty key names parse with `ghostty_config_diagnostics_count == 0`)
    - Terminal search navigation: `super+e=performable:search_selection`, `super+g=performable:navigate_search:next`, `super+shift+g=performable:navigate_search:previous`, `super+shift+f=performable:end_search`, `escape=performable:end_search`
  - Keep `embeddedConfig` (loaded after `~/.config/ghostty/config`) pinning `super+arrow_up/down=jump_to_prompt` and unbinding Desktop-owned app shortcuts (`super+q`, `super+,`, `super+comma`, `super+shift+w`) so personal Ghostty configs cannot break Quit, Settings, or Close Window.
- **Composition guard on Desktop shortcut monitors (`DesktopActions.swift`, `WorkPalette.swift`).**
  - Check `(window.firstResponder as? NSTextInputClient)?.hasMarkedText() != true` in both monitors (done).
- **Held-terminal relay (`rust/loopflow-terminal/src/repaint.rs`, `rust/loopflow-execution/src/transport/`).**
  - Existing mode replay and query filtering already align with this model; verified end-to-end across reattachment (done).

### Choices made for review

- **Clear Ghostty's standalone defaults with `keybind = clear` and declare only supported surface-owned bindings in `GhosttyManager.loopflowConfig`.** Passed over maintaining a negative `standaloneShortcuts` blacklist, which already missed 4 unimplemented macOS defaults (`super+shift+j`, `super+ctrl+shift+j`, `super+alt+shift+j`, `super+shift+v`) and breaks silently whenever Ghostty adds standalone chrome bindings. Reversible.
- **Complete Ghostty's AppKit responder contract (`⌃/`, `lastPerformKeyEvent` + `doCommand(by:)` for `⌘.`, 4-modifier `translationEvent`) and filter `shadowedByShiftedMenuItem` to visible, enabled menu items.** Passed over leaving those gaps in the responder or broadening this Task to fix AppKit Shift-menu matching outside terminals. Reversible.
- **Delete the direct printable-key bypass (`ghosttyShouldHandleKeyDownDirectly`).** Passed over keeping a second ASCII-only path because it skips `interpretKeyEvents` on non-IME dead-key layouts (US-International, ABC-Extended, European) and duplicates key translation. Reversible.
- **Keep Ghostty's effective `macos-option-as-alt` setting (layout-dependent default + user `~/.config/ghostty/config`) and sync layout changes via `ghostty_app_keyboard_changed`.** Passed over forcing a Desktop-wide Alt default, which would break accented Option typing on non-US keyboards and override explicit user settings. Reversible.

App-wide Shift-menu matching outside focused terminals (Sidebar, Brief, Files), unifying `DesktopActionMonitor` with `WorkPaletteShortcut`, clipboard paste, `lf desktop send`/`send-key`, selection, focus, connection recovery, iPhone, Rex, CLI flags, and wire formats are outside this change.

## Build and check

1. **Built the Option-B slice.** Desktop key equivalents now probe for a binding before sending a key. The shared Session and Shell responder lets AppKit interpret Option on the side Ghostty selected, then encodes committed text with the original key. Clipboard paste still uses the paste API.
2. **Built modifier, composition and shortcut ownership.** Each modifier release follows its physical key. Both Desktop shortcut monitors and the terminal's modifier handler defer during marked text. Input-source changes refresh Ghostty's keymap, and Edit → Select All selects terminal output.
3. **Verified held Session reattachment.** `SessionModifierInputTests` opens a prepared Session on a private Machine. Its provider sets the PTY to raw mode and requests Kitty disambiguation once, printing each batch as hex before and after releasing and reopening the surface.
4. **QA found and patched Shift-menu shadowing (`5c9193e1dd`).** Added `GhosttyTerminalShortcut.shadowedByShiftedMenuItem` so `⌘↩` and `⌘E` are not stolen by `⇧⌘↩` (Zoom Pane) and `⇧⌘E` (Toggle Browser).
5. **Built embedded terminal defaults and AppKit command dispatch.** Ghostty starts with cleared bindings and loads the supported editing, selection, scrolling, prompt, font and search actions. Option-Left and Option-Right keep their word movements. The responder routes Control-/ directly and gives menus first refusal on Command-period. Only visible, enabled menu items participate in Shift matching.

**Checked this slice.** `scripts/test_desktop.sh --filter 'GhosttyModifierInputTests|SessionModifierInputTests|embeddedConfigBindsPromptJumps'` passed 15 tests, including zero config diagnostics, `⌃/` in both protocols, one `⌘.` via AppKit command dispatch and held Session reattachment. One display-only composition check was skipped.

**Antigravity and color.** The installed `agy` 1.5.14 requested modifyOtherKeys level 2 and Kitty flags 5. The focused Swift run with the live QA probe passed 17 tests and skipped one display diagnostic. The unsent draft read `q?Ax`, then `q?Axq?Ax` after reattachment. The provider received `xterm-256color:truecolor`. The permanent fixtures cover both modes and reattachment without an installed provider. [QA report](qa.html)

- `cargo test -p loopflow-execution --lib held_children_get_terminal_colors_when_the_caller_leaves_them_unset` passed with missing, empty, custom and partial environments.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and the four `documented_commands::` checks passed.

**Remaining.** Gate and unlocked-Mac QA for physical key delivery, live menus, palette focus, input-source switching and IME composition.

### Previous checks

- **Slice 1–3 & compress:** `scripts/test_desktop.sh --skip-build --filter 'GhosttyModifierInputTests|SessionModifierInputTests'` passed 8 headless tests; mounted display tests (`focusedKeyEquivalents`, `composingInFocusedTerminal`, `pasteShortcutFollowsFirstResponder`, `typingAfterKeyboardRestore`, `nativeOpeningAndRecovery`) passed under `LOOPFLOW_NATIVE_TESTS=1`.
- **QA ([report](scratch/qa.html)):** Branch Dev bundle on a private Machine (screen locked, keys posted into the app event queue) verified legacy and Kitty modified Return/Tab/Option before and after quitting/relaunching the app, `⌘,` opening Settings, and `⌘↩`/`⌘E` after the Shift-menu fix (`11` headless tests passed). Open for a person at an unlocked Mac: live key-window dispatch order, `⌘K` palette, physical keyboard, input-source switching, and IME.

**Demo.** Open the isolated Session in Loopflow Desktop. Press Return, Shift-Return, and Control-Return; the fixture shows `0d`, `1b 5b 31 33 3b 32 75`, and `1b 5b 31 33 3b 35 75` once each. Close and reopen the pane and repeat. In legacy mode, Option-B shows `1b 62` with Option-as-Alt enabled and `∫` with it disabled on a US layout, and `⌃/` sends `1f` without beeping. Confirm `⌘Q`, `⌘,`, and `⌘⇧W` still reach Loopflow Desktop menus while the Kitty-mode Session is focused, and `⌘↩`, `⌘E`, `⇧⌘J`, and `⌘.` reach the program.

References: [Desktop keyboard docs](swift/README.md), [Concept review](scratch/concept-review.html), [Pinned Ghostty surface API](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/src/apprt/embedded.zig#L2015-L2076), [Ghostty AppKit responder](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/macos/Sources/Ghostty/Surface%20View/SurfaceView_AppKit.swift#L1102-L1426), [Ghostty event fields](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/macos/Sources/Ghostty/NSEvent%2BExtension.swift).
